use std::{
    sync::{Arc, mpsc},
    thread,
};

use dragonforge_desktop::{DesktopService, ItemDraft};
use dragonforge_sync_server::{
    AccountRecord, AppState, InMemoryStore, SyncStore, build_router, hash_sync_token,
};
use tempfile::tempdir;
use uuid::Uuid;

const MASTER: &str = "phase-ten-test-master";

fn token() -> String {
    "10".repeat(32)
}

fn start_sync_server() -> String {
    let store = InMemoryStore::default();
    let account = AccountRecord {
        account_id: Uuid::new_v4(),
        token_hash: hash_sync_token(&token()),
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

    format!("http://{}", receiver.recv().unwrap())
}

fn login() -> ItemDraft {
    ItemDraft {
        id: None,
        name: "Recovery Login".into(),
        kind: "login".into(),
        favorite: true,
        tags: vec!["phase10".into()],
        username: "recovery@example.com".into(),
        password: "test-value".into(),
        url: "https://example.com".into(),
        notes: "Phase 10 recovery test".into(),
    }
}

#[test]
fn lost_device_recovery_restores_vault_and_rotates_trust() {
    let server = start_sync_server();
    let temp = tempdir().unwrap();
    let original_path = temp.path().join("original.dfvault");
    let recovered_path = temp.path().join("recovered.dfvault");
    let replay_path = temp.path().join("replay.dfvault");

    let original = DesktopService::default();
    let created = original.create_vault(&original_path, MASTER).unwrap();
    original.save_item(&login()).unwrap();
    original.configure_sync(&server, &token()).unwrap();
    let uploaded = original.sync_now().unwrap();
    assert_eq!(uploaded.action, "uploaded");

    let setup = original
        .configure_recovery(&created.account_secret_hex)
        .unwrap();
    assert!(setup.recovery_kit.starts_with("DFRK1:"));
    assert_eq!(setup.generation, 1);

    let old_kit = setup.recovery_kit.clone();
    let replacement = DesktopService::default();
    let result = replacement
        .recover_synced_vault(
            &recovered_path,
            &server,
            &old_kit,
            MASTER,
            Some("Replacement PC"),
        )
        .unwrap();

    assert_eq!(result.account_secret_hex, created.account_secret_hex);
    assert_ne!(result.recovery_kit, old_kit);
    assert_eq!(result.generation, 2);
    assert!(result.status.unlocked);
    assert!(result.sync_status.configured);
    assert_eq!(result.sync_status.device_name.as_deref(), Some("Replacement PC"));
    assert_eq!(replacement.list_items(Some("Recovery Login")).unwrap().len(), 1);

    let old_device_error = original.sync_now().unwrap_err().to_string();
    assert!(
        old_device_error.contains("authentication")
            || old_device_error.contains("not active"),
        "unexpected old-device failure: {old_device_error}"
    );

    let replay = DesktopService::default();
    let replay_error = replay
        .recover_synced_vault(&replay_path, &server, &old_kit, MASTER, Some("Replay PC"))
        .err()
        .expect("reusing an old recovery kit must fail")
        .to_string();
    assert!(
        replay_error.contains("rotated")
            || replay_error.contains("already used")
            || replay_error.contains("recovery"),
        "unexpected replay failure: {replay_error}"
    );
    assert!(!replay_path.exists());
}

#[test]
fn wrong_master_does_not_consume_recovery_kit() {
    let server = start_sync_server();
    let temp = tempdir().unwrap();
    let original_path = temp.path().join("master-original.dfvault");
    let failed_path = temp.path().join("wrong-master.dfvault");
    let recovered_path = temp.path().join("correct-master.dfvault");

    let original = DesktopService::default();
    let created = original.create_vault(&original_path, MASTER).unwrap();
    original.configure_sync(&server, &token()).unwrap();
    original.sync_now().unwrap();
    let setup = original
        .configure_recovery(&created.account_secret_hex)
        .unwrap();

    let replacement = DesktopService::default();
    assert!(
        replacement
            .recover_synced_vault(
                &failed_path,
                &server,
                &setup.recovery_kit,
                "incorrect-test-master",
                Some("Wrong Master PC"),
            )
            .is_err()
    );
    assert!(!failed_path.exists());

    let recovered = replacement
        .recover_synced_vault(
            &recovered_path,
            &server,
            &setup.recovery_kit,
            MASTER,
            Some("Correct Master PC"),
        )
        .unwrap();
    assert_eq!(recovered.generation, 2);
    assert!(recovered.status.unlocked);
}
