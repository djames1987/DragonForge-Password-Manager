use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use dragonforge_vault::{MAX_VAULT_FILE_BYTES, inspect_vault_file};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;
use zeroize::{Zeroize, ZeroizeOnDrop};

const SYNC_CONFIG_VERSION: u16 = 1;
const HEADER_BASE_REVISION: &str = "x-dragonforge-base-revision";
const HEADER_REVISION: &str = "x-dragonforge-revision";
const HEADER_CONTENT_SHA256: &str = "x-dragonforge-content-sha256";

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
    #[error("sync server returned malformed metadata")]
    InvalidResponse,
    #[error("sync state could not be read or written: {0}")]
    Io(String),
    #[error("remote vault failed structural validation: {0}")]
    InvalidRemoteVault(String),
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub configured: bool,
    pub server_url: Option<String>,
    pub last_revision: u64,
    pub last_content_sha256: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub action: String,
    pub revision: u64,
    pub message: String,
    pub vault_locked: bool,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
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

pub(crate) fn configure(
    vault_path: &Path,
    server_url: &str,
    sync_token: &str,
) -> Result<SyncStatus, SyncError> {
    validate_server_url(server_url)?;
    validate_token(sync_token)?;

    let config = SyncConfig {
        version: SYNC_CONFIG_VERSION,
        server_url: server_url.trim_end_matches('/').to_owned(),
        sync_token: sync_token.to_owned(),
        last_revision: 0,
        last_content_sha256: None,
    };
    save_config(vault_path, &config)?;
    Ok(status_from_config(&config))
}

pub(crate) fn remove(vault_path: &Path) -> Result<SyncStatus, SyncError> {
    let path = config_path(vault_path);
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(SyncError::Io(error.to_string())),
    }
    Ok(SyncStatus {
        configured: false,
        server_url: None,
        last_revision: 0,
        last_content_sha256: None,
    })
}

pub(crate) fn status(vault_path: &Path) -> Result<SyncStatus, SyncError> {
    match load_config(vault_path)? {
        Some(config) => Ok(status_from_config(&config)),
        None => Ok(SyncStatus {
            configured: false,
            server_url: None,
            last_revision: 0,
            last_content_sha256: None,
        }),
    }
}

pub(crate) fn sync(vault_path: &Path, vault_id: &str) -> Result<SyncExecution, SyncError> {
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
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
        Some(remote)
            if config.last_revision == 0 && config.last_content_sha256.is_none() =>
        {
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
                    validate_remote_bytes(&remote.bytes)?;
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
    match strategy {
        "keepLocal" => {
            let local = read_vault_bytes(vault_path)?;
            let current_remote_revision = fetch_remote(&config, vault_id)?
                .map_or(0, |remote| remote.revision);
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
            let remote = fetch_remote(&config, vault_id)?.ok_or_else(|| {
                SyncError::Transport("remote vault does not exist".to_owned())
            })?;
            validate_remote_bytes(&remote.bytes)?;
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
    pull: PullPayload,
) -> Result<(), SyncError> {
    validate_remote_bytes(&pull.bytes)?;
    replace_file_atomically(vault_path, &pull.bytes)?;
    let mut config = load_config(vault_path)?.ok_or(SyncError::NotConfigured)?;
    config.last_revision = pull.revision;
    config.last_content_sha256 = Some(pull.content_sha256);
    save_config(vault_path, &config)
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
    let response = client()?
        .get(format!("{}/v1/vaults/{vault_id}", config.server_url))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
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
    let response = client()?
        .put(format!("{}/v1/vaults/{vault_id}", config.server_url))
        .header(AUTHORIZATION, bearer(&config.sync_token)?)
        .header(HEADER_BASE_REVISION, base_revision.to_string())
        .header("content-type", "application/octet-stream")
        .body(bytes.to_vec())
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SyncError::Unauthorized);
    }
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

    let metadata: Metadata = response
        .json()
        .map_err(|_| SyncError::InvalidResponse)?;
    if metadata.content_sha256.len() != 64
        || !metadata
            .content_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(SyncError::InvalidResponse);
    }
    Ok(UploadMetadata {
        revision: metadata.revision,
        content_sha256: metadata.content_sha256,
    })
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

fn read_response_bytes(mut response: Response) -> Result<Vec<u8>, SyncError> {
    let mut bytes = Vec::new();
    response
        .take((MAX_VAULT_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    if bytes.len() > MAX_VAULT_FILE_BYTES {
        return Err(SyncError::InvalidResponse);
    }
    Ok(bytes)
}

fn validate_remote_bytes(bytes: &[u8]) -> Result<(), SyncError> {
    let temp_dir = std::env::temp_dir();
    let temp = temp_dir.join(format!(
        "dragonforge-sync-validate-{}-{}.dfvault",
        std::process::id(),
        sha256_hex(bytes)
    ));
    fs::write(&temp, bytes).map_err(|error| SyncError::Io(error.to_string()))?;
    let result = inspect_vault_file(&temp)
        .map(|_| ())
        .map_err(|error| SyncError::InvalidRemoteVault(error.to_string()));
    let _ = fs::remove_file(temp);
    result
}

fn read_vault_bytes(path: &Path) -> Result<Vec<u8>, SyncError> {
    let metadata = fs::metadata(path).map_err(|error| SyncError::Io(error.to_string()))?;
    if metadata.len() > MAX_VAULT_FILE_BYTES as u64 {
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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn config_path(vault_path: &Path) -> PathBuf {
    let mut path = vault_path.as_os_str().to_os_string();
    path.push(".sync.json");
    PathBuf::from(path)
}

fn load_config(vault_path: &Path) -> Result<Option<SyncConfig>, SyncError> {
    let path = config_path(vault_path);
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SyncError::Io(error.to_string())),
    };
    let config: SyncConfig =
        serde_json::from_slice(&bytes).map_err(|error| SyncError::InvalidConfig(error.to_string()))?;
    if config.version != SYNC_CONFIG_VERSION {
        return Err(SyncError::InvalidConfig(
            "unsupported sync configuration version".to_owned(),
        ));
    }
    validate_server_url(&config.server_url)?;
    validate_token(&config.sync_token)?;
    Ok(Some(config))
}

fn save_config(vault_path: &Path, config: &SyncConfig) -> Result<(), SyncError> {
    let path = config_path(vault_path);
    let temp = path.with_extension("json.tmp");
    let bytes =
        serde_json::to_vec(config).map_err(|error| SyncError::InvalidConfig(error.to_string()))?;

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
    file.write_all(&bytes)
        .map_err(|error| SyncError::Io(error.to_string()))?;
    file.sync_all()
        .map_err(|error| SyncError::Io(error.to_string()))?;
    fs::rename(&temp, &path).map_err(|error| SyncError::Io(error.to_string()))
}

fn replace_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
    let temp = path.with_extension("dfvault.sync.tmp");
    let backup = path.with_extension("dfvault.sync.bak");
    fs::write(&temp, bytes).map_err(|error| SyncError::Io(error.to_string()))?;

    if backup.exists() {
        fs::remove_file(&backup).map_err(|error| SyncError::Io(error.to_string()))?;
    }
    fs::rename(path, &backup).map_err(|error| SyncError::Io(error.to_string()))?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::rename(&backup, path);
        return Err(SyncError::Io(error.to_string()));
    }
    fs::remove_file(backup).map_err(|error| SyncError::Io(error.to_string()))
}

fn status_from_config(config: &SyncConfig) -> SyncStatus {
    SyncStatus {
        configured: true,
        server_url: Some(config.server_url.clone()),
        last_revision: config.last_revision,
        last_content_sha256: config.last_content_sha256.clone(),
    }
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
}
