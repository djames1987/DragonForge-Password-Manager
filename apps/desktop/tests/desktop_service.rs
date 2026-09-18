use dragonforge_desktop::{DesktopService, ItemDraft};
use tempfile::tempdir;

const MASTER: &str = "phase-five-desktop-test-master";

#[test]
fn desktop_service_create_lock_unlock_and_crud() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("desktop.dfvault");
    let service = DesktopService::default();

    let created = service.create_vault(&path, MASTER).unwrap();
    assert!(created.status.unlocked);
    assert_eq!(created.status.item_count, 0);
    assert_eq!(created.account_secret_hex.len(), 64);

    let id = service
        .save_item(&ItemDraft {
            id: None,
            name: "Example Login".into(),
            kind: "login".into(),
            favorite: true,
            tags: vec!["work".into(), " Work ".into()],
            username: "user@example.com".into(),
            password: "desktop-test-password".into(),
            url: "https://example.com".into(),
            notes: "desktop integration".into(),
        })
        .unwrap();

    let summaries = service.list_items(None).unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, id);
    assert!(summaries[0].favorite);

    let item = service.get_item(&id).unwrap();
    assert_eq!(item.name, "Example Login");
    assert_eq!(item.password, "desktop-test-password");

    service.verify_vault().unwrap();
    service.lock_vault().unwrap();
    assert!(service.get_item(&id).is_err());

    let status = service
        .unlock_vault(&path, MASTER, &created.account_secret_hex)
        .unwrap();
    assert!(status.unlocked);
    assert_eq!(status.item_count, 1);

    service.delete_item(&id).unwrap();
    assert!(service.list_items(None).unwrap().is_empty());
}

#[test]
fn desktop_service_search_and_note_round_trip() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("search.dfvault");
    let service = DesktopService::default();
    service.create_vault(&path, MASTER).unwrap();

    service
        .save_item(&ItemDraft {
            id: None,
            name: "Router Recovery".into(),
            kind: "secure_note".into(),
            favorite: false,
            tags: vec!["network".into()],
            username: String::new(),
            password: String::new(),
            url: String::new(),
            notes: "replace the lab router after testing".into(),
        })
        .unwrap();

    assert_eq!(service.list_items(Some("router")).unwrap().len(), 1);
    assert_eq!(service.list_items(Some("missing")).unwrap().len(), 0);
}

#[test]
fn password_generator_and_backup_are_available_from_desktop_service() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("primary.dfvault");
    let backup = temp.path().join("backup.dfvault");
    let service = DesktopService::default();
    service.create_vault(&path, MASTER).unwrap();

    let generated = service.generate_password(28).unwrap();
    assert_eq!(generated.len(), 28);

    service.export_backup(&backup).unwrap();
    assert!(backup.exists());
}

#[test]
fn invalid_item_drafts_are_rejected() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("invalid.dfvault");
    let service = DesktopService::default();
    service.create_vault(&path, MASTER).unwrap();

    let result = service.save_item(&ItemDraft {
        id: None,
        name: " ".into(),
        kind: "login".into(),
        favorite: false,
        tags: vec![],
        username: String::new(),
        password: String::new(),
        url: String::new(),
        notes: String::new(),
    });

    assert!(result.is_err());
}
