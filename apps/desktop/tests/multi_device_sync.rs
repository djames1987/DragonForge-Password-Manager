use std::{
    fs,
    sync::{Arc, mpsc},
    thread,
};
use dragonforge_desktop::{DesktopService, ItemDraft};
use dragonforge_sync_server::{
    AccountRecord, AppState, InMemoryStore, SyncStore, build_router, hash_sync_token,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use uuid::Uuid;

const MASTER: &str = "phase-eight-multi-device-master";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn start_sync_server() -> String {
    let store = InMemoryStore::default();
    let account = AccountRecord {
        account_id: Uuid::new_v4(),
        token_hash: hash_sync_token(TOKEN),
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(store.create_account(account)).unwrap();
    let router = build_router(AppState::new(Arc::new(store), None));

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            sender.send(listener.local_addr().unwrap()).unwrap();
            axum::serve(listener, router).await.unwrap();
        });
    });

    let address = receiver.recv().unwrap();
    format!("http://{address}")
}

fn start_sync_server_with_store() -> (String, InMemoryStore, Uuid) {
    let store = InMemoryStore::default();
    let account_id = Uuid::new_v4();
    let account = AccountRecord {
        account_id,
        token_hash: hash_sync_token(TOKEN),
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(store.create_account(account)).unwrap();
    let router = build_router(AppState::new(Arc::new(store.clone()), None));

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            sender.send(listener.local_addr().unwrap()).unwrap();
            axum::serve(listener, router).await.unwrap();
        });
    });

    let address = receiver.recv().unwrap();
    (format!("http://{address}"), store, account_id)
}

fn login(name: &str, username: &str) -> ItemDraft {
    ItemDraft {
        id: None,
        name: name.into(),
        kind: "login".into(),
        favorite: false,
        tags: vec!["sync".into()],
        username: username.into(),
        password: format!("{name}-password"),
        url: "https://example.com/login".into(),
        notes: format!("{name} synchronized note"),
    }
}

#[test]
fn encrypted_vault_syncs_between_two_devices_and_detects_conflicts() {
    let server = start_sync_server();
    let temp = tempdir().unwrap();
    let device_a_path = temp.path().join("device-a.dfvault");
    let device_b_path = temp.path().join("device-b.dfvault");

    let device_a = DesktopService::default();
    let created = device_a.create_vault(&device_a_path, MASTER).unwrap();
    device_a
        .save_item(&login("Initial Login", "initial@example.com"))
        .unwrap();
    device_a.configure_sync(&server, TOKEN).unwrap();

    let initial = device_a.sync_now().unwrap();
    assert_eq!(initial.action, "uploaded");
    assert_eq!(initial.revision, 1);
    assert!(!initial.vault_locked);

    fs::copy(&device_a_path, &device_b_path).unwrap();

    let device_b = DesktopService::default();
    device_b
        .unlock_vault(&device_b_path, MASTER, &created.account_secret_hex)
        .unwrap();
    device_b.configure_sync(&server, TOKEN).unwrap();

    let pending = device_b.enroll_device(Some("Device B")).unwrap();
    assert_eq!(pending.status, "pending");
    let approved = device_a.approve_device(&pending.device_id).unwrap();
    assert_eq!(approved.status, "active");

    let initial_conflict = device_b.sync_now().unwrap();
    assert_eq!(initial_conflict.action, "initialConflict");

    let adopted = device_b.resolve_sync_conflict("keepRemote").unwrap();
    assert_eq!(adopted.action, "keptRemote");
    assert!(adopted.vault_locked);
    device_b
        .unlock_vault(&device_b_path, MASTER, &created.account_secret_hex)
        .unwrap();

    device_a
        .save_item(&login("Device A Login", "a@example.com"))
        .unwrap();
    let uploaded = device_a.sync_now().unwrap();
    assert_eq!(uploaded.action, "uploaded");
    assert_eq!(uploaded.revision, 2);

    let downloaded = device_b.sync_now().unwrap();
    assert_eq!(downloaded.action, "downloaded");
    assert!(downloaded.vault_locked);
    assert!(!device_b.status().unwrap().unlocked);

    device_b
        .unlock_vault(&device_b_path, MASTER, &created.account_secret_hex)
        .unwrap();
    assert_eq!(
        device_b.list_items(Some("Device A Login")).unwrap().len(),
        1
    );

    device_a
        .save_item(&login("A Conflict Winner", "winner@example.com"))
        .unwrap();
    device_b
        .save_item(&login("B Local Conflict", "local@example.com"))
        .unwrap();

    let a_revision = device_a.sync_now().unwrap();
    assert_eq!(a_revision.action, "uploaded");
    assert_eq!(a_revision.revision, 3);

    let conflict = device_b.sync_now().unwrap();
    assert_eq!(conflict.action, "conflict");
    assert_eq!(conflict.revision, 3);
    assert!(device_b.status().unwrap().unlocked);

    let resolved = device_b.resolve_sync_conflict("keepRemote").unwrap();
    assert_eq!(resolved.action, "keptRemote");
    assert!(resolved.vault_locked);

    device_b
        .unlock_vault(&device_b_path, MASTER, &created.account_secret_hex)
        .unwrap();
    assert_eq!(
        device_b
            .list_items(Some("A Conflict Winner"))
            .unwrap()
            .len(),
        1
    );
    assert!(
        device_b
            .list_items(Some("B Local Conflict"))
            .unwrap()
            .is_empty()
    );

    let status = device_b.sync_status().unwrap();
    assert!(status.configured);
    assert_eq!(status.last_revision, 3);
}

#[test]
fn explicit_keep_local_conflict_resolution_uploads_new_revision() {
    let server = start_sync_server();
    let temp = tempdir().unwrap();
    let device_a_path = temp.path().join("local-a.dfvault");
    let device_b_path = temp.path().join("local-b.dfvault");

    let device_a = DesktopService::default();
    let created = device_a.create_vault(&device_a_path, MASTER).unwrap();
    device_a.configure_sync(&server, TOKEN).unwrap();
    assert_eq!(device_a.sync_now().unwrap().revision, 1);

    fs::copy(&device_a_path, &device_b_path).unwrap();
    let device_b = DesktopService::default();
    device_b
        .unlock_vault(&device_b_path, MASTER, &created.account_secret_hex)
        .unwrap();
    device_b.configure_sync(&server, TOKEN).unwrap();
    let pending = device_b.enroll_device(Some("Device B")).unwrap();
    assert_eq!(pending.status, "pending");
    device_a.approve_device(&pending.device_id).unwrap();
    assert_eq!(
        device_b.resolve_sync_conflict("keepRemote").unwrap().action,
        "keptRemote"
    );
    device_b
        .unlock_vault(&device_b_path, MASTER, &created.account_secret_hex)
        .unwrap();

    device_a
        .save_item(&login("Remote Change", "remote@example.com"))
        .unwrap();
    assert_eq!(device_a.sync_now().unwrap().revision, 2);

    device_b
        .save_item(&login("Local Choice", "local-choice@example.com"))
        .unwrap();
    assert_eq!(device_b.sync_now().unwrap().action, "conflict");

    let kept = device_b.resolve_sync_conflict("keepLocal").unwrap();
    assert_eq!(kept.action, "keptLocal");
    assert_eq!(kept.revision, 3);
    assert!(!kept.vault_locked);

    let pulled = device_a.sync_now().unwrap();
    assert_eq!(pulled.action, "downloaded");
    device_a
        .unlock_vault(&device_a_path, MASTER, &created.account_secret_hex)
        .unwrap();
    assert_eq!(device_a.list_items(Some("Local Choice")).unwrap().len(), 1);
}

#[test]
fn tampered_remote_snapshot_is_rejected_before_local_replacement() {
    let (server, store, account_id) = start_sync_server_with_store();
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("tamper-test.dfvault");

    let device = DesktopService::default();
    let created = device.create_vault(&vault_path, MASTER).unwrap();
    device
        .save_item(&login("Protected Login", "protected@example.com"))
        .unwrap();
    device.configure_sync(&server, TOKEN).unwrap();

    let initial = device.sync_now().unwrap();
    assert_eq!(initial.action, "uploaded");
    assert_eq!(initial.revision, 1);

    let original_bytes = fs::read(&vault_path).unwrap();
    let mut tampered_json: serde_json::Value = serde_json::from_slice(&original_bytes).unwrap();
    let ciphertext = tampered_json["items"][0]["payload"]["ciphertext"]
        .as_array_mut()
        .expect("encrypted item ciphertext array");
    let first_byte = ciphertext[0].as_u64().expect("ciphertext byte");
    ciphertext[0] = serde_json::Value::from((first_byte ^ 1) as u8);
    let tampered_bytes = serde_json::to_vec_pretty(&tampered_json).unwrap();

    let vault_id = Uuid::parse_str(&device.status().unwrap().vault_id.unwrap()).unwrap();
    let content_sha256: [u8; 32] = Sha256::digest(&tampered_bytes).into();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let stored = runtime
        .block_on(store.put_vault(
            account_id,
            vault_id,
            1,
            tampered_bytes,
            content_sha256,
        ))
        .unwrap();
    assert_eq!(stored.revision, 2);

    let error = match device.sync_now() {
        Ok(_) => panic!("tampered remote snapshot was accepted"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("integrity")
            || error.to_string().contains("decrypt")
            || error.to_string().contains("cryptographic"),
        "unexpected tamper rejection: {error}"
    );

    assert!(device.status().unwrap().unlocked);
    assert_eq!(fs::read(&vault_path).unwrap(), original_bytes);
    assert_eq!(device.list_items(Some("Protected Login")).unwrap().len(), 1);

    device.lock_vault().unwrap();
    device
        .unlock_vault(&vault_path, MASTER, &created.account_secret_hex)
        .unwrap();
}
