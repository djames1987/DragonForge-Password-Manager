use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dragonforge_crypto::{MlDsa65VerifyingKey, constant_time_eq};
use sha2::{Digest, Sha256};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer};
use uuid::Uuid;

use crate::{
    AccountRecord, AccountResponse, ApiError, DeviceDecisionRequest, DeviceStatus, DeviceSummary,
    EnrollDeviceRequest, EnrollDeviceResponse, HealthResponse, MAX_SYNC_BLOB_BYTES,
    SYNC_PROTOCOL_VERSION, SyncMetadata, SyncStore, hash_sync_token, new_sync_token,
};

const HEADER_BASE_REVISION: &str = "x-dragonforge-base-revision";
const HEADER_REVISION: &str = "x-dragonforge-revision";
const HEADER_CONTENT_SHA256: &str = "x-dragonforge-content-sha256";
const HEADER_UPDATED_AT_MS: &str = "x-dragonforge-updated-at-ms";
const HEADER_ADMIN_TOKEN: &str = "x-dragonforge-admin-token";
const HEADER_DEVICE_ID: &str = "x-dragonforge-device-id";
const HEADER_DEVICE_TIMESTAMP: &str = "x-dragonforge-device-timestamp";
const HEADER_DEVICE_SIGNATURE: &str = "x-dragonforge-device-signature";
const MAX_DEVICE_CLOCK_SKEW_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct AppState {
    store: Arc<dyn SyncStore>,
    admin_token_hash: Option<[u8; 32]>,
}

impl AppState {
    pub fn new(store: Arc<dyn SyncStore>, admin_token: Option<&str>) -> Self {
        Self {
            store,
            admin_token_hash: admin_token.map(hash_sync_token),
        }
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/accounts", post(create_account))
        .route("/v1/devices/enroll", post(enroll_device))
        .route("/v1/devices", get(list_devices))
        .route("/v1/devices/{device_id}", get(device_status))
        .route("/v1/devices/{device_id}/approve", post(approve_device))
        .route("/v1/devices/{device_id}/revoke", post(revoke_device))
        .route("/v1/vaults/{vault_id}", get(get_vault).put(put_vault))
        .layer(RequestBodyLimitLayer::new(MAX_SYNC_BLOB_BYTES + 64 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        protocol_version: SYNC_PROTOCOL_VERSION,
    })
}

async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<AccountResponse>), ApiError> {
    require_admin(&state, &headers)?;

    let account_id = Uuid::new_v4();
    let sync_token = new_sync_token();
    state
        .store
        .create_account(AccountRecord {
            account_id,
            token_hash: hash_sync_token(&sync_token),
        })
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(AccountResponse {
            account_id,
            sync_token,
        }),
    ))
}

async fn enroll_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EnrollDeviceRequest>,
) -> Result<(StatusCode, Json<EnrollDeviceResponse>), ApiError> {
    let account_id = authenticate_account(&state, &headers).await?;
    let name = request.name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err(ApiError::BadRequest(
            "device name must contain 1 to 120 characters".to_owned(),
        ));
    }

    let verifying_key = hex::decode(&request.verifying_key_hex)
        .map_err(|_| ApiError::BadRequest("device verifying key must be hexadecimal".to_owned()))?;
    MlDsa65VerifyingKey::from_bytes(&verifying_key)
        .map_err(|_| ApiError::BadRequest("device verifying key is invalid".to_owned()))?;

    let (device, first_device) = state
        .store
        .enroll_device(
            account_id,
            request.device_id,
            name.to_owned(),
            verifying_key,
        )
        .await?;

    let status = if device.status == DeviceStatus::Active {
        StatusCode::CREATED
    } else {
        StatusCode::ACCEPTED
    };
    Ok((
        status,
        Json(EnrollDeviceResponse {
            device: DeviceSummary::from(&device),
            first_device,
        }),
    ))
}

async fn device_status(
    State(state): State<AppState>,
    Path(device_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DeviceSummary>, ApiError> {
    let account_id = authenticate_account(&state, &headers).await?;
    let device = state.store.get_device(account_id, device_id).await?;
    Ok(Json(DeviceSummary::from(&device)))
}

async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceSummary>>, ApiError> {
    let account_id = authenticate_signed_request(
        &state,
        &headers,
        "GET",
        "/v1/devices",
        &[],
        None,
    )
    .await?;
    let devices = state.store.list_devices(account_id).await?;
    Ok(Json(devices.iter().map(DeviceSummary::from).collect()))
}

async fn approve_device(
    State(state): State<AppState>,
    Path(target_device_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<DeviceDecisionRequest>,
) -> Result<Json<DeviceSummary>, ApiError> {
    let account_id = authenticate_account(&state, &headers).await?;
    let target = state
        .store
        .get_device(account_id, target_device_id)
        .await?;
    if target.status != DeviceStatus::Pending {
        return Err(ApiError::BadRequest(
            "only pending devices can be approved".to_owned(),
        ));
    }

    verify_device_decision(
        &state,
        account_id,
        request.approver_device_id,
        &target,
        "approve",
        &request.signature_hex,
    )
    .await?;

    let approved = state
        .store
        .set_device_status(account_id, target_device_id, DeviceStatus::Active)
        .await?;
    Ok(Json(DeviceSummary::from(&approved)))
}

async fn revoke_device(
    State(state): State<AppState>,
    Path(target_device_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<DeviceDecisionRequest>,
) -> Result<Json<DeviceSummary>, ApiError> {
    let account_id = authenticate_account(&state, &headers).await?;
    if request.approver_device_id == target_device_id {
        return Err(ApiError::BadRequest(
            "a device cannot revoke itself".to_owned(),
        ));
    }
    let target = state
        .store
        .get_device(account_id, target_device_id)
        .await?;
    if target.status == DeviceStatus::Revoked {
        return Ok(Json(DeviceSummary::from(&target)));
    }

    verify_device_decision(
        &state,
        account_id,
        request.approver_device_id,
        &target,
        "revoke",
        &request.signature_hex,
    )
    .await?;

    let revoked = state
        .store
        .set_device_status(account_id, target_device_id, DeviceStatus::Revoked)
        .await?;
    Ok(Json(DeviceSummary::from(&revoked)))
}

async fn get_vault(
    State(state): State<AppState>,
    Path(vault_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let path = format!("/v1/vaults/{vault_id}");
    let account_id =
        authenticate_signed_request(&state, &headers, "GET", &path, &[], None).await?;
    let stored = state.store.get_vault(account_id, vault_id).await?;

    let mut response = (StatusCode::OK, stored.ciphertext).into_response();
    let response_headers = response.headers_mut();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    insert_u64_header(response_headers, HEADER_REVISION, stored.revision)?;
    insert_u64_header(response_headers, HEADER_UPDATED_AT_MS, stored.updated_at_ms)?;
    insert_string_header(
        response_headers,
        HEADER_CONTENT_SHA256,
        &hex::encode(stored.content_sha256),
    )?;
    Ok(response)
}

async fn put_vault(
    State(state): State<AppState>,
    Path(vault_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<SyncMetadata>), ApiError> {
    let base_revision = parse_base_revision(&headers)?;
    let path = format!("/v1/vaults/{vault_id}");
    let account_id = authenticate_signed_request(
        &state,
        &headers,
        "PUT",
        &path,
        &body,
        Some(base_revision),
    )
    .await?;

    if body.is_empty() {
        return Err(ApiError::BadRequest(
            "encrypted sync payload must not be empty".to_owned(),
        ));
    }
    if body.len() > MAX_SYNC_BLOB_BYTES {
        return Err(ApiError::BadRequest(
            "encrypted sync payload exceeds the maximum size".to_owned(),
        ));
    }

    let digest = Sha256::digest(&body);
    let mut content_sha256 = [0_u8; 32];
    content_sha256.copy_from_slice(&digest);

    let stored = state
        .store
        .put_vault(
            account_id,
            vault_id,
            base_revision,
            body.to_vec(),
            content_sha256,
        )
        .await?;

    let status = if stored.revision == 1 {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((
        status,
        Json(SyncMetadata {
            vault_id,
            revision: stored.revision,
            content_sha256: hex::encode(stored.content_sha256),
            updated_at_ms: stored.updated_at_ms,
        }),
    ))
}

async fn authenticate_account(state: &AppState, headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_sync_token(token);
    state
        .store
        .authenticate(token_hash)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Unauthorized)
}

async fn authenticate_signed_request(
    state: &AppState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
    base_revision: Option<u64>,
) -> Result<Uuid, ApiError> {
    let account_id = authenticate_account(state, headers).await?;
    if state.store.list_devices(account_id).await?.is_empty() {
        return Ok(account_id);
    }
    let device_id = parse_device_id(headers)?;
    let timestamp = parse_device_timestamp(headers)?;
    validate_timestamp(timestamp)?;
    let signature = parse_signature(headers)?;

    let device = state.store.get_device(account_id, device_id).await?;
    if device.status != DeviceStatus::Active {
        return Err(ApiError::Forbidden);
    }

    let verifying_key = MlDsa65VerifyingKey::from_bytes(&device.verifying_key)
        .map_err(|_| ApiError::Forbidden)?;
    let message = request_signature_message(method, path, timestamp, body, base_revision);
    verifying_key
        .verify(&message, &signature)
        .map_err(|_| ApiError::Forbidden)?;
    Ok(account_id)
}

async fn verify_device_decision(
    state: &AppState,
    account_id: Uuid,
    approver_device_id: Uuid,
    target: &crate::DeviceRecord,
    action: &str,
    signature_hex: &str,
) -> Result<(), ApiError> {
    let approver = state
        .store
        .get_device(account_id, approver_device_id)
        .await?;
    if approver.status != DeviceStatus::Active {
        return Err(ApiError::Forbidden);
    }
    let verifying_key = MlDsa65VerifyingKey::from_bytes(&approver.verifying_key)
        .map_err(|_| ApiError::Forbidden)?;
    let signature = hex::decode(signature_hex)
        .map_err(|_| ApiError::BadRequest("device signature must be hexadecimal".to_owned()))?;
    let message = device_decision_message(approver_device_id, target.device_id, action);
    verifying_key
        .verify(&message, &signature)
        .map_err(|_| ApiError::Forbidden)
}

pub fn request_signature_message(
    method: &str,
    path: &str,
    timestamp: u64,
    body: &[u8],
    base_revision: Option<u64>,
) -> Vec<u8> {
    format!(
        "dragonforge/device-request/v1\n{}\n{}\n{}\n{}\n{}",
        method,
        path,
        timestamp,
        hex::encode(Sha256::digest(body)),
        base_revision
            .map(|value| value.to_string())
            .unwrap_or_default()
    )
    .into_bytes()
}

pub fn device_decision_message(
    approver_device_id: Uuid,
    target_device_id: Uuid,
    action: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/device-decision/v1\n{}\n{}\n{}",
        action, approver_device_id, target_device_id
    )
    .into_bytes()
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .ok_or(ApiError::Unauthorized)?
        .to_str()
        .map_err(|_| ApiError::Unauthorized)?;

    value
        .strip_prefix("Bearer ")
        .filter(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(ApiError::Unauthorized)
}

fn parse_device_id(headers: &HeaderMap) -> Result<Uuid, ApiError> {
    headers
        .get(HEADER_DEVICE_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(ApiError::Forbidden)
}

fn parse_device_timestamp(headers: &HeaderMap) -> Result<u64, ApiError> {
    headers
        .get(HEADER_DEVICE_TIMESTAMP)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(ApiError::Forbidden)
}

fn parse_signature(headers: &HeaderMap) -> Result<Vec<u8>, ApiError> {
    headers
        .get(HEADER_DEVICE_SIGNATURE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| hex::decode(value).ok())
        .ok_or(ApiError::Forbidden)
}

fn validate_timestamp(timestamp: u64) -> Result<(), ApiError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::Unavailable)?
        .as_secs();
    if now.abs_diff(timestamp) > MAX_DEVICE_CLOCK_SKEW_SECONDS {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = state.admin_token_hash.ok_or(ApiError::Forbidden)?;
    let supplied = headers
        .get(HEADER_ADMIN_TOKEN)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Forbidden)?;
    let supplied_hash = hash_sync_token(supplied);

    if constant_time_eq(&expected, &supplied_hash) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn parse_base_revision(headers: &HeaderMap) -> Result<u64, ApiError> {
    headers
        .get(HEADER_BASE_REVISION)
        .ok_or_else(|| ApiError::BadRequest(format!("{HEADER_BASE_REVISION} header is required")))?
        .to_str()
        .map_err(|_| ApiError::BadRequest("base revision header is invalid".to_owned()))?
        .parse::<u64>()
        .map_err(|_| ApiError::BadRequest("base revision must be an unsigned integer".to_owned()))
}

fn insert_u64_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: u64,
) -> Result<(), ApiError> {
    insert_string_header(headers, name, &value.to_string())
}

fn insert_string_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), ApiError> {
    let header_value = HeaderValue::from_str(value).map_err(|_| ApiError::Unavailable)?;
    headers.insert(name, header_value);
    Ok(())
}
