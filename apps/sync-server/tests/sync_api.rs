use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dragonforge_sync_server::{AppState, InMemoryStore, build_router};
use http_body_util::BodyExt;
use serde::Deserialize;
use tower::ServiceExt;
use uuid::Uuid;

const ADMIN_TOKEN: &str = "phase7-test-admin-token";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    account_id: Uuid,
    sync_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncMetadata {
    revision: u64,
    content_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: String,
    current_revision: Option<u64>,
}

fn app() -> axum::Router {
    build_router(AppState::new(
        Arc::new(InMemoryStore::default()),
        Some(ADMIN_TOKEN),
    ))
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

async fn create_account(app: &axum::Router) -> AccountResponse {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts")
                .header("x-dragonforge-admin-token", ADMIN_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    serde_json::from_slice(&body_bytes(response).await).unwrap()
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[tokio::test]
async fn health_reports_protocol_version() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&body_bytes(response).await).unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["protocolVersion"], 1);
}

#[tokio::test]
async fn provisioning_requires_admin_token_and_returns_high_entropy_sync_token() {
    let router = app();
    let denied = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let account = create_account(&router).await;
    assert_ne!(account.account_id, Uuid::nil());
    assert_eq!(account.sync_token.len(), 64);
    assert!(
        account
            .sync_token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
}

#[tokio::test]
async fn opaque_vault_round_trip_and_revision_conflict_are_enforced() {
    let router = app();
    let account = create_account(&router).await;
    let vault_id = Uuid::new_v4();
    let first_blob = b"opaque-encrypted-vault-version-one".to_vec();

    let created = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .header("x-dragonforge-base-revision", "0")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(first_blob.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(created.status(), StatusCode::CREATED);
    let created_meta: SyncMetadata = serde_json::from_slice(&body_bytes(created).await).unwrap();
    assert_eq!(created_meta.revision, 1);
    assert_eq!(created_meta.content_sha256.len(), 64);

    let fetched = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(fetched.headers()["x-dragonforge-revision"], "1");
    assert_eq!(body_bytes(fetched).await, first_blob);

    let stale = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .header("x-dragonforge-base-revision", "0")
                .body(Body::from("stale-write"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let error: ErrorBody = serde_json::from_slice(&body_bytes(stale).await).unwrap();
    assert_eq!(error.code, "revisionConflict");
    assert_eq!(error.current_revision, Some(1));

    let second_blob = b"opaque-encrypted-vault-version-two".to_vec();
    let updated = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .header("x-dragonforge-base-revision", "1")
                .body(Body::from(second_blob.clone()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(updated.status(), StatusCode::OK);
    let updated_meta: SyncMetadata = serde_json::from_slice(&body_bytes(updated).await).unwrap();
    assert_eq!(updated_meta.revision, 2);

    let fetched = router
        .oneshot(
            Request::builder()
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(body_bytes(fetched).await, second_blob);
}

#[tokio::test]
async fn accounts_are_isolated_and_invalid_tokens_are_rejected() {
    let router = app();
    let first = create_account(&router).await;
    let second = create_account(&router).await;
    let vault_id = Uuid::new_v4();

    let created = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&first.sync_token))
                .header("x-dragonforge-base-revision", "0")
                .body(Body::from("ciphertext"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let isolated = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&second.sync_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(isolated.status(), StatusCode::NOT_FOUND);

    let unauthorized = router
        .oneshot(
            Request::builder()
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(
                    header::AUTHORIZATION,
                    "Bearer 0000000000000000000000000000000000000000000000000000000000000000",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn empty_payload_and_missing_base_revision_are_rejected() {
    let router = app();
    let account = create_account(&router).await;
    let vault_id = Uuid::new_v4();

    let missing_revision = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .body(Body::from("ciphertext"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_revision.status(), StatusCode::BAD_REQUEST);

    let empty = router
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/vaults/{vault_id}"))
                .header(header::AUTHORIZATION, bearer(&account.sync_token))
                .header("x-dragonforge-base-revision", "0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty.status(), StatusCode::BAD_REQUEST);
}
