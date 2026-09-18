use std::path::PathBuf;

use rfd::FileDialog;
use tauri::State;
use zeroize::Zeroize;

use crate::{
    AppStatus, CreateVaultResponse, DesktopService, ItemDraft, ItemDto, ItemSummaryDto,
    SyncOutcome, SyncStatus,
};

fn error_message(error: impl core::fmt::Display) -> String {
    error.to_string()
}

#[tauri::command]
pub fn app_status(service: State<'_, DesktopService>) -> Result<AppStatus, String> {
    service.status().map_err(error_message)
}

#[tauri::command]
pub fn create_vault(
    service: State<'_, DesktopService>,
    path: String,
    mut master_password: String,
) -> Result<CreateVaultResponse, String> {
    let result = service
        .create_vault(PathBuf::from(path), &master_password)
        .map_err(error_message);
    master_password.zeroize();
    result
}

#[tauri::command]
pub fn unlock_vault(
    service: State<'_, DesktopService>,
    path: String,
    mut master_password: String,
    mut account_secret_hex: String,
) -> Result<AppStatus, String> {
    let result = service
        .unlock_vault(PathBuf::from(path), &master_password, &account_secret_hex)
        .map_err(error_message);
    master_password.zeroize();
    account_secret_hex.zeroize();
    result
}

#[tauri::command]
pub fn lock_vault(service: State<'_, DesktopService>) -> Result<AppStatus, String> {
    service.lock_vault().map_err(error_message)
}

#[tauri::command]
pub fn list_items(
    service: State<'_, DesktopService>,
    query: Option<String>,
) -> Result<Vec<ItemSummaryDto>, String> {
    service.list_items(query.as_deref()).map_err(error_message)
}

#[tauri::command]
pub fn get_item(service: State<'_, DesktopService>, id: String) -> Result<ItemDto, String> {
    service.get_item(&id).map_err(error_message)
}

#[tauri::command]
pub fn save_item(
    service: State<'_, DesktopService>,
    mut draft: ItemDraft,
) -> Result<String, String> {
    let result = service.save_item(&draft).map_err(error_message);
    draft.zeroize();
    result
}

#[tauri::command]
pub fn delete_item(service: State<'_, DesktopService>, id: String) -> Result<(), String> {
    service.delete_item(&id).map_err(error_message)
}

#[tauri::command]
pub fn generate_password(
    service: State<'_, DesktopService>,
    length: usize,
) -> Result<String, String> {
    service.generate_password(length).map_err(error_message)
}

#[tauri::command]
pub fn change_master_password(
    service: State<'_, DesktopService>,
    mut new_master_password: String,
    mut account_secret_hex: String,
) -> Result<(), String> {
    let result = service
        .change_master_password(&new_master_password, &account_secret_hex)
        .map_err(error_message);
    new_master_password.zeroize();
    account_secret_hex.zeroize();
    result
}

#[tauri::command]
pub fn verify_vault(service: State<'_, DesktopService>) -> Result<(), String> {
    service.verify_vault().map_err(error_message)
}

#[tauri::command]
pub fn export_backup(
    service: State<'_, DesktopService>,
    destination: String,
) -> Result<(), String> {
    service
        .export_backup(PathBuf::from(destination))
        .map_err(error_message)
}

#[tauri::command]
pub fn pick_existing_vault() -> Option<String> {
    FileDialog::new()
        .add_filter("DragonForge Vault", &["dfvault"])
        .pick_file()
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub fn pick_new_vault() -> Option<String> {
    FileDialog::new()
        .add_filter("DragonForge Vault", &["dfvault"])
        .set_file_name("my-vault.dfvault")
        .save_file()
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub fn pick_backup_destination() -> Option<String> {
    FileDialog::new()
        .add_filter("DragonForge Vault Backup", &["dfvault"])
        .set_file_name("dragonforge-backup.dfvault")
        .save_file()
        .map(|path| path.display().to_string())
}


#[tauri::command]
pub fn sync_status(service: State<'_, DesktopService>) -> Result<SyncStatus, String> {
    service.sync_status().map_err(error_message)
}

#[tauri::command]
pub fn configure_sync(
    service: State<'_, DesktopService>,
    server_url: String,
    mut sync_token: String,
) -> Result<SyncStatus, String> {
    let result = service
        .configure_sync(&server_url, &sync_token)
        .map_err(error_message);
    sync_token.zeroize();
    result
}

#[tauri::command]
pub async fn sync_now(service: State<'_, DesktopService>) -> Result<SyncOutcome, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.sync_now().map_err(error_message))
        .await
        .map_err(error_message)?
}

#[tauri::command]
pub async fn resolve_sync_conflict(
    service: State<'_, DesktopService>,
    strategy: String,
) -> Result<SyncOutcome, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service
            .resolve_sync_conflict(&strategy)
            .map_err(error_message)
    })
    .await
    .map_err(error_message)?
}

#[tauri::command]
pub fn remove_sync(service: State<'_, DesktopService>) -> Result<SyncStatus, String> {
    service.remove_sync().map_err(error_message)
}
