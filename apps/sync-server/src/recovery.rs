use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use dragonforge_crypto::MlDsa65VerifyingKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ApiError, AppState, DeviceStatus, DeviceSummary, MAX_RECOVERY_ENVELOPE_BYTES, RecoveryRecord,
    hash_sync_token, new_sync_token,
};

const MAX_CLOCK_SKEW_SECONDS: u64 = 300;
const MAX_ATTEMPTS_PER_WINDOW: usize = 10;
const ATTEMPT_WINDOW_SECONDS: u64 = 300;
const HEADER_REVISION: &str = "x-dragonforge-revision";
const HEADER_CONTENT_SHA256: &str = "x-dragonforge-content-sha256";

static RECOVERY_ATTEMPTS: OnceLock<Mutex<HashMap<Uuid, VecDeque<u64>>>> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigureRecoveryRequest {
    vault_id: Uuid,
    device_id: Uuid,
    recovery_verifying_key_hex: String,
    envelope_hex: String,
    signature_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigureRecoveryResponse {
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryAuthRequest {
    account_id: Uuid,
    vault_id: Uuid,
    timestamp: u64,
    nonce_hex: String,
    signature_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryBeginResponse {
    generation: u64,
    envelope_hex: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompleteRecoveryRequest {
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    timestamp: u64,
    nonce_hex: String,
    replacement_device_id: Uuid,
    replacement_name: String,
    replacement_verifying_key_hex: String,
    replacement_proof_signature_hex: String,
    new_recovery_verifying_key_hex: String,
    new_recovery_proof_signature_hex: String,
    new_envelope_hex: String,
    signature_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompleteRecoveryResponse {
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    sync_token: String,
    device: DeviceSummary,
}

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/recovery/configure", post(configure_recovery))
        .route("/v1/recovery/begin", post(begin_recovery))
        .route("/v1/recovery/vault", post(fetch_recovery_vault))
        .route("/v1/recovery/complete", post(complete_recovery))
        .with_state(state)
}

async fn configure_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ConfigureRecoveryRequest>,
) -> Result<(StatusCode, Json<ConfigureRecoveryResponse>), ApiError> {
    let account_id = authenticate_account(&state, &headers).await?;
    let device = state
        .store()
        .get_device(account_id, request.device_id)
        .await?;
    if device.status != DeviceStatus::Active {
        return Err(ApiError::Forbidden);
    }

    let recovery_key = parse_verifying_key(
        &request.recovery_verifying_key_hex,
        "recovery verifying key",
    )?;
    let envelope = parse_envelope(&request.envelope_hex)?;
    let signature = parse_hex(&request.signature_hex, "device signature")?;
    let device_key =
        MlDsa65VerifyingKey::from_bytes(&device.verifying_key).map_err(|_| ApiError::Forbidden)?;
    let message = configure_message(
        request.device_id,
        request.vault_id,
        &request.recovery_verifying_key_hex,
        &envelope,
    );
    device_key
        .verify(&message, &signature)
        .map_err(|_| ApiError::Forbidden)?;

    let record = state
        .store()
        .configure_recovery(
            account_id,
            request.vault_id,
            recovery_key.as_bytes().to_vec(),
            envelope,
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ConfigureRecoveryResponse {
            account_id,
            vault_id: record.vault_id,
            generation: record.generation,
        }),
    ))
}

async fn begin_recovery(
    State(state): State<AppState>,
    Json(request): Json<RecoveryAuthRequest>,
) -> Result<Json<RecoveryBeginResponse>, ApiError> {
    check_rate_limit(request.account_id)?;
    let record = authenticate_recovery(&state, &request, "begin").await?;
    Ok(Json(RecoveryBeginResponse {
        generation: record.generation,
        envelope_hex: hex::encode(record.envelope),
    }))
}

async fn fetch_recovery_vault(
    State(state): State<AppState>,
    Json(request): Json<RecoveryAuthRequest>,
) -> Result<Response, ApiError> {
    check_rate_limit(request.account_id)?;
    let record = authenticate_recovery(&state, &request, "vault").await?;
    let stored = state
        .store()
        .get_vault(request.account_id, record.vault_id)
        .await?;

    let mut response = (StatusCode::OK, stored.ciphertext).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
    headers.insert(
        HEADER_REVISION,
        HeaderValue::from_str(&stored.revision.to_string()).map_err(|_| ApiError::Unavailable)?,
    );
    headers.insert(
        HEADER_CONTENT_SHA256,
        HeaderValue::from_str(&hex::encode(stored.content_sha256))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn complete_recovery(
    State(state): State<AppState>,
    Json(request): Json<CompleteRecoveryRequest>,
) -> Result<Json<CompleteRecoveryResponse>, ApiError> {
    check_rate_limit(request.account_id)?;
    validate_timestamp(request.timestamp)?;
    validate_nonce(&request.nonce_hex)?;
    validate_device_name(&request.replacement_name)?;

    let record = state.store().get_recovery(request.account_id).await?;
    if record.vault_id != request.vault_id {
        return Err(ApiError::Forbidden);
    }
    if record.generation != request.generation {
        return Err(ApiError::RecoveryConflict);
    }

    let replacement_key = parse_verifying_key(
        &request.replacement_verifying_key_hex,
        "replacement device verifying key",
    )?;
    let replacement_proof = parse_hex(
        &request.replacement_proof_signature_hex,
        "replacement device proof",
    )?;
    replacement_key
        .verify(
            &device_enrollment_message(
                request.replacement_device_id,
                request.replacement_name.trim(),
                &request.replacement_verifying_key_hex,
            ),
            &replacement_proof,
        )
        .map_err(|_| ApiError::Forbidden)?;

    let new_recovery_key = parse_verifying_key(
        &request.new_recovery_verifying_key_hex,
        "new recovery verifying key",
    )?;
    let new_recovery_proof = parse_hex(
        &request.new_recovery_proof_signature_hex,
        "new recovery proof",
    )?;
    new_recovery_key
        .verify(
            &recovery_rotation_message(
                request.account_id,
                request.vault_id,
                &request.new_recovery_verifying_key_hex,
            ),
            &new_recovery_proof,
        )
        .map_err(|_| ApiError::Forbidden)?;

    let new_envelope = parse_envelope(&request.new_envelope_hex)?;
    let signature = parse_hex(&request.signature_hex, "recovery signature")?;
    let old_recovery_key =
        MlDsa65VerifyingKey::from_bytes(&record.verifying_key).map_err(|_| ApiError::Forbidden)?;
    let message = complete_message(&request, &new_envelope);
    old_recovery_key
        .verify(&message, &signature)
        .map_err(|_| ApiError::Forbidden)?;

    let sync_token = new_sync_token();
    let (device, recovery) = state
        .store()
        .complete_recovery(
            request.account_id,
            request.generation,
            request.replacement_device_id,
            request.replacement_name.trim().to_owned(),
            replacement_key.as_bytes().to_vec(),
            new_recovery_key.as_bytes().to_vec(),
            new_envelope,
            hash_sync_token(&sync_token),
        )
        .await?;

    Ok(Json(CompleteRecoveryResponse {
        account_id: request.account_id,
        vault_id: recovery.vault_id,
        generation: recovery.generation,
        sync_token,
        device: DeviceSummary::from(&device),
    }))
}

async fn authenticate_recovery(
    state: &AppState,
    request: &RecoveryAuthRequest,
    action: &str,
) -> Result<RecoveryRecord, ApiError> {
    validate_timestamp(request.timestamp)?;
    validate_nonce(&request.nonce_hex)?;
    let record = state.store().get_recovery(request.account_id).await?;
    if record.vault_id != request.vault_id {
        return Err(ApiError::Forbidden);
    }
    let verifying_key =
        MlDsa65VerifyingKey::from_bytes(&record.verifying_key).map_err(|_| ApiError::Forbidden)?;
    let signature = parse_hex(&request.signature_hex, "recovery signature")?;
    let message = recovery_auth_message(
        action,
        request.account_id,
        request.vault_id,
        record.generation,
        request.timestamp,
        &request.nonce_hex,
    );
    verifying_key
        .verify(&message, &signature)
        .map_err(|_| ApiError::Forbidden)?;
    Ok(record)
}

async fn authenticate_account(state: &AppState, headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let value = headers
        .get(header::AUTHORIZATION)
        .ok_or(ApiError::Unauthorized)?
        .to_str()
        .map_err(|_| ApiError::Unauthorized)?;
    let token = value
        .strip_prefix("Bearer ")
        .filter(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(ApiError::Unauthorized)?;
    state
        .store()
        .authenticate(hash_sync_token(token))
        .await?
        .ok_or(ApiError::Unauthorized)
}

fn parse_verifying_key(value: &str, label: &str) -> Result<MlDsa65VerifyingKey, ApiError> {
    let bytes = parse_hex(value, label)?;
    MlDsa65VerifyingKey::from_bytes(&bytes)
        .map_err(|_| ApiError::BadRequest(format!("{label} is invalid")))
}

fn parse_envelope(value: &str) -> Result<Vec<u8>, ApiError> {
    let envelope = parse_hex(value, "recovery envelope")?;
    if envelope.is_empty() || envelope.len() > MAX_RECOVERY_ENVELOPE_BYTES {
        return Err(ApiError::BadRequest(
            "recovery envelope size is invalid".to_owned(),
        ));
    }
    Ok(envelope)
}

fn parse_hex(value: &str, label: &str) -> Result<Vec<u8>, ApiError> {
    hex::decode(value)
        .map_err(|_| ApiError::BadRequest(format!("{label} must be hexadecimal")))
}

fn validate_device_name(name: &str) -> Result<(), ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ApiError::BadRequest(
            "device name must contain 1 to 120 characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_nonce(nonce_hex: &str) -> Result<(), ApiError> {
    let nonce = parse_hex(nonce_hex, "recovery nonce")?;
    if nonce.len() != 32 {
        return Err(ApiError::BadRequest(
            "recovery nonce must be 32 bytes".to_owned(),
        ));
    }
    Ok(())
}

fn validate_timestamp(timestamp: u64) -> Result<(), ApiError> {
    let now = now_seconds()?;
    if now.abs_diff(timestamp) > MAX_CLOCK_SKEW_SECONDS {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

fn check_rate_limit(account_id: Uuid) -> Result<(), ApiError> {
    let now = now_seconds()?;
    let attempts = RECOVERY_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut attempts = attempts.lock().map_err(|_| ApiError::Unavailable)?;
    let entries = attempts.entry(account_id).or_default();
    while entries
        .front()
        .is_some_and(|time| now.saturating_sub(*time) > ATTEMPT_WINDOW_SECONDS)
    {
        entries.pop_front();
    }
    if entries.len() >= MAX_ATTEMPTS_PER_WINDOW {
        return Err(ApiError::BadRequest(
            "too many recovery attempts; wait five minutes before retrying".to_owned(),
        ));
    }
    entries.push_back(now);
    Ok(())
}

fn now_seconds() -> Result<u64, ApiError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| ApiError::Unavailable)
}

fn configure_message(
    device_id: Uuid,
    vault_id: Uuid,
    recovery_verifying_key_hex: &str,
    envelope: &[u8],
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-configure/v1\n{}\n{}\n{}\n{}",
        device_id,
        vault_id,
        recovery_verifying_key_hex.to_ascii_lowercase(),
        hex::encode(Sha256::digest(envelope))
    )
    .into_bytes()
}

fn recovery_auth_message(
    action: &str,
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    timestamp: u64,
    nonce_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-auth/v1\n{}\n{}\n{}\n{}\n{}\n{}",
        action,
        account_id,
        vault_id,
        generation,
        timestamp,
        nonce_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

fn complete_message(request: &CompleteRecoveryRequest, new_envelope: &[u8]) -> Vec<u8> {
    format!(
        "dragonforge/recovery-complete/v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        request.account_id,
        request.vault_id,
        request.generation,
        request.timestamp,
        request.nonce_hex.to_ascii_lowercase(),
        request.replacement_device_id,
        request.replacement_name.trim(),
        request.replacement_verifying_key_hex.to_ascii_lowercase(),
        request.new_recovery_verifying_key_hex.to_ascii_lowercase(),
        hex::encode(Sha256::digest(new_envelope))
    )
    .into_bytes()
}

fn recovery_rotation_message(
    account_id: Uuid,
    vault_id: Uuid,
    verifying_key_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-rotation/v1\n{}\n{}\n{}",
        account_id,
        vault_id,
        verifying_key_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

fn device_enrollment_message(
    device_id: Uuid,
    name: &str,
    verifying_key_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        device_id,
        name,
        verifying_key_hex.to_ascii_lowercase()
    )
    .into_bytes()
}
