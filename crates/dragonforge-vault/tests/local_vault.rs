use std::fs;

use dragonforge_vault::{
    AccountSecret, LoginItem, PasswordPolicy, SecureNoteItem, Vault, VaultItemData, VaultItemKind,
    generate_password,
};
use tempfile::tempdir;

const MASTER: &str = "correct horse battery staple for dragonforge";

#[test]
fn create_add_lock_unlock_and_reload() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("primary.dfvault");

    let (mut vault, account_secret) = Vault::create(&path, MASTER).unwrap();
    assert!(vault.is_empty());

    let login_id = vault
        .add_login(
            "Example",
            "david@example.com",
            "never-store-this-in-plaintext",
            "https://example.com",
            "primary account",
            vec!["work".into(), "email".into()],
        )
        .unwrap();
    let note_id = vault
        .add_secure_note(
            "Recovery checklist",
            "keep this encrypted",
            vec!["recovery".into()],
        )
        .unwrap();

    assert_eq!(vault.len(), 2);
    vault.verify_integrity().unwrap();

    let locked = vault.lock();
    let vault = locked.unlock(MASTER, &account_secret).unwrap();
    assert_eq!(vault.len(), 2);

    let login = vault.get_item(&login_id).unwrap();
    assert_eq!(login.name, "Example");
    match login.data {
        VaultItemData::Login(data) => {
            assert_eq!(data.username, "david@example.com");
            assert_eq!(data.password, "never-store-this-in-plaintext");
        }
        _ => panic!("expected login"),
    }

    let note = vault.get_item(&note_id).unwrap();
    assert_eq!(note.data.kind(), VaultItemKind::SecureNote);
}

#[test]
fn sensitive_item_fields_are_not_plaintext_on_disk() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("metadata.dfvault");
    let (mut vault, _secret) = Vault::create(&path, MASTER).unwrap();

    vault
        .add_login(
            "TOP-SECRET-TITLE",
            "UNIQUE-USERNAME-493",
            "UNIQUE-PASSWORD-927",
            "https://private.example/unique-path",
            "UNIQUE-NOTE-181",
            vec!["UNIQUE-TAG-712".into()],
        )
        .unwrap();

    let disk = fs::read_to_string(path).unwrap();
    for forbidden in [
        "TOP-SECRET-TITLE",
        "UNIQUE-USERNAME-493",
        "UNIQUE-PASSWORD-927",
        "private.example",
        "UNIQUE-NOTE-181",
        "UNIQUE-TAG-712",
        MASTER,
    ] {
        assert!(!disk.contains(forbidden), "plaintext leaked: {forbidden}");
    }
}

#[test]
fn wrong_password_and_wrong_account_secret_cannot_unlock() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("unlock.dfvault");
    let (_vault, account_secret) = Vault::create(&path, MASTER).unwrap();

    assert!(Vault::open(&path, "wrong password", &account_secret).is_err());

    let wrong_secret = AccountSecret::generate().unwrap();
    assert!(Vault::open(&path, MASTER, &wrong_secret).is_err());
}

#[test]
fn local_search_decrypts_and_matches_useful_fields() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("search.dfvault");
    let (mut vault, _secret) = Vault::create(&path, MASTER).unwrap();

    vault
        .add_login(
            "Git Hosting",
            "dragon-user",
            "password",
            "https://code.example",
            "development account",
            vec!["engineering".into()],
        )
        .unwrap();
    vault
        .add_secure_note("Shopping", "buy furnace filters", vec!["home".into()])
        .unwrap();

    assert_eq!(vault.search("dragon-user").unwrap().len(), 1);
    assert_eq!(vault.search("engineering").unwrap().len(), 1);
    assert_eq!(vault.search("furnace").unwrap().len(), 1);
    assert_eq!(vault.search("not-present").unwrap().len(), 0);
    assert_eq!(vault.list().unwrap().len(), 2);
}

#[test]
fn update_rotates_record_and_delete_persists() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("mutations.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();

    let id = vault
        .add_login(
            "Old",
            "old-user",
            "old-pass",
            "https://old.example",
            "",
            vec![],
        )
        .unwrap();
    let before = fs::read_to_string(&path).unwrap();

    vault
        .update_item(
            &id,
            "New".into(),
            true,
            vec!["updated".into()],
            VaultItemData::Login(LoginItem {
                username: "new-user".into(),
                password: "new-pass".into(),
                url: "https://new.example".into(),
                notes: "changed".into(),
            }),
        )
        .unwrap();

    let after = fs::read_to_string(&path).unwrap();
    assert_ne!(before, after);

    let reopened = Vault::open(&path, MASTER, &secret).unwrap();
    let item = reopened.get_item(&id).unwrap();
    assert_eq!(item.name, "New");
    assert!(item.favorite);

    let mut reopened = reopened;
    reopened.delete_item(&id).unwrap();
    assert!(reopened.is_empty());
    assert!(Vault::open(&path, MASTER, &secret).unwrap().is_empty());
}

#[test]
fn master_password_change_rewraps_vmk_without_losing_items() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("password-change.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();
    let id = vault
        .add_secure_note("Persistent", "survives rewrap", vec![])
        .unwrap();

    vault
        .change_master_password("brand new master password", &secret)
        .unwrap();
    drop(vault);

    assert!(Vault::open(&path, MASTER, &secret).is_err());
    let reopened = Vault::open(&path, "brand new master password", &secret).unwrap();
    assert_eq!(reopened.get_item(&id).unwrap().name, "Persistent");
}

#[test]
fn encrypted_backup_can_be_imported_and_validated() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("source.dfvault");
    let backup = temp.path().join("backup.dfvault");
    let restored = temp.path().join("restored.dfvault");

    let (mut vault, secret) = Vault::create(&source, MASTER).unwrap();
    let id = vault
        .add_secure_note("Backup Test", "encrypted backup body", vec!["backup".into()])
        .unwrap();
    vault.export_backup(&backup).unwrap();

    let imported = Vault::import_backup(&backup, &restored, MASTER, &secret).unwrap();
    imported.verify_integrity().unwrap();
    assert_eq!(imported.get_item(&id).unwrap().name, "Backup Test");

    let disk = fs::read_to_string(backup).unwrap();
    assert!(!disk.contains("encrypted backup body"));
}

#[test]
fn ciphertext_tampering_is_detected() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("tamper.dfvault");
    let (mut vault, secret) = Vault::create(&path, MASTER).unwrap();
    vault
        .add_secure_note("Tamper target", "authenticated body", vec![])
        .unwrap();
    drop(vault);

    let raw = fs::read_to_string(&path).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let ciphertext = json["items"][0]["payload"]["ciphertext"]
        .as_array_mut()
        .unwrap();
    let value = ciphertext[0].as_u64().unwrap();
    ciphertext[0] = serde_json::Value::from(value ^ 0x80);
    fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

    let reopened = Vault::open(&path, MASTER, &secret).unwrap();
    assert!(reopened.verify_integrity().is_err());
}

#[test]
fn password_generator_honors_policy() {
    let password = generate_password(PasswordPolicy {
        length: 32,
        lowercase: true,
        uppercase: true,
        digits: true,
        symbols: true,
    })
    .unwrap();

    assert_eq!(password.len(), 32);
    assert!(password.bytes().any(|value| value.is_ascii_lowercase()));
    assert!(password.bytes().any(|value| value.is_ascii_uppercase()));
    assert!(password.bytes().any(|value| value.is_ascii_digit()));
    assert!(password.bytes().any(|value| !value.is_ascii_alphanumeric()));
}

#[test]
fn item_types_round_trip() {
    let login = VaultItemData::Login(LoginItem {
        username: "user".into(),
        password: "pass".into(),
        url: "https://example.com".into(),
        notes: "notes".into(),
    });
    let note = VaultItemData::SecureNote(SecureNoteItem {
        notes: "secret".into(),
    });

    assert_eq!(login.kind(), VaultItemKind::Login);
    assert_eq!(note.kind(), VaultItemKind::SecureNote);
}
