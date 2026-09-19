use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dragonforge_crypto::MlDsa65KeyPair;
use dragonforge_sync_server::{
    AccountRecord, AppState, DeviceStatus, InMemoryStore, SyncStore, build_router, hash_sync_token,
};
use http_body_util::BodyExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

const ADMIN_TOKEN: &str = "phase9-device-enrollment-admin-token";
const SYNC_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    sync_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    device_id: Uuid,
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrollResponse {
    device: Device,
    first_device: bool,
}

fn app() -> axum::Router {
    build_router(AppState::new(
        Arc::new(InMemoryStore::default()),
        Some(ADMIN_TOKEN),
    ))
}

async fn response_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

async fn create_account(app: &axum::Router) -> String {
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
    let account: AccountResponse =
        serde_json::from_slice(&response_bytes(response).await).unwrap();
    account.sync_token
}

async fn enroll(
    app: &axum::Router,
    token: &str,
    device_id: Uuid,
    name: &str,
    key: &MlDsa65KeyPair,
) -> EnrollResponse {
    let verifying_key_hex = hex::encode(key.verifying_key().as_bytes());
    let proof_message = format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        device_id,
        name,
        verifying_key_hex
    );
    let body = serde_json::json!({
        "deviceId": device_id,
        "name": name,
        "verifyingKeyHex": verifying_key_hex,
        "proofSignatureHex": hex::encode(key.sign(proof_message.as_bytes())),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/devices/enroll")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        matches!(
            response.status(),
            StatusCode::CREATED | StatusCode::ACCEPTED
        ),
        "{}",
        response.status()
    );
    serde_json::from_slice(&response_bytes(response).await).unwrap()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn request_message(
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
        base_revision.map(|value| value.to_string()).unwrap_or_default()
    )
    .into_bytes()
}

fn decision_message(approver: Uuid, target: Uuid, action: &str) -> Vec<u8> {
    format!(
        "dragonforge/device-decision/v1\n{}\n{}\n{}",
        action, approver, target
    )
    .into_bytes()
}

async fn signed_put(
    app: &axum::Router,
    token: &str,
    device_id: Uuid,
    key: &MlDsa65KeyPair,
    vault_id: Uuid,
    base_revision: u64,
    body: &[u8],
) -> StatusCode {
    let path = format!("/v1/vaults/{vault_id}");
    let timestamp = now_seconds();
    let signature = key.sign(&request_message(
        "PUT",
        &path,
        timestamp,
        body,
        Some(base_revision),
    ));
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-dragonforge-device-id", device_id.to_string())
                .header("x-dragonforge-device-timestamp", timestamp.to_string())
                .header("x-dragonforge-device-signature", hex::encode(signature))
                .header("x-dragonforge-base-revision", base_revision.to_string())
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn device_enrollment_approval_and_revocation_gate_sync_access() {
    let app = app();
    let token = create_account(&app).await;
    assert_ne!(token, SYNC_TOKEN);

    let first_id = Uuid::new_v4();
    let first_key = MlDsa65KeyPair::generate();
    let first = enroll(&app, &token, first_id, "Primary PC", &first_key).await;
    assert!(first.first_device);
    assert_eq!(first.device.device_id, first_id);
    assert_eq!(first.device.status, "active");

    let second_id = Uuid::new_v4();
    let second_key = MlDsa65KeyPair::generate();
    let second = enroll(&app, &token, second_id, "Laptop", &second_key).await;
    assert!(!second.first_device);
    assert_eq!(second.device.status, "pending");

    let vault_id = Uuid::new_v4();
    assert_eq!(
        signed_put(
            &app,
            &token,
            second_id,
            &second_key,
            vault_id,
            0,
            b"opaque-ciphertext"
        )
        .await,
        StatusCode::FORBIDDEN
    );

    let approval = serde_json::json!({
        "approverDeviceId": first_id,
        "signatureHex": hex::encode(first_key.sign(&decision_message(first_id, second_id, "approve"))),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/devices/{second_id}/approve"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&approval).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        signed_put(
            &app,
            &token,
            second_id,
            &second_key,
            vault_id,
            0,
            b"opaque-ciphertext"
        )
        .await,
        StatusCode::CREATED
    );

    let revocation = serde_json::json!({
        "approverDeviceId": first_id,
        "signatureHex": hex::encode(first_key.sign(&decision_message(first_id, second_id, "revoke"))),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/devices/{second_id}/revoke"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&revocation).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        signed_put(
            &app,
            &token,
            second_id,
            &second_key,
            vault_id,
            1,
            b"new-ciphertext"
        )
        .await,
        StatusCode::FORBIDDEN
    );
}


#[tokio::test]
async fn revoked_device_history_does_not_allow_first_device_rebootstrap() {
    let store = InMemoryStore::default();
    let account_id = Uuid::new_v4();
    store
        .create_account(AccountRecord {
            account_id,
            token_hash: hash_sync_token(SYNC_TOKEN),
        })
        .await
        .unwrap();

    let first_id = Uuid::new_v4();
    let first_key = MlDsa65KeyPair::generate();
    let (first, first_device) = store
        .enroll_device(
            account_id,
            first_id,
            "Lost PC".to_owned(),
            first_key.verifying_key().as_bytes().to_vec(),
        )
        .await
        .unwrap();
    assert!(first_device);
    assert_eq!(first.status, DeviceStatus::Active);

    store
        .set_device_status(account_id, first_id, DeviceStatus::Revoked)
        .await
        .unwrap();
    assert!(
        store
            .set_device_status(account_id, first_id, DeviceStatus::Active)
            .await
            .is_err(),
        "revoked device identity was reactivated"
    );

    let replacement_id = Uuid::new_v4();
    let replacement_key = MlDsa65KeyPair::generate();
    let (replacement, first_device) = store
        .enroll_device(
            account_id,
            replacement_id,
            "Unapproved replacement".to_owned(),
            replacement_key.verifying_key().as_bytes().to_vec(),
        )
        .await
        .unwrap();

    assert!(!first_device);
    assert_eq!(replacement.status, DeviceStatus::Pending);
}


#[tokio::test]
async fn enrollment_rejects_invalid_device_proof_of_possession() {
    let app = app();
    let token = create_account(&app).await;
    let device_id = Uuid::new_v4();
    let claimed_key = MlDsa65KeyPair::generate();
    let wrong_key = MlDsa65KeyPair::generate();
    let verifying_key_hex = hex::encode(claimed_key.verifying_key().as_bytes());
    let proof_message = format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        device_id,
        "Untrusted Device",
        verifying_key_hex
    );
    let body = serde_json::json!({
        "deviceId": device_id,
        "name": "Untrusted Device",
        "verifyingKeyHex": verifying_key_hex,
        "proofSignatureHex": hex::encode(wrong_key.sign(proof_message.as_bytes())),
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/devices/enroll")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}


#[tokio::test]
async fn enrolled_accounts_require_fresh_signed_device_requests() {
    let app = app();
    let token = create_account(&app).await;
    let device_id = Uuid::new_v4();
    let key = MlDsa65KeyPair::generate();
    let enrolled = enroll(&app, &token, device_id, "Primary PC", &key).await;
    assert_eq!(enrolled.device.status, "active");

    let vault_id = Uuid::new_v4();
    let path = format!("/v1/vaults/{vault_id}");

    let unsigned = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unsigned.status(), StatusCode::FORBIDDEN);

    let stale_timestamp = now_seconds().saturating_sub(301);
    let stale_signature = key.sign(&request_message(
        "GET",
        &path,
        stale_timestamp,
        &[],
        None,
    ));
    let stale = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-dragonforge-device-id", device_id.to_string())
                .header(
                    "x-dragonforge-device-timestamp",
                    stale_timestamp.to_string(),
                )
                .header(
                    "x-dragonforge-device-signature",
                    hex::encode(stale_signature),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::FORBIDDEN);

    let timestamp = now_seconds();
    let valid_signature = key.sign(&request_message("GET", &path, timestamp, &[], None));
    let valid = app
        .oneshot(
            Request::builder()
                .uri(&path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-dragonforge-device-id", device_id.to_string())
                .header("x-dragonforge-device-timestamp", timestamp.to_string())
                .header(
                    "x-dragonforge-device-signature",
                    hex::encode(valid_signature),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid.status(), StatusCode::NOT_FOUND);
}
