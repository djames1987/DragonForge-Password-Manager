#![forbid(unsafe_code)]

mod bridge;
mod commands;
mod service;

pub use bridge::{
    BROWSER_PROTOCOL_VERSION, BrowserAction, BrowserBridge, BrowserRequest, BrowserResponse,
    NATIVE_HOST_NAME, forward_native_request,
};
pub use service::{
    AppStatus, BrowserCredential, BrowserLoginSummary, CreateVaultResponse, DesktopError,
    DesktopService, ItemDraft, ItemDto, ItemSummaryDto,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let service = DesktopService::default();
    let browser_bridge = BrowserBridge::start(service.clone()).ok();

    tauri::Builder::default()
        .manage(service)
        .manage(browser_bridge)
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::create_vault,
            commands::unlock_vault,
            commands::lock_vault,
            commands::list_items,
            commands::get_item,
            commands::save_item,
            commands::delete_item,
            commands::generate_password,
            commands::change_master_password,
            commands::verify_vault,
            commands::export_backup,
            commands::pick_existing_vault,
            commands::pick_new_vault,
            commands::pick_backup_destination,
        ])
        .run(tauri::generate_context!())
        .expect("DragonForge desktop runtime failed");
}
