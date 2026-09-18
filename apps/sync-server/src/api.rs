use std::{sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use dragonforge_crypto::constant_time_eq;
use sha2::{Digest, Sha256};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer};
use uuid::Uuid;

use crate::{
    AccountRecord, AccountResponse, ApiError, HealthResponse, MAX_SYNC_BLOB_BYTES,
    SYNC_PROTOCOL_VERSION, SyncMetadata, SyncStore, hash_sync_token, new_sync_token,
};

const HEADER_BASE_REVISION: &str = "x-dragonforge-base-revision";
const HEADER_REVISION: &str = "x-dragonforge-revision";
const HEADER_CONTENT_SHA256: &str = "x-dragonforge-content-sha256";
const HEADER_UPDATED_AT_MS: &str = "x-dragonforge-updated-at-ms";
const HEADER_ADMIN_TOKEN: &str = "x-dragonforge-admin-token";

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
        .route("/v1/vaults/{vault_id}", get(get_vault).put(put_vault))
        .layer(RequestBodyLimitLayer::new(MAX_SYNC_BLOB_BYTES + 64 * 1024))
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
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

async fn get_vault(
    State(state): State<AppState>,
    Path(vault_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let account_id = authenticate(&state, &headers).await?;
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
    let account_id = authenticate(&state, &headers).await?;
    let base_revision = parse_base_revision(&headers)?;

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

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let token = bearer_token(headers)?;
    let token_hash = hash_sync_token(token);
    state
        .store
        .authenticate(token_hash)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::Unauthorized)
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
