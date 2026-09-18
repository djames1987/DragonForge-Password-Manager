use std::fs;

use dragonforge_vault::{
    MigrationStatus, Vault, VaultError, CURRENT_VAULT_FORMAT_VERSION, inspect_vault_file,
};
use tempfile::tempdir;

const MASTER: &str = "phase-four-hardening-master-password";

fn load_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn format_inspection_reports_current_version_without_unlocking() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("inspect.dfvault");
    let (mut vault, _secret) = Vault::create(&path, MASTER).unwrap();
    vault
        .add_secure_note("inspect me", "encrypted", vec!["phase4".into()])
        .unwrap();

    let info = inspect_vault_file(&path).unwrap();
    assert_eq!(info.version, CURRENT_VAULT_FORMAT_VERSION);
    assert_eq!(info.item_count, 1);
    assert!(info.file_bytes > 0);
    assert_eq!(info.migration, MigrationStatus::Current);
}

#[test]
fn malformed_structure_is_rejected_before_unlock_work() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("mutations.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();
    vault
        .add_secure_note("one", "first", vec![])
        .unwrap();
    vault
        .add_secure_note("two", "second", vec![])
        .unwrap();
    drop(vault);

    let original = load_json(&path);

    let mut future = original.clone();
    future["version"] = serde_json::Value::from(CURRENT_VAULT_FORMAT_VERSION + 1);
    write_json(&path, &future);
    assert!(matches!(
        Vault::open(&path, MASTER, &secret),
        Err(VaultError::UnsupportedFormatVersion(_))
    ));

    let mut bad_uuid = original.clone();
    bad_uuid["vault_id"] = serde_json::Value::from("not-a-uuid");
    write_json(&path, &bad_uuid);
    assert!(matches!(
        Vault::open(&path, MASTER, &secret),
        Err(VaultError::InvalidStructure(_))
    ));

    let mut revision_zero = original.clone();
    revision_zero["items"][0]["revision"] = serde_json::Value::from(0_u64);
    write_json(&path, &revision_zero);
    assert!(matches!(
        Vault::open(&path, MASTER, &secret),
        Err(VaultError::InvalidStructure(_))
    ));

    let mut duplicate = original.clone();
    let first_id = duplicate["items"][0]["id"].clone();
    duplicate["items"][1]["id"] = first_id;
    write_json(&path, &duplicate);
    assert!(matches!(
        Vault::open(&path, MASTER, &secret),
        Err(VaultError::InvalidStructure(_))
    ));

    let mut hostile_kdf = original.clone();
    hostile_kdf["kdf"]["memory_kib"] = serde_json::Value::from(u32::MAX);
    write_json(&path, &hostile_kdf);
    assert!(matches!(
        Vault::open(&path, MASTER, &secret),
        Err(VaultError::ResourceLimit(_))
    ));

    write_json(&path, &original);
    Vault::open(&path, MASTER, &secret)
        .unwrap()
        .verify_integrity()
        .unwrap();
}

#[test]
fn missing_live_file_recovers_from_last_complete_backup() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("recovery.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();
    let id = vault
        .add_secure_note("recoverable", "backup body", vec![])
        .unwrap();
    drop(vault);

    let backup = path.with_extension("dfvault.bak");
    fs::rename(&path, &backup).unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    let recovered = Vault::open(&path, MASTER, &secret).unwrap();
    assert_eq!(recovered.get_item(&id).unwrap().name, "recoverable");
    assert!(path.exists());
    assert!(!backup.exists());
}

#[test]
fn orphan_temporary_file_is_never_treated_as_a_valid_vault() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("orphan.dfvault");
    let tmp = path.with_extension("dfvault.tmp");
    fs::write(&tmp, b"{\"partial\": true").unwrap();

    assert!(Vault::open(&path, MASTER, &dragonforge_vault::AccountSecret::generate().unwrap()).is_err());
    assert!(!path.exists());
    assert!(tmp.exists());
}

#[test]
fn medium_vault_stress_round_trip_and_search() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("stress.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();

    for index in 0..100 {
        vault
            .add_login(
                format!("Service {index:03}"),
                format!("user-{index}@example.test"),
                format!("generated-password-{index}-not-real"),
                format!("https://service-{index}.example.test"),
                format!("stress note {index}"),
                vec!["stress".into(), format!("batch-{}", index / 10)],
            )
            .unwrap();
    }

    assert_eq!(vault.len(), 100);
    vault.verify_integrity().unwrap();
    drop(vault);

    let reopened = Vault::open(&path, MASTER, &secret).unwrap();
    assert_eq!(reopened.len(), 100);
    assert_eq!(reopened.search("Service 042").unwrap().len(), 1);
    assert_eq!(reopened.search("batch-7").unwrap().len(), 10);
    reopened.verify_integrity().unwrap();
}

#[test]
fn repeated_updates_remain_decryptable() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("revisions.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();
    let id = vault
        .add_secure_note("revision-test", "version-0", vec![])
        .unwrap();

    for revision in 1..=50 {
        vault
            .update_item(
                &id,
                "revision-test".into(),
                revision % 2 == 0,
                vec!["updates".into()],
                dragonforge_vault::VaultItemData::SecureNote(
                    dragonforge_vault::SecureNoteItem {
                        notes: format!("version-{revision}"),
                    },
                ),
            )
            .unwrap();
    }
    drop(vault);

    let reopened = Vault::open(&path, MASTER, &secret).unwrap();
    let item = reopened.get_item(&id).unwrap();
    match item.data {
        dragonforge_vault::VaultItemData::SecureNote(note) => {
            assert_eq!(note.notes, "version-50");
        }
        _ => panic!("expected secure note"),
    }
    reopened.verify_integrity().unwrap();
}
