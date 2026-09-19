#[cfg(target_os = "windows")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(target_os = "windows")]
pub(crate) const WINDOWS_STORAGE_LABEL: &str = "windowsCredentialManager";
#[cfg(not(target_os = "windows"))]
pub(crate) const LEGACY_STORAGE_LABEL: &str = "legacySidecar";

#[cfg(target_os = "windows")]
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncSecretBundle {
    pub sync_token: String,
    pub device_signing_seed_hex: Option<String>,
}

#[cfg(target_os = "windows")]
const SERVICE_NAME: &str = "DragonForge Password Manager";

#[cfg(target_os = "windows")]
fn entry(credential_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE_NAME, credential_id)
        .map_err(|error| format!("Windows Credential Manager entry could not be opened: {error}"))
}

#[cfg(target_os = "windows")]
pub(crate) fn store(credential_id: &str, bundle: &SyncSecretBundle) -> Result<(), String> {
    let entry = entry(credential_id)?;
    let mut serialized = serde_json::to_string(bundle)
        .map_err(|error| format!("sync secret bundle could not be serialized: {error}"))?;
    let result = entry
        .set_password(&serialized)
        .map_err(|error| format!("Windows Credential Manager rejected sync secrets: {error}"));
    serialized.zeroize();
    result
}

#[cfg(target_os = "windows")]
pub(crate) fn load(credential_id: &str) -> Result<SyncSecretBundle, String> {
    let entry = entry(credential_id)?;
    let mut serialized = entry
        .get_password()
        .map_err(|error| format!("Windows Credential Manager sync secrets are unavailable: {error}"))?;
    let parsed = serde_json::from_str(&serialized)
        .map_err(|error| format!("Windows Credential Manager sync secrets are malformed: {error}"));
    serialized.zeroize();
    parsed
}

#[cfg(target_os = "windows")]
pub(crate) fn delete(credential_id: &str) -> Result<(), String> {
    let entry = entry(credential_id)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Windows Credential Manager sync secrets could not be removed: {error}"
        )),
    }
}

pub(crate) const fn storage_label() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        WINDOWS_STORAGE_LABEL
    }
    #[cfg(not(target_os = "windows"))]
    {
        LEGACY_STORAGE_LABEL
    }
}
