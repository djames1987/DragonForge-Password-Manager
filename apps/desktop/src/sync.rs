use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dragonforge_crypto::MlDsa65KeyPair;
use dragonforge_vault::{MAX_VAULT_FILE_BYTES, validate_encrypted_vault_bytes};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{AUTHORIZATION, HeaderValue},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::secret_store::{self, SyncSecretBundle};

const SYNC_CONFIG_VERSION: u16 = 3;
const HEADER_BASE_REVISION: &str = "x-dragonforge-base-revision";
const HEADER_REVISION: &str = "x-dragonforge-revision";
const HEADER_CONTENT_SHA256: &str = "x-dragonforge-content-sha256";
const HEADER_DEVICE_ID: &str = "x-dragonforge-device-id";
const HEADER_DEVICE_TIMESTAMP: &str = "x-dragonforge-device-timestamp";
const HEADER_DEVICE_SIGNATURE: &str = "x-dragonforge-device-signature";

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("sync is not configured for this vault")]
    NotConfigured,
    #[error("invalid sync configuration: {0}")]
    InvalidConfig(String),
    #[error("sync server error: {0}")]
    Transport(String),
    #[error("sync server rejected authentication")]
    Unauthorized,
    #[error("this device is not active; approve it from an already-authorized device")]
    DeviceNotActive,
    #[error("sync server returned malformed metadata")]
    InvalidResponse,
    #[error("sync state could not be read or written: {0}")]
    Io(String),
    #[error("remote vault failed structural validation: {0}")]
    InvalidRemoteVault(String),
    #[error("secure local credential storage failed: {0}")]
    SecretStorage(String),
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub configured: bool,
    pub server_url: Option<String>,
    pub last_revision: u64,
    pub last_content_sha256: Option<String>,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub secret_storage: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub action: String,
    pub revision: u64,
    pub message: String,
    pub vault_locked: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub device_id: String,
    pub name: String,
    pub status: String,
    pub created_at_ms: u64,
    pub approved_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
}

#[derive(Zeroize, ZeroizeOnDrop)]
struct SyncConfig {
    #[zeroize(skip)]
    version: u16,
    #[zeroize(skip)]
    server_url: String,
    sync_token: String,
    #[zeroize(skip)]
    last_revision: u64,
    #[zeroize(skip)]
    last_content_sha256: Option<String>,
    #[zeroize(skip)]
    device_id: Option<String>,
    #[zeroize(skip)]
    device_name: Option<String>,
    device_signing_seed_hex: Option<String>,
    #[zeroize(skip)]
    credential_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncConfigDisk {
    version: u16,
    server_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sync_token: Option<String>,
    last_revision: u64,
    last_content_sha256: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    device_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    device_signing_seed_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_id: Option<String>,
}

pub(crate) struct PullPayload {
    pub bytes: Vec<u8>,
    pub revision: u64,
    pub content_sha256: String,
}

pub(crate) struct SyncExecution {
    pub outcome: SyncOutcome,
    pub pull: Option<PullPayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerDeviceSummary {
    device_id: Uuid,
    name: String,
    status: ServerDeviceStatus,
    created_at_ms: u64,
    approved_at_ms: Option<u64>,
    revoked_at_ms: Option<u64>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ServerDeviceStatus {
    Pending,
    Active,
    Revoked,
}

impl ServerDeviceStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrollResponse {
    device: ServerDeviceSummary,
    first_device: bool,
}

pub(crate) fn configure(
    vault_path: &Path,
    server_url: &str,
    sync_token: &str,
) -> Result<SyncStatus, SyncError> {
    validate_server_url(server_url)?;
    validate_token(sync_token)?;

    let normalized_url = server_url.trim_end_matches('/').to_owned();
    let existing = load_config(vault_path)?;
    let old_credential_id = existing
        .as_ref()
        .and_then(|config| config.credential_id.clone());
    let mut config = match existing {
        Some(mut config)
            if config.server_url == normalized_url && config.sync_token == sync_token =>
        {
            config.version = SYNC_CONFIG_VERSION;
            config
        }
        _ => SyncConfig {
            version: SYNC_CONFIG_VERSION,
            server_url: normalized_url,
            sync_token: sync_token.to_owned(),
            last_revision: 0,
            last_content_sha256: None,
            device_id: None,
            device_name: None,
            device_signing_seed_hex: None,
            credential_id: new_credential_id(),
        },
    };
    ensure_device_identity(&mut config, None)?;
    ensure_credential_id(&mut config);
    save_config(vault_path, &config)?;
    cleanup_replaced_credential(old_credential_id.as_deref(), config.credential_id.as_deref())?;
    Ok(status_from_config(&config))
}

pub(crate) fn remove(vault_path: &Path) -> Result<SyncStatus, SyncError> {
    let credential_id = load_config(vault_path)?
        .and_then(|config| config.credential_id.clone());
    #[cfg(target_os = "windows")]
    if let Some(credential_id) = credential_id.as_deref() {
        secret_store::delete(credential_id).map_err(SyncError::SecretStorage)?;
    }

    let path = config_path(vault_path);
    for candidate in [
        path.clone(),
        path.with_extension("json.tmp"),
        path.with_extension("json.bak"),
    ] {
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(SyncError::Io(error.to_string())),
        }
    }
    Ok(empty_status())
}

pub(crate) fn status(vault_path: &Path) -> Result<SyncStatus, SyncError> {
    match load_config(vault_path)? {
        Some(config) => Ok(status_from_config(&config)),
        None => Ok(empty_status()),
    }
}

pub(crate) fn enroll_device(
    vault_path: &Path,
    preferred_name: Option<&str>,
) -> Result<DeviceSummary, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_device_identity(&mut config, preferred_name)?;
    save_config(vault_path, &config)?;
    let response = post_enrollment(&config)?;
    if response.first_device && !matches!(response.device.status, ServerDeviceStatus::Active) {
        return Err(SyncError::InvalidResponse);
    }
    Ok(device_summary(response.device))
}

pub(crate) fn own_device_status(vault_path: &Path) -> Result<DeviceSummary, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_device_identity(&mut config, None)?;
    save_config(vault_path, &config)?;
    let device_id = device_id(&config)?;
    let response = client()?
        .get(format!("{}/v1/devices/{device_id}", config.server_url))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(SyncError::DeviceNotActive);
    }
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SyncError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {}",
            response.status()
        )));
    }
    let device: ServerDeviceSummary = response.json().map_err(|_| SyncError::InvalidResponse)?;
    save_config(vault_path, &config)?;
    Ok(device_summary(device))
}

pub(crate) fn list_devices(vault_path: &Path) -> Result<Vec<DeviceSummary>, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_active_device(vault_path, &mut config)?;
    let path = "/v1/devices";
    let timestamp = now_seconds()?;
    let signature = sign_request(&config, "GET", path, timestamp, &[], None)?;
    let response = client()?
        .get(format!("{}{}", config.server_url, path))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .header(HEADER_DEVICE_ID, device_id(&config)?.to_string())
        .header(HEADER_DEVICE_TIMESTAMP, timestamp.to_string())
        .header(HEADER_DEVICE_SIGNATURE, signature)
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    map_auth_status(&response)?;
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {}",
            response.status()
        )));
    }
    let devices: Vec<ServerDeviceSummary> =
        response.json().map_err(|_| SyncError::InvalidResponse)?;
    Ok(devices.into_iter().map(device_summary).collect())
}

pub(crate) fn approve_device(
    vault_path: &Path,
    target_device_id: &str,
) -> Result<DeviceSummary, SyncError> {
    device_decision(vault_path, target_device_id, "approve")
}

pub(crate) fn revoke_device(
    vault_path: &Path,
    target_device_id: &str,
) -> Result<DeviceSummary, SyncError> {
    device_decision(vault_path, target_device_id, "revoke")
}

pub(crate) fn sync(vault_path: &Path, vault_id: &str) -> Result<SyncExecution, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_active_device(vault_path, &mut config)?;
    let local = read_vault_bytes(vault_path)?;
    let local_hash = sha256_hex(&local);
    let remote = fetch_remote(&config, vault_id)?;

    match remote {
        None if config.last_revision == 0 && config.last_content_sha256.is_none() => {
            let metadata = upload(&config, vault_id, 0, &local)?;
            config.last_revision = metadata.revision;
            config.last_content_sha256 = Some(metadata.content_sha256.clone());
            save_config(vault_path, &config)?;
            Ok(SyncExecution {
                outcome: SyncOutcome {
                    action: "uploaded".to_owned(),
                    revision: metadata.revision,
                    message: "Initial encrypted vault state uploaded.".to_owned(),
                    vault_locked: false,
                },
                pull: None,
            })
        }
        None => Ok(conflict_outcome(
            "remoteMissing",
            config.last_revision,
            "The remote vault disappeared after a previous sync; no data was changed.",
        )),
        Some(remote) if config.last_revision == 0 && config.last_content_sha256.is_none() => {
            Ok(conflict_outcome(
                "initialConflict",
                remote.revision,
                "The server already contains this vault. Choose whether to keep the local or remote encrypted copy.",
            ))
        }
        Some(remote) => {
            let last_hash = config
                .last_content_sha256
                .as_deref()
                .ok_or_else(|| SyncError::InvalidConfig("missing last content hash".to_owned()))?;

            if remote.revision < config.last_revision {
                return Ok(conflict_outcome(
                    "remoteRollback",
                    remote.revision,
                    "The server revision is older than the last synchronized revision. Sync was stopped.",
                ));
            }

            if remote.revision == config.last_revision && remote.content_sha256 != last_hash {
                return Ok(conflict_outcome(
                    "remoteMismatch",
                    remote.revision,
                    "The remote ciphertext changed without a revision change. Sync was stopped.",
                ));
            }

            let local_changed = local_hash != last_hash;
            let remote_changed = remote.revision > config.last_revision;

            match (local_changed, remote_changed) {
                (false, false) => Ok(SyncExecution {
                    outcome: SyncOutcome {
                        action: "upToDate".to_owned(),
                        revision: config.last_revision,
                        message: "Vault is already synchronized.".to_owned(),
                        vault_locked: false,
                    },
                    pull: None,
                }),
                (true, false) => {
                    let metadata = upload(&config, vault_id, config.last_revision, &local)?;
                    config.last_revision = metadata.revision;
                    config.last_content_sha256 = Some(metadata.content_sha256.clone());
                    save_config(vault_path, &config)?;
                    Ok(SyncExecution {
                        outcome: SyncOutcome {
                            action: "uploaded".to_owned(),
                            revision: metadata.revision,
                            message: "Local encrypted changes uploaded.".to_owned(),
                            vault_locked: false,
                        },
                        pull: None,
                    })
                }
                (false, true) => {
                    validate_remote_bytes(&remote.bytes, vault_id)?;
                    Ok(SyncExecution {
                        outcome: SyncOutcome {
                            action: "downloaded".to_owned(),
                            revision: remote.revision,
                            message: "Newer remote encrypted vault downloaded. The vault was locked and must be unlocked again.".to_owned(),
                            vault_locked: true,
                        },
                        pull: Some(PullPayload {
                            bytes: remote.bytes,
                            revision: remote.revision,
                            content_sha256: remote.content_sha256,
                        }),
                    })
                }
                (true, true) => Ok(conflict_outcome(
                    "conflict",
                    remote.revision,
                    "Both the local vault and remote vault changed since the last sync. Choose a conflict resolution.",
                )),
            }
        }
    }
}

pub(crate) fn resolve(
    vault_path: &Path,
    vault_id: &str,
    strategy: &str,
) -> Result<SyncExecution, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_active_device(vault_path, &mut config)?;
    match strategy {
        "keepLocal" => {
            let local = read_vault_bytes(vault_path)?;
            let current_remote_revision =
                fetch_remote(&config, vault_id)?.map_or(0, |remote| remote.revision);
            let metadata = upload(&config, vault_id, current_remote_revision, &local)?;
            config.last_revision = metadata.revision;
            config.last_content_sha256 = Some(metadata.content_sha256.clone());
            save_config(vault_path, &config)?;
            Ok(SyncExecution {
                outcome: SyncOutcome {
                    action: "keptLocal".to_owned(),
                    revision: metadata.revision,
                    message: "Local encrypted vault was explicitly chosen and uploaded.".to_owned(),
                    vault_locked: false,
                },
                pull: None,
            })
        }
        "keepRemote" => {
            let remote = fetch_remote(&config, vault_id)?
                .ok_or_else(|| SyncError::Transport("remote vault does not exist".to_owned()))?;
            validate_remote_bytes(&remote.bytes, vault_id)?;
            Ok(SyncExecution {
                outcome: SyncOutcome {
                    action: "keptRemote".to_owned(),
                    revision: remote.revision,
                    message: "Remote encrypted vault was explicitly chosen. The local vault was locked and replaced.".to_owned(),
                    vault_locked: true,
                },
                pull: Some(PullPayload {
                    bytes: remote.bytes,
                    revision: remote.revision,
                    content_sha256: remote.content_sha256,
                }),
            })
        }
        _ => Err(SyncError::InvalidConfig(
            "conflict strategy must be keepLocal or keepRemote".to_owned(),
        )),
    }
}

pub(crate) fn commit_pull(
    vault_path: &Path,
    vault_id: &str,
    pull: PullPayload,
) -> Result<(), SyncError> {
    validate_remote_bytes(&pull.bytes, vault_id)?;
    replace_file_atomically(vault_path, &pull.bytes)?;
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    config.last_revision = pull.revision;
    config.last_content_sha256 = Some(pull.content_sha256);
    save_config(vault_path, &config)
}

fn ensure_active_device(vault_path: &Path, config: &mut SyncConfig) -> Result<(), SyncError> {
    ensure_device_identity(config, None)?;
    save_config(vault_path, config)?;
    let enrollment = post_enrollment(config)?;
    match enrollment.device.status {
        ServerDeviceStatus::Active => Ok(()),
        ServerDeviceStatus::Pending | ServerDeviceStatus::Revoked => {
            Err(SyncError::DeviceNotActive)
        }
    }
}

fn ensure_device_identity(
    config: &mut SyncConfig,
    preferred_name: Option<&str>,
) -> Result<(), SyncError> {
    ensure_credential_id(config);
    if config.device_id.is_some() && config.device_signing_seed_hex.is_some() {
        if let Some(name) = preferred_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            config.device_name = Some(validate_device_name(name)?.to_owned());
        }
        config.version = SYNC_CONFIG_VERSION;
        return Ok(());
    }

    let device_id = Uuid::new_v4();
    let key_pair = MlDsa65KeyPair::generate();
    let seed = key_pair.export_seed();
    let default_name = format!("DragonForge Desktop {}", &device_id.to_string()[..8]);
    let name = preferred_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&default_name);
    validate_device_name(name)?;

    config.version = SYNC_CONFIG_VERSION;
    config.device_id = Some(device_id.to_string());
    config.device_name = Some(name.to_owned());
    config.device_signing_seed_hex = Some(hex::encode(seed.as_slice()));
    Ok(())
}

fn post_enrollment(config: &SyncConfig) -> Result<EnrollResponse, SyncError> {
    let key_pair = device_key_pair(config)?;
    let device_id = device_id(config)?;
    let name = config
        .device_name
        .as_deref()
        .ok_or_else(|| SyncError::InvalidConfig("missing device name".to_owned()))?;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Request<'a> {
        device_id: Uuid,
        name: &'a str,
        verifying_key_hex: String,
        proof_signature_hex: String,
    }

    let verifying_key_hex = hex::encode(key_pair.verifying_key().as_bytes());
    let proof_signature_hex = hex::encode(key_pair.sign(&device_enrollment_message(
        device_id,
        name,
        &verifying_key_hex,
    )));

    let response = client()?
        .post(format!("{}/v1/devices/enroll", config.server_url))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .json(&Request {
            device_id,
            name,
            verifying_key_hex,
            proof_signature_hex,
        })
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SyncError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {} during device enrollment",
            response.status()
        )));
    }
    response.json().map_err(|_| SyncError::InvalidResponse)
}

fn device_decision(
    vault_path: &Path,
    target_device_id: &str,
    action: &str,
) -> Result<DeviceSummary, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_active_device(vault_path, &mut config)?;
    let approver = device_id(&config)?;
    let target = Uuid::parse_str(target_device_id)
        .map_err(|_| SyncError::InvalidConfig("target device ID is invalid".to_owned()))?;
    if action == "revoke" && approver == target {
        return Err(SyncError::InvalidConfig(
            "a device cannot revoke itself".to_owned(),
        ));
    }
    let signature = hex::encode(
        device_key_pair(&config)?.sign(&device_decision_message(approver, target, action)),
    );

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Request {
        approver_device_id: Uuid,
        signature_hex: String,
    }

    let response = client()?
        .post(format!(
            "{}/v1/devices/{target}/{action}",
            config.server_url
        ))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .json(&Request {
            approver_device_id: approver,
            signature_hex: signature,
        })
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    map_auth_status(&response)?;
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {} during device {action}",
            response.status()
        )));
    }
    let device: ServerDeviceSummary = response.json().map_err(|_| SyncError::InvalidResponse)?;
    Ok(device_summary(device))
}

fn conflict_outcome(action: &str, revision: u64, message: &str) -> SyncExecution {
    SyncExecution {
        outcome: SyncOutcome {
            action: action.to_owned(),
            revision,
            message: message.to_owned(),
            vault_locked: false,
        },
        pull: None,
    }
}

struct RemoteVault {
    revision: u64,
    content_sha256: String,
    bytes: Vec<u8>,
}

struct UploadMetadata {
    revision: u64,
    content_sha256: String,
}

fn fetch_remote(config: &SyncConfig, vault_id: &str) -> Result<Option<RemoteVault>, SyncError> {
    let path = format!("/v1/vaults/{vault_id}");
    let timestamp = now_seconds()?;
    let signature = sign_request(config, "GET", &path, timestamp, &[], None)?;
    let response = client()?
        .get(format!("{}{}", config.server_url, path))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .header(HEADER_DEVICE_ID, device_id(config)?.to_string())
        .header(HEADER_DEVICE_TIMESTAMP, timestamp.to_string())
        .header(HEADER_DEVICE_SIGNATURE, signature)
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    map_auth_status(&response)?;
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {}",
            response.status()
        )));
    }

    let revision = parse_u64_header(&response, HEADER_REVISION)?;
    let content_sha256 = parse_hash_header(&response)?;
    let bytes = read_response_bytes(response)?;
    if sha256_hex(&bytes) != content_sha256 {
        return Err(SyncError::InvalidResponse);
    }

    Ok(Some(RemoteVault {
        revision,
        content_sha256,
        bytes,
    }))
}

fn upload(
    config: &SyncConfig,
    vault_id: &str,
    base_revision: u64,
    bytes: &[u8],
) -> Result<UploadMetadata, SyncError> {
    let path = format!("/v1/vaults/{vault_id}");
    let timestamp = now_seconds()?;
    let signature = sign_request(config, "PUT", &path, timestamp, bytes, Some(base_revision))?;
    let response = client()?
        .put(format!("{}{}", config.server_url, path))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .header(HEADER_DEVICE_ID, device_id(config)?.to_string())
        .header(HEADER_DEVICE_TIMESTAMP, timestamp.to_string())
        .header(HEADER_DEVICE_SIGNATURE, signature)
        .header(HEADER_BASE_REVISION, base_revision.to_string())
        .header("content-type", "application/octet-stream")
        .body(bytes.to_vec())
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;

    map_auth_status(&response)?;
    if response.status() == StatusCode::CONFLICT {
        return Err(SyncError::Transport(
            "server revision changed while uploading; run sync again".to_owned(),
        ));
    }
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {}",
            response.status()
        )));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Metadata {
        revision: u64,
        content_sha256: String,
    }

    let metadata: Metadata = response.json().map_err(|_| SyncError::InvalidResponse)?;
    if metadata.content_sha256.len() != 64
        || !metadata
            .content_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(SyncError::InvalidResponse);
    }

    let expected_hash = sha256_hex(bytes);
    let expected_revision = base_revision
        .checked_add(1)
        .ok_or(SyncError::InvalidResponse)?;
    if metadata.content_sha256 != expected_hash || metadata.revision != expected_revision {
        return Err(SyncError::InvalidResponse);
    }

    Ok(UploadMetadata {
        revision: metadata.revision,
        content_sha256: metadata.content_sha256,
    })
}

fn sign_request(
    config: &SyncConfig,
    method: &str,
    path: &str,
    timestamp: u64,
    body: &[u8],
    base_revision: Option<u64>,
) -> Result<String, SyncError> {
    let message = request_signature_message(method, path, timestamp, body, base_revision);
    Ok(hex::encode(device_key_pair(config)?.sign(&message)))
}

fn device_enrollment_message(device_id: Uuid, name: &str, verifying_key_hex: &str) -> Vec<u8> {
    format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        device_id,
        name,
        verifying_key_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

fn request_signature_message(
    method: &str,
    path: &str,
    timestamp: u64,
    body: &[u8],
    base_revision: Option<u64>,
) -> Vec<u8> {
    format!(
        "dragonforge/device-request/v1\n{}\n{}\n{}\n{}\n{}",
        method,
        path,
        timestamp,
        sha256_hex(body),
        base_revision
            .map(|value| value.to_string())
            .unwrap_or_default()
    )
    .into_bytes()
}

fn device_decision_message(
    approver_device_id: Uuid,
    target_device_id: Uuid,
    action: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/device-decision/v1\n{}\n{}\n{}",
        action, approver_device_id, target_device_id
    )
    .into_bytes()
}

fn device_key_pair(config: &SyncConfig) -> Result<MlDsa65KeyPair, SyncError> {
    let seed_hex = config
        .device_signing_seed_hex
        .as_deref()
        .ok_or_else(|| SyncError::InvalidConfig("missing device signing key".to_owned()))?;
    let mut seed = hex::decode(seed_hex)
        .map_err(|_| SyncError::InvalidConfig("device signing key is invalid".to_owned()))?;
    let result = MlDsa65KeyPair::from_seed(&seed)
        .map_err(|_| SyncError::InvalidConfig("device signing key is invalid".to_owned()));
    seed.zeroize();
    result
}

fn device_id(config: &SyncConfig) -> Result<Uuid, SyncError> {
    config
        .device_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| SyncError::InvalidConfig("missing device identity".to_owned()))
}

fn device_summary(device: ServerDeviceSummary) -> DeviceSummary {
    DeviceSummary {
        device_id: device.device_id.to_string(),
        name: device.name,
        status: device.status.as_str().to_owned(),
        created_at_ms: device.created_at_ms,
        approved_at_ms: device.approved_at_ms,
        revoked_at_ms: device.revoked_at_ms,
    }
}

fn map_auth_status(response: &Response) -> Result<(), SyncError> {
    if response.status() == StatusCode::UNAUTHORIZED {
        Err(SyncError::Unauthorized)
    } else if response.status() == StatusCode::FORBIDDEN {
        Err(SyncError::DeviceNotActive)
    } else {
        Ok(())
    }
}

fn client() -> Result<Client, SyncError> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .https_only(false)
        .build()
        .map_err(|error| SyncError::Transport(error.to_string()))
}

fn bearer(token: &str) -> Result<HeaderValue, SyncError> {
    HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| SyncError::InvalidResponse)
}

fn parse_u64_header(response: &Response, name: &str) -> Result<u64, SyncError> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(SyncError::InvalidResponse)
}

fn parse_hash_header(response: &Response) -> Result<String, SyncError> {
    let hash = response
        .headers()
        .get(HEADER_CONTENT_SHA256)
        .and_then(|value| value.to_str().ok())
        .ok_or(SyncError::InvalidResponse)?
        .to_ascii_lowercase();
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SyncError::InvalidResponse);
    }
    Ok(hash)
}

fn read_response_bytes(response: Response) -> Result<Vec<u8>, SyncError> {
    let mut bytes = Vec::new();
    response
        .take(MAX_VAULT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    if bytes.len() as u64 > MAX_VAULT_FILE_BYTES {
        return Err(SyncError::InvalidResponse);
    }
    Ok(bytes)
}

fn validate_remote_bytes(bytes: &[u8], expected_vault_id: &str) -> Result<(), SyncError> {
    let vault_id = validate_encrypted_vault_bytes(bytes)
        .map_err(|error| SyncError::InvalidRemoteVault(error.to_string()))?;
    if vault_id != expected_vault_id {
        return Err(SyncError::InvalidRemoteVault(
            "remote encrypted vault ID does not match the local vault".to_owned(),
        ));
    }
    Ok(())
}

fn read_vault_bytes(path: &Path) -> Result<Vec<u8>, SyncError> {
    let metadata = fs::metadata(path).map_err(|error| SyncError::Io(error.to_string()))?;
    if metadata.len() > MAX_VAULT_FILE_BYTES {
        return Err(SyncError::InvalidConfig(
            "local vault exceeds sync size limit".to_owned(),
        ));
    }
    fs::read(path).map_err(|error| SyncError::Io(error.to_string()))
}

fn validate_server_url(value: &str) -> Result<(), SyncError> {
    let parsed = Url::parse(value)
        .map_err(|_| SyncError::InvalidConfig("server URL is invalid".to_owned()))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = parsed.host_str().unwrap_or_default();
            if matches!(host, "127.0.0.1" | "::1" | "localhost") {
                Ok(())
            } else {
                Err(SyncError::InvalidConfig(
                    "plaintext HTTP is allowed only for loopback development servers".to_owned(),
                ))
            }
        }
        _ => Err(SyncError::InvalidConfig(
            "sync server URL must use HTTPS, except loopback HTTP for development".to_owned(),
        )),
    }
}

fn validate_token(token: &str) -> Result<(), SyncError> {
    if token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(SyncError::InvalidConfig(
            "sync token must be a 64-character hexadecimal value".to_owned(),
        ))
    }
}

fn validate_device_name(name: &str) -> Result<&str, SyncError> {
    if name.is_empty() || name.chars().count() > 120 {
        Err(SyncError::InvalidConfig(
            "device name must contain 1 to 120 characters".to_owned(),
        ))
    } else {
        Ok(name)
    }
}

fn now_seconds() -> Result<u64, SyncError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(|_| SyncError::InvalidResponse)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn new_credential_id() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        Some(Uuid::new_v4().to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn ensure_credential_id(config: &mut SyncConfig) {
    #[cfg(target_os = "windows")]
    if config.credential_id.is_none() {
        config.credential_id = new_credential_id();
    }
}

fn cleanup_replaced_credential(
    previous: Option<&str>,
    current: Option<&str>,
) -> Result<(), SyncError> {
    #[cfg(target_os = "windows")]
    if let Some(previous) = previous.filter(|previous| Some(*previous) != current) {
        secret_store::delete(previous).map_err(SyncError::SecretStorage)?;
    }
    let _ = (previous, current);
    Ok(())
}

fn config_path(vault_path: &Path) -> PathBuf {
    let mut path = vault_path.as_os_str().to_os_string();
    path.push(".sync.json");
    PathBuf::from(path)
}

fn load_config(vault_path: &Path) -> Result<Option<SyncConfig>, SyncError> {
    let path = config_path(vault_path);
    let backup = path.with_extension("json.bak");
    if !path.exists() && backup.exists() {
        fs::rename(&backup, &path).map_err(|error| SyncError::Io(error.to_string()))?;
    }

    let mut bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SyncError::Io(error.to_string())),
    };
    let parsed = serde_json::from_slice::<SyncConfigDisk>(&bytes)
        .map_err(|error| SyncError::InvalidConfig(error.to_string()));
    bytes.zeroize();
    let mut disk = parsed?;
    if !matches!(disk.version, 1 | 2 | SYNC_CONFIG_VERSION) {
        return Err(SyncError::InvalidConfig(
            "unsupported sync configuration version".to_owned(),
        ));
    }
    validate_server_url(&disk.server_url)?;

    let original_version = disk.version;

    #[cfg(target_os = "windows")]
    let (sync_token, device_signing_seed_hex, credential_id) = if original_version
        == SYNC_CONFIG_VERSION
    {
        if disk.sync_token.is_some() || disk.device_signing_seed_hex.is_some() {
            return Err(SyncError::InvalidConfig(
                "version 3 Windows sync sidecars must not contain plaintext secrets".to_owned(),
            ));
        }
        let credential_id = disk.credential_id.clone().ok_or_else(|| {
            SyncError::InvalidConfig("missing Windows credential reference".to_owned())
        })?;
        let mut bundle = secret_store::load(&credential_id).map_err(SyncError::SecretStorage)?;
        let sync_token = std::mem::take(&mut bundle.sync_token);
        let device_signing_seed_hex = bundle.device_signing_seed_hex.take();
        (sync_token, device_signing_seed_hex, Some(credential_id))
    } else {
        let sync_token = disk.sync_token.take().ok_or_else(|| {
            SyncError::InvalidConfig("legacy sync sidecar is missing its sync token".to_owned())
        })?;
        let device_signing_seed_hex = disk.device_signing_seed_hex.take();
        (
            sync_token,
            device_signing_seed_hex,
            Some(Uuid::new_v4().to_string()),
        )
    };

    #[cfg(not(target_os = "windows"))]
    let (sync_token, device_signing_seed_hex, credential_id) = (
        disk.sync_token.take().ok_or_else(|| {
            SyncError::InvalidConfig("sync sidecar is missing its sync token".to_owned())
        })?,
        disk.device_signing_seed_hex.take(),
        None,
    );

    validate_token(&sync_token)?;
    let config = SyncConfig {
        version: SYNC_CONFIG_VERSION,
        server_url: disk.server_url,
        sync_token,
        last_revision: disk.last_revision,
        last_content_sha256: disk.last_content_sha256,
        device_id: disk.device_id,
        device_name: disk.device_name,
        device_signing_seed_hex,
        credential_id,
    };

    if original_version != SYNC_CONFIG_VERSION {
        save_config(vault_path, &config)?;
    }
    Ok(Some(config))
}

fn disk_config(config: &SyncConfig, protect_secrets: bool) -> SyncConfigDisk {
    SyncConfigDisk {
        version: SYNC_CONFIG_VERSION,
        server_url: config.server_url.clone(),
        sync_token: (!protect_secrets).then(|| config.sync_token.clone()),
        last_revision: config.last_revision,
        last_content_sha256: config.last_content_sha256.clone(),
        device_id: config.device_id.clone(),
        device_name: config.device_name.clone(),
        device_signing_seed_hex: if protect_secrets {
            None
        } else {
            config.device_signing_seed_hex.clone()
        },
        credential_id: config.credential_id.clone(),
    }
}

fn save_config(vault_path: &Path, config: &SyncConfig) -> Result<(), SyncError> {
    validate_token(&config.sync_token)?;

    #[cfg(target_os = "windows")]
    {
        let credential_id = config.credential_id.as_deref().ok_or_else(|| {
            SyncError::InvalidConfig("missing Windows credential reference".to_owned())
        })?;
        let bundle = SyncSecretBundle {
            sync_token: config.sync_token.clone(),
            device_signing_seed_hex: config.device_signing_seed_hex.clone(),
        };
        secret_store::store(credential_id, &bundle).map_err(SyncError::SecretStorage)?;
    }

    let disk = disk_config(config, cfg!(target_os = "windows"));
    let path = config_path(vault_path);
    let temp = path.with_extension("json.tmp");
    let mut bytes =
        serde_json::to_vec(&disk).map_err(|error| SyncError::InvalidConfig(error.to_string()))?;

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .map_err(|error| SyncError::Io(error.to_string()))?;
    let write_result = file
        .write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| SyncError::Io(error.to_string()));
    bytes.zeroize();
    write_result?;
    let backup = path.with_extension("json.bak");
    if backup.exists() {
        fs::remove_file(&backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }

    if path.exists() {
        fs::rename(&path, &backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }

    if let Err(error) = fs::rename(&temp, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(SyncError::Io(error.to_string()));
    }

    if backup.exists() {
        fs::remove_file(&backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }

    Ok(())
}

fn replace_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("vault");
    let temp = path.with_extension(format!("{extension}.tmp"));
    let backup = path.with_extension(format!("{extension}.bak"));

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .map_err(|error| SyncError::Io(error.to_string()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| SyncError::Io(error.to_string()))?;

    if backup.exists() {
        fs::remove_file(&backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }
    if path.exists() {
        fs::rename(path, &backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }
    if let Err(error) = fs::rename(&temp, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(SyncError::Io(error.to_string()));
    }
    if backup.exists() {
        fs::remove_file(backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }
    Ok(())
}

fn status_from_config(config: &SyncConfig) -> SyncStatus {
    SyncStatus {
        configured: true,
        server_url: Some(config.server_url.clone()),
        last_revision: config.last_revision,
        last_content_sha256: config.last_content_sha256.clone(),
        device_id: config.device_id.clone(),
        device_name: config.device_name.clone(),
        secret_storage: secret_store::storage_label().to_owned(),
    }
}

fn empty_status() -> SyncStatus {
    SyncStatus {
        configured: false,
        server_url: None,
        last_revision: 0,
        last_content_sha256: None,
        device_id: None,
        device_name: None,
        secret_storage: secret_store::storage_label().to_owned(),
    }
}

pub(crate) struct RecoveryRegistration {
    pub account_id: Uuid,
    pub generation: u64,
}

pub(crate) fn register_recovery(
    vault_path: &Path,
    vault_id: &str,
    recovery_verifying_key_hex: &str,
    envelope_hex: &str,
) -> Result<RecoveryRegistration, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    ensure_active_device(vault_path, &mut config)?;
    let vault_id = Uuid::parse_str(vault_id)
        .map_err(|_| SyncError::InvalidConfig("vault ID is invalid".to_owned()))?;
    let device_id = device_id(&config)?;
    let envelope = hex::decode(envelope_hex)
        .map_err(|_| SyncError::InvalidConfig("recovery envelope is invalid".to_owned()))?;
    let message = format!(
        "dragonforge/recovery-configure/v1\n{}\n{}\n{}\n{}",
        device_id,
        vault_id,
        recovery_verifying_key_hex.to_ascii_lowercase(),
        sha256_hex(&envelope)
    );
    let signature = hex::encode(device_key_pair(&config)?.sign(message.as_bytes()));

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Request<'a> {
        vault_id: Uuid,
        device_id: Uuid,
        recovery_verifying_key_hex: &'a str,
        envelope_hex: &'a str,
        signature_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ResponseBody {
        account_id: Uuid,
        generation: u64,
    }

    let response = client()?
        .post(format!("{}/v1/recovery/configure", config.server_url))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .json(&Request {
            vault_id,
            device_id,
            recovery_verifying_key_hex,
            envelope_hex,
            signature_hex: signature,
        })
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    map_auth_status(&response)?;
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {} while configuring recovery",
            response.status()
        )));
    }
    let body: ResponseBody = response.json().map_err(|_| SyncError::InvalidResponse)?;
    Ok(RecoveryRegistration {
        account_id: body.account_id,
        generation: body.generation,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn install_recovered_config(
    vault_path: &Path,
    server_url: String,
    sync_token: String,
    revision: u64,
    content_sha256: String,
    device_id: Uuid,
    device_name: String,
    device_signing_seed_hex: String,
) -> Result<SyncStatus, SyncError> {
    validate_server_url(&server_url)?;
    validate_token(&sync_token)?;
    let config = SyncConfig {
        version: SYNC_CONFIG_VERSION,
        server_url: server_url.trim_end_matches('/').to_owned(),
        sync_token,
        last_revision: revision,
        last_content_sha256: Some(content_sha256),
        device_id: Some(device_id.to_string()),
        device_name: Some(device_name),
        device_signing_seed_hex: Some(device_signing_seed_hex),
        credential_id: new_credential_id(),
    };
    save_config(vault_path, &config)?;
    Ok(status_from_config(&config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_loopback_http_server_urls_are_allowed() {
        assert!(validate_server_url("https://sync.example.com").is_ok());
        assert!(validate_server_url("http://127.0.0.1:8787").is_ok());
        assert!(validate_server_url("http://localhost:8787").is_ok());
        assert!(validate_server_url("http://example.com").is_err());
        assert!(validate_server_url("file:///tmp/sync").is_err());
    }

    #[test]
    fn sync_token_requires_256_bit_hex_value() {
        assert!(validate_token(&"a".repeat(64)).is_ok());
        assert!(validate_token(&"g".repeat(64)).is_err());
        assert!(validate_token(&"a".repeat(63)).is_err());
    }

    #[test]
    fn request_transcript_changes_when_body_or_revision_changes() {
        let timestamp = 42;
        let one = request_signature_message("PUT", "/v1/vaults/test", timestamp, b"one", Some(1));
        let two = request_signature_message("PUT", "/v1/vaults/test", timestamp, b"two", Some(1));
        let revision =
            request_signature_message("PUT", "/v1/vaults/test", timestamp, b"one", Some(2));
        assert_ne!(one, two);
        assert_ne!(one, revision);
    }

    #[test]
    fn phase8_sidecar_migrates_without_losing_sync_history() {
        let directory = tempfile::tempdir().unwrap();
        let vault_path = directory.path().join("legacy.dfvault");
        let sidecar = config_path(&vault_path);
        let last_hash = "ab".repeat(32);
        let legacy = serde_json::json!({
            "version": 1,
            "serverUrl": "http://127.0.0.1:8787",
            "syncToken": "a".repeat(64),
            "lastRevision": 7,
            "lastContentSha256": last_hash,
        });
        fs::write(&sidecar, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let status = configure(&vault_path, "http://127.0.0.1:8787", &"a".repeat(64)).unwrap();

        assert!(status.configured);
        assert_eq!(status.last_revision, 7);
        assert_eq!(
            status.last_content_sha256.as_deref(),
            Some(last_hash.as_str())
        );
        assert!(status.device_id.is_some());
        assert!(status.device_name.is_some());

        let migrated: serde_json::Value =
            serde_json::from_slice(&fs::read(sidecar).unwrap()).unwrap();
        assert_eq!(migrated["version"], 2);
        assert_eq!(migrated["lastRevision"], 7);
        assert_eq!(migrated["lastContentSha256"], last_hash);
        assert_eq!(
            migrated["syncToken"],
            serde_json::Value::String("a".repeat(64))
        );
        assert!(migrated["deviceId"].as_str().is_some());
        assert_eq!(
            migrated["deviceSigningSeedHex"].as_str().map(str::len),
            Some(64)
        );
    }
}
