use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Result, VaultError, storage::read_file};

pub const CURRENT_VAULT_FORMAT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultFormatInfo {
    pub version: u16,
    pub item_count: usize,
    pub file_bytes: u64,
    pub migration: MigrationStatus,
}

#[derive(Debug, Deserialize, Serialize)]
struct FormatProbe {
    version: u16,
    items: Vec<serde_json::Value>,
}

pub fn inspect_vault_file(path: impl AsRef<Path>) -> Result<VaultFormatInfo> {
    let path = path.as_ref();
    let bytes = read_file(path)?;
    let probe: FormatProbe = serde_json::from_slice(&bytes)?;

    if probe.version != CURRENT_VAULT_FORMAT_VERSION {
        return Err(VaultError::UnsupportedFormatVersion(probe.version));
    }

    Ok(VaultFormatInfo {
        version: probe.version,
        item_count: probe.items.len(),
        file_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        migration: MigrationStatus::Current,
    })
}
