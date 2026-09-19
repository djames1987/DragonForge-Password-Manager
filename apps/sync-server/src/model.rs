use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const SYNC_PROTOCOL_VERSION: u16 = 2;
pub const MAX_SYNC_BLOB_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RECOVERY_ENVELOPE_BYTES: usize = 16 * 1024;

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

#[derive(Clone)]
pub struct RecoveryRecord {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub verifying_key: Vec<u8>,
    pub envelope: Vec<u8>,
    pub generation: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    pub protocol_version: u16,
}

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    #[zeroize(skip)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeviceStatus {
    Pending,
    Active,
    Revoked,
}

#[derive(Clone)]
pub struct DeviceRecord {
    pub account_id: Uuid,
    pub device_id: Uuid,
    pub name: String,
    pub verifying_key: Vec<u8>,
    pub status: DeviceStatus,
    pub created_at_ms: u64,
    pub approved_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub device_id: Uuid,
    pub name: String,
    pub status: DeviceStatus,
    pub created_at_ms: u64,
    pub approved_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
}

impl From<&DeviceRecord> for DeviceSummary {
    fn from(value: &DeviceRecord) -> Self {
        Self {
            device_id: value.device_id,
            name: value.name.clone(),
            status: value.status,
            created_at_ms: value.created_at_ms,
            approved_at_ms: value.approved_at_ms,
            revoked_at_ms: value.revoked_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollDeviceRequest {
    pub device_id: Uuid,
    pub name: String,
    pub verifying_key_hex: String,
    pub proof_signature_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollDeviceResponse {
    pub device: DeviceSummary,
    pub first_device: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDecisionRequest {
    pub approver_device_id: Uuid,
    pub signature_hex: String,
}
