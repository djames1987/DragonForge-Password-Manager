use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use dragonforge_crypto::MlDsa65KeyPair;
use dragonforge_sync_server::{AppState, InMemoryStore, build_router};
use http_body_util::BodyExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

const ADMIN_TOKEN: &str = "phase10-secure-recovery-admin-token";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    account_id: Uuid,
    sync_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryConfigured {
    generation: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryBegun {
    generation: u64,
    envelope_hex: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryCompleted {
    generation: u64,
    sync_token: String,
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

async fn enroll_first(
    app: &axum::Router,
    token: &str,
    device_id: Uuid,
    name: &str,
    key: &MlDsa65KeyPair,
) {
    let verifying_key_hex = hex::encode(key.verifying_key().as_bytes());
    let message = format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        device_id, name, verifying_key_hex
    );
    let body = serde_json::json!({
        "deviceId": device_id,
        "name": name,
        "verifyingKeyHex": verifying_key_hex,
        "proofSignatureHex": hex::encode(key.sign(message.as_bytes())),
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
    assert_eq!(response.status(), StatusCode::CREATED);
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

fn auth_message(
    action: &str,
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    timestamp: u64,
    nonce_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-auth/v1\n{}\n{}\n{}\n{}\n{}\n{}",
        action, account_id, vault_id, generation, timestamp, nonce_hex
    )
    .into_bytes()
}

async fn begin(
    app: &axum::Router,
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    key: &MlDsa65KeyPair,
    nonce_byte: &str,
) -> axum::response::Response {
    let timestamp = now_seconds();
    let nonce_hex = nonce_byte.repeat(32);
    let signature = key.sign(&auth_message(
        "begin",
        account_id,
        vault_id,
        generation,
        timestamp,
        &nonce_hex,
    ));
    let body = serde_json::json!({
        "accountId": account_id,
        "vaultId": vault_id,
        "generation": generation,
        "timestamp": timestamp,
        "nonceHex": nonce_hex,
        "signatureHex": hex::encode(signature),
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/recovery/begin")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn recovery_rotates_credentials_revokes_devices_and_invalidates_old_kit() {
    let app = app();
    let account = create_account(&app).await;
    let vault_id = Uuid::new_v4();

    let first_device_id = Uuid::new_v4();
    let first_device_key = MlDsa65KeyPair::generate();
    enroll_first(
        &app,
        &account.sync_token,
        first_device_id,
        "Primary PC",
        &first_device_key,
    )
    .await;

    assert_eq!(
        signed_put(
            &app,
            &account.sync_token,
            first_device_id,
            &first_device_key,
            vault_id,
            0,
            b"opaque-recovery-test-vault",
        )
        .await,
        StatusCode::CREATED
    );

    let recovery_key = MlDsa65KeyPair::generate();
    let recovery_verifying_key_hex = hex::encode(recovery_key.verifying_key().as_bytes());
    let envelope = b"client-side-encrypted-account-secret-envelope";
    let envelope_hex = hex::encode(envelope);
    let configure_message = format!(
        "dragonforge/recovery-configure/v1\n{}\n{}\n{}\n{}",
        first_device_id,
        vault_id,
        recovery_verifying_key_hex,
        hex::encode(Sha256::digest(envelope)),
    );
    let configure_body = serde_json::json!({
        "vaultId": vault_id,
        "deviceId": first_device_id,
        "recoveryVerifyingKeyHex": recovery_verifying_key_hex,
        "envelopeHex": envelope_hex,
        "signatureHex": hex::encode(first_device_key.sign(configure_message.as_bytes())),
    });
    let configured = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/recovery/configure")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", account.sync_token),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&configure_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(configured.status(), StatusCode::CREATED);
    let configured: RecoveryConfigured =
        serde_json::from_slice(&body_bytes(configured).await).unwrap();
    assert_eq!(configured.generation, 1);

    let begun = begin(&app, account.account_id, vault_id, 1, &recovery_key, "ab").await;
    assert_eq!(begun.status(), StatusCode::OK);
    let begun: RecoveryBegun = serde_json::from_slice(&body_bytes(begun).await).unwrap();
    assert_eq!(begun.generation, 1);
    assert_eq!(begun.envelope_hex, hex::encode(envelope));

    let replayed = begin(&app, account.account_id, vault_id, 1, &recovery_key, "ab").await;
    assert_eq!(replayed.status(), StatusCode::FORBIDDEN);

    let replacement_id = Uuid::new_v4();
    let replacement_key = MlDsa65KeyPair::generate();
    let replacement_name = "Recovered PC";
    let replacement_vk = hex::encode(replacement_key.verifying_key().as_bytes());
    let replacement_proof_message = format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        replacement_id, replacement_name, replacement_vk
    );

    let new_recovery_key = MlDsa65KeyPair::generate();
    let new_recovery_vk = hex::encode(new_recovery_key.verifying_key().as_bytes());
    let new_recovery_proof_message = format!(
        "dragonforge/recovery-rotation/v1\n{}\n{}\n{}",
        account.account_id, vault_id, new_recovery_vk
    );
    let new_envelope = b"new-encrypted-account-secret-envelope";
    let new_envelope_hex = hex::encode(new_envelope);
    let timestamp = now_seconds();
    let nonce_hex = "cd".repeat(32);
    let complete_message = format!(
        "dragonforge/recovery-complete/v1\n{}\n{}\n1\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        account.account_id,
        vault_id,
        timestamp,
        nonce_hex,
        replacement_id,
        replacement_name,
        replacement_vk,
        new_recovery_vk,
        hex::encode(Sha256::digest(new_envelope)),
    );
    let complete_body = serde_json::json!({
        "accountId": account.account_id,
        "vaultId": vault_id,
        "generation": 1,
        "timestamp": timestamp,
        "nonceHex": nonce_hex,
        "replacementDeviceId": replacement_id,
        "replacementName": replacement_name,
        "replacementVerifyingKeyHex": replacement_vk,
        "replacementProofSignatureHex": hex::encode(replacement_key.sign(replacement_proof_message.as_bytes())),
        "newRecoveryVerifyingKeyHex": new_recovery_vk,
        "newRecoveryProofSignatureHex": hex::encode(new_recovery_key.sign(new_recovery_proof_message.as_bytes())),
        "newEnvelopeHex": new_envelope_hex,
        "signatureHex": hex::encode(recovery_key.sign(complete_message.as_bytes())),
    });
    let completed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/recovery/complete")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&complete_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::OK);
    let completed: RecoveryCompleted =
        serde_json::from_slice(&body_bytes(completed).await).unwrap();
    assert_eq!(completed.generation, 2);
    assert_ne!(completed.sync_token, account.sync_token);

    assert_eq!(
        signed_put(
            &app,
            &account.sync_token,
            first_device_id,
            &first_device_key,
            vault_id,
            1,
            b"old-device-write",
        )
        .await,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        signed_put(
            &app,
            &completed.sync_token,
            replacement_id,
            &replacement_key,
            vault_id,
            1,
            b"replacement-device-write",
        )
        .await,
        StatusCode::OK
    );

    let old_kit = begin(&app, account.account_id, vault_id, 1, &recovery_key, "bc").await;
    assert_eq!(old_kit.status(), StatusCode::CONFLICT);

    let rotated = begin(
        &app,
        account.account_id,
        vault_id,
        2,
        &new_recovery_key,
        "ef",
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
}
