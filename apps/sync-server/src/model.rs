use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SYNC_PROTOCOL_VERSION: u16 = 1;
pub const MAX_SYNC_BLOB_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct AccountRecord {
    pub account_id: Uuid,
    pub token_hash: [u8; 32],
}

#[derive(Clone)]
pub struct StoredVault {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub revision: u64,
    pub content_sha256: [u8; 32],
    pub ciphertext: Vec<u8>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    pub protocol_version: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub account_id: Uuid,
    pub sync_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncMetadata {
    pub vault_id: Uuid,
    pub revision: u64,
    pub content_sha256: String,
    pub updated_at_ms: u64,
}
