#![cfg(target_os = "windows")]

use std::fs;

use dragonforge_desktop::DesktopService;
use tempfile::tempdir;

#[test]
fn windows_credential_manager_protects_sync_token_and_device_seed() {
    let directory = tempdir().unwrap();
    let vault_path = directory.path().join("credential-store-test.dfvault");
    let service = DesktopService::default();

    service
        .create_vault(&vault_path, "Phase11-Test-Master-Password!")
        .unwrap();
    let status = service
        .configure_sync("http://127.0.0.1:8787", &"a".repeat(64))
        .unwrap();

    assert_eq!(status.secret_storage, "windowsCredentialManager");
    assert!(status.device_id.is_some());

    let sidecar_path = format!("{}.sync.json", vault_path.display());
    let sidecar: serde_json::Value =
        serde_json::from_slice(&fs::read(&sidecar_path).unwrap()).unwrap();

    assert_eq!(sidecar["version"], 3);
    assert!(sidecar.get("syncToken").is_none());
    assert!(sidecar.get("deviceSigningSeedHex").is_none());
    assert!(sidecar["credentialId"].as_str().is_some());

    let reloaded = service.sync_status().unwrap();
    assert!(reloaded.configured);
    assert_eq!(reloaded.secret_storage, "windowsCredentialManager");
    assert_eq!(reloaded.device_id, status.device_id);

    let removed = service.remove_sync().unwrap();
    assert!(!removed.configured);
    assert!(!std::path::Path::new(&sidecar_path).exists());
}
