use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dragonforge_crypto::{
    AeadCipher, Aes256GcmCipher, EncryptedEnvelope, MlDsa65KeyPair, OsRandom, SecretKey,
};
use dragonforge_vault::{AccountSecret, MAX_VAULT_FILE_BYTES, Vault, validate_encrypted_vault_bytes};
use rand_core::{OsRng, RngCore};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::sync::{self, DeviceSummary, SyncError, SyncStatus};

const RECOVERY_KIT_PREFIX: &str = "DFRK1";
const RECOVERY_AAD_DOMAIN: &str = "dragonforge/recovery-envelope/v1";
const HEADER_REVISION: &str = "x-dragonforge-revision";
const HEADER_CONTENT_SHA256: &str = "x-dragonforge-content-sha256";

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySetup {
    pub recovery_kit: String,
    #[zeroize(skip)]
    pub account_id: String,
    #[zeroize(skip)]
    pub vault_id: String,
    #[zeroize(skip)]
    pub generation: u64,
}

pub(crate) struct RecoveredVault {
    pub vault: Vault,
    pub path: PathBuf,
    pub account_secret_hex: String,
    pub recovery_kit: String,
    pub generation: u64,
    pub sync_status: SyncStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeginResponse {
    generation: u64,
    envelope_hex: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompleteResponse {
    generation: u64,
    sync_token: String,
    device: ServerDeviceSummary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerDeviceSummary {
    device_id: Uuid,
    name: String,
    status: String,
    created_at_ms: u64,
    approved_at_ms: Option<u64>,
    revoked_at_ms: Option<u64>,
}

struct ParsedKit {
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    seed: Vec<u8>,
}

pub(crate) fn configure(
    vault_path: &Path,
    vault_id: &str,
    account_secret_hex: &str,
) -> Result<RecoverySetup, SyncError> {
    let vault_id_uuid = Uuid::parse_str(vault_id)
        .map_err(|_| SyncError::InvalidConfig("vault ID is invalid".to_owned()))?;
    let mut account_secret = decode_account_secret_bytes(account_secret_hex)?;
    let recovery_key = MlDsa65KeyPair::generate();
    let recovery_seed = recovery_key.export_seed();
    let envelope = encrypt_account_secret(&recovery_seed, vault_id_uuid, &account_secret)?;
    account_secret.zeroize();

    let envelope_bytes =
        serde_json::to_vec(&envelope).map_err(|_| SyncError::InvalidResponse)?;
    let envelope_hex = hex::encode(&envelope_bytes);
    let recovery_verifying_key_hex = hex::encode(recovery_key.verifying_key().as_bytes());
    let registration = sync::register_recovery(
        vault_path,
        vault_id,
        &recovery_verifying_key_hex,
        &envelope_hex,
    )?;

    let recovery_kit = format!(
        "{}:{}:{}:{}:{}",
        RECOVERY_KIT_PREFIX,
        registration.account_id,
        vault_id_uuid,
        registration.generation,
        hex::encode(recovery_seed.as_slice())
    );

    Ok(RecoverySetup {
        recovery_kit,
        account_id: registration.account_id.to_string(),
        vault_id: vault_id_uuid.to_string(),
        generation: registration.generation,
    })
}

pub(crate) fn recover(
    destination: &Path,
    server_url: &str,
    recovery_kit: &str,
    master_password: &str,
    device_name: Option<&str>,
) -> Result<RecoveredVault, SyncError> {
    if destination.exists() {
        return Err(SyncError::InvalidConfig(
            "recovery destination already exists".to_owned(),
        ));
    }
    validate_server_url(server_url)?;
    let server_url = server_url.trim_end_matches('/').to_owned();
    let parsed = parse_recovery_kit(recovery_kit)?;
    let old_recovery_key = MlDsa65KeyPair::from_seed(&parsed.seed)
        .map_err(|_| SyncError::InvalidConfig("recovery kit is invalid".to_owned()))?;

    let begin = recovery_auth_request(
        &server_url,
        "/v1/recovery/begin",
        "begin",
        &parsed,
        &old_recovery_key,
    )?;
    let begin: BeginResponse = begin.json().map_err(|_| SyncError::InvalidResponse)?;
    if begin.generation != parsed.generation {
        return Err(SyncError::InvalidResponse);
    }

    let envelope_bytes = hex::decode(&begin.envelope_hex)
        .map_err(|_| SyncError::InvalidResponse)?;
    let envelope: EncryptedEnvelope =
        serde_json::from_slice(&envelope_bytes).map_err(|_| SyncError::InvalidResponse)?;
    let mut account_secret_bytes =
        decrypt_account_secret(&parsed.seed, parsed.vault_id, &envelope)?;
    let account_secret = AccountSecret::from_bytes(&account_secret_bytes)
        .map_err(|error| SyncError::InvalidRemoteVault(error.to_string()))?;
    let account_secret_hex = hex::encode(&account_secret_bytes);
    account_secret_bytes.zeroize();

    let vault_response = recovery_auth_request(
        &server_url,
        "/v1/recovery/vault",
        "vault",
        &parsed,
        &old_recovery_key,
    )?;
    let revision = parse_u64_header(&vault_response, HEADER_REVISION)?;
    let content_sha256 = parse_hash_header(&vault_response)?;
    let vault_bytes = read_response_bytes(vault_response)?;
    if hex::encode(Sha256::digest(&vault_bytes)) != content_sha256 {
        return Err(SyncError::InvalidResponse);
    }
    let actual_vault_id = validate_encrypted_vault_bytes(&vault_bytes)
        .map_err(|error| SyncError::InvalidRemoteVault(error.to_string()))?;
    if actual_vault_id != parsed.vault_id.to_string() {
        return Err(SyncError::InvalidRemoteVault(
            "recovery vault ID does not match the recovery kit".to_owned(),
        ));
    }

    atomic_install(destination, &vault_bytes)?;
    let vault = match Vault::open(destination, master_password, &account_secret) {
        Ok(vault) => vault,
        Err(error) => {
            let _ = fs::remove_file(destination);
            return Err(SyncError::InvalidRemoteVault(format!(
                "recovered vault could not be unlocked: {error}"
            )));
        }
    };

    let replacement_device_id = Uuid::new_v4();
    let replacement_key = MlDsa65KeyPair::generate();
    let replacement_seed = replacement_key.export_seed();
    let replacement_name = normalized_device_name(device_name, replacement_device_id)?;
    let replacement_verifying_key_hex =
        hex::encode(replacement_key.verifying_key().as_bytes());
    let replacement_proof_signature_hex = hex::encode(replacement_key.sign(
        &device_enrollment_message(
            replacement_device_id,
            &replacement_name,
            &replacement_verifying_key_hex,
        ),
    ));

    let new_recovery_key = MlDsa65KeyPair::generate();
    let new_recovery_seed = new_recovery_key.export_seed();
    let new_recovery_verifying_key_hex =
        hex::encode(new_recovery_key.verifying_key().as_bytes());
    let new_recovery_proof_signature_hex = hex::encode(new_recovery_key.sign(
        &recovery_rotation_message(
            parsed.account_id,
            parsed.vault_id,
            &new_recovery_verifying_key_hex,
        ),
    ));
    let account_secret_export = account_secret.export();
    let new_envelope = encrypt_account_secret(
        &new_recovery_seed,
        parsed.vault_id,
        account_secret_export.as_slice(),
    )?;
    let new_envelope_bytes =
        serde_json::to_vec(&new_envelope).map_err(|_| SyncError::InvalidResponse)?;
    let new_envelope_hex = hex::encode(&new_envelope_bytes);

    let timestamp = now_seconds()?;
    let nonce_hex = fresh_nonce_hex();
    let complete_message = complete_message(
        parsed.account_id,
        parsed.vault_id,
        parsed.generation,
        timestamp,
        &nonce_hex,
        replacement_device_id,
        &replacement_name,
        &replacement_verifying_key_hex,
        &new_recovery_verifying_key_hex,
        &new_envelope_bytes,
    );
    let signature_hex = hex::encode(old_recovery_key.sign(&complete_message));

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CompleteRequest<'a> {
        account_id: Uuid,
        vault_id: Uuid,
        generation: u64,
        timestamp: u64,
        nonce_hex: &'a str,
        replacement_device_id: Uuid,
        replacement_name: &'a str,
        replacement_verifying_key_hex: &'a str,
        replacement_proof_signature_hex: &'a str,
        new_recovery_verifying_key_hex: &'a str,
        new_recovery_proof_signature_hex: &'a str,
        new_envelope_hex: &'a str,
        signature_hex: &'a str,
    }

    let response = client()?
        .post(format!("{server_url}/v1/recovery/complete"))
        .json(&CompleteRequest {
            account_id: parsed.account_id,
            vault_id: parsed.vault_id,
            generation: parsed.generation,
            timestamp,
            nonce_hex: &nonce_hex,
            replacement_device_id,
            replacement_name: &replacement_name,
            replacement_verifying_key_hex: &replacement_verifying_key_hex,
            replacement_proof_signature_hex: &replacement_proof_signature_hex,
            new_recovery_verifying_key_hex: &new_recovery_verifying_key_hex,
            new_recovery_proof_signature_hex: &new_recovery_proof_signature_hex,
            new_envelope_hex: &new_envelope_hex,
            signature_hex: &signature_hex,
        })
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    if response.status() == StatusCode::FORBIDDEN {
        let _ = fs::remove_file(destination);
        return Err(SyncError::Unauthorized);
    }
    if response.status() == StatusCode::CONFLICT {
        let _ = fs::remove_file(destination);
        return Err(SyncError::Transport(
            "the recovery kit was already used or rotated".to_owned(),
        ));
    }
    if !response.status().is_success() {
        let status = response.status();
        let _ = fs::remove_file(destination);
        return Err(SyncError::Transport(format!(
            "server returned HTTP {status} while completing recovery"
        )));
    }
    let completed: CompleteResponse =
        response.json().map_err(|_| SyncError::InvalidResponse)?;
    if completed.device.device_id != replacement_device_id
        || completed.device.status != "active"
        || completed.generation != parsed.generation.saturating_add(1)
    {
        return Err(SyncError::InvalidResponse);
    }

    let sync_status = sync::install_recovered_config(
        destination,
        server_url,
        completed.sync_token,
        revision,
        content_sha256,
        replacement_device_id,
        replacement_name,
        hex::encode(replacement_seed.as_slice()),
    )?;
    let new_recovery_kit = format!(
        "{}:{}:{}:{}:{}",
        RECOVERY_KIT_PREFIX,
        parsed.account_id,
        parsed.vault_id,
        completed.generation,
        hex::encode(new_recovery_seed.as_slice())
    );

    Ok(RecoveredVault {
        vault,
        path: destination.to_path_buf(),
        account_secret_hex,
        recovery_kit: new_recovery_kit,
        generation: completed.generation,
        sync_status,
    })
}

fn recovery_auth_request(
    server_url: &str,
    path: &str,
    action: &str,
    kit: &ParsedKit,
    recovery_key: &MlDsa65KeyPair,
) -> Result<Response, SyncError> {
    let timestamp = now_seconds()?;
    let nonce_hex = fresh_nonce_hex();
    let message = recovery_auth_message(
        action,
        kit.account_id,
        kit.vault_id,
        kit.generation,
        timestamp,
        &nonce_hex,
    );
    let signature_hex = hex::encode(recovery_key.sign(&message));

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Request<'a> {
        account_id: Uuid,
        vault_id: Uuid,
        generation: u64,
        timestamp: u64,
        nonce_hex: &'a str,
        signature_hex: &'a str,
    }

    let response = client()?
        .post(format!("{server_url}{path}"))
        .json(&Request {
            account_id: kit.account_id,
            vault_id: kit.vault_id,
            generation: kit.generation,
            timestamp,
            nonce_hex: &nonce_hex,
            signature_hex: &signature_hex,
        })
        .send()
        .map_err(|error| SyncError::Transport(error.to_string()))?;
    if response.status() == StatusCode::FORBIDDEN {
        return Err(SyncError::Unauthorized);
    }
    if response.status() == StatusCode::CONFLICT {
        return Err(SyncError::Transport(
            "the recovery kit has been rotated or already used".to_owned(),
        ));
    }
    if !response.status().is_success() {
        return Err(SyncError::Transport(format!(
            "server returned HTTP {} during recovery",
            response.status()
        )));
    }
    Ok(response)
}

fn encrypt_account_secret(
    seed: &[u8],
    vault_id: Uuid,
    account_secret: &[u8],
) -> Result<EncryptedEnvelope, SyncError> {
    let key = recovery_encryption_key(seed, vault_id)?;
    Aes256GcmCipher
        .seal(
            &OsRandom,
            &key,
            account_secret,
            recovery_aad(vault_id).as_bytes(),
        )
        .map_err(|error| SyncError::InvalidConfig(error.to_string()))
}

fn decrypt_account_secret(
    seed: &[u8],
    vault_id: Uuid,
    envelope: &EncryptedEnvelope,
) -> Result<Vec<u8>, SyncError> {
    let key = recovery_encryption_key(seed, vault_id)?;
    let plaintext = Aes256GcmCipher
        .open(&key, envelope, recovery_aad(vault_id).as_bytes())
        .map_err(|_| SyncError::Unauthorized)?;
    if plaintext.len() != 32 {
        return Err(SyncError::InvalidResponse);
    }
    Ok(plaintext)
}

fn recovery_encryption_key(seed: &[u8], vault_id: Uuid) -> Result<SecretKey, SyncError> {
    let mut digest = Sha256::new();
    digest.update(RECOVERY_AAD_DOMAIN.as_bytes());
    digest.update([0]);
    digest.update(vault_id.as_bytes());
    digest.update([0]);
    digest.update(seed);
    let bytes: [u8; 32] = digest.finalize().into();
    Ok(SecretKey::from_bytes(bytes))
}

fn recovery_aad(vault_id: Uuid) -> String {
    format!("{RECOVERY_AAD_DOMAIN}\n{vault_id}")
}

fn parse_recovery_kit(value: &str) -> Result<ParsedKit, SyncError> {
    let parts: Vec<_> = value.trim().split(':').collect();
    if parts.len() != 5 || parts[0] != RECOVERY_KIT_PREFIX {
        return Err(SyncError::InvalidConfig(
            "recovery kit format is invalid".to_owned(),
        ));
    }
    let account_id = Uuid::parse_str(parts[1])
        .map_err(|_| SyncError::InvalidConfig("recovery account ID is invalid".to_owned()))?;
    let vault_id = Uuid::parse_str(parts[2])
        .map_err(|_| SyncError::InvalidConfig("recovery vault ID is invalid".to_owned()))?;
    let generation = parts[3]
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| SyncError::InvalidConfig("recovery generation is invalid".to_owned()))?;
    let seed = hex::decode(parts[4])
        .map_err(|_| SyncError::InvalidConfig("recovery seed is invalid".to_owned()))?;
    MlDsa65KeyPair::from_seed(&seed)
        .map_err(|_| SyncError::InvalidConfig("recovery seed is invalid".to_owned()))?;
    Ok(ParsedKit {
        account_id,
        vault_id,
        generation,
        seed,
    })
}

fn decode_account_secret_bytes(value: &str) -> Result<Vec<u8>, SyncError> {
    let compact: String = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let bytes = hex::decode(compact)
        .map_err(|_| SyncError::InvalidConfig("Account Secret is invalid".to_owned()))?;
    if bytes.len() != 32 {
        return Err(SyncError::InvalidConfig(
            "Account Secret is invalid".to_owned(),
        ));
    }
    Ok(bytes)
}

fn normalized_device_name(
    preferred: Option<&str>,
    device_id: Uuid,
) -> Result<String, SyncError> {
    let default_name = format!("Recovered DragonForge Desktop {}", &device_id.to_string()[..8]);
    let name = preferred
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&default_name);
    if name.chars().count() > 120 {
        return Err(SyncError::InvalidConfig(
            "device name must contain 1 to 120 characters".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

fn fresh_nonce_hex() -> String {
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    hex::encode(nonce)
}

fn recovery_auth_message(
    action: &str,
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    timestamp: u64,
    nonce_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-auth/v1\n{}\n{}\n{}\n{}\n{}\n{}",
        action,
        account_id,
        vault_id,
        generation,
        timestamp,
        nonce_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

#[allow(clippy::too_many_arguments)]
fn complete_message(
    account_id: Uuid,
    vault_id: Uuid,
    generation: u64,
    timestamp: u64,
    nonce_hex: &str,
    replacement_device_id: Uuid,
    replacement_name: &str,
    replacement_verifying_key_hex: &str,
    new_recovery_verifying_key_hex: &str,
    new_envelope: &[u8],
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-complete/v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        account_id,
        vault_id,
        generation,
        timestamp,
        nonce_hex.to_ascii_lowercase(),
        replacement_device_id,
        replacement_name.trim(),
        replacement_verifying_key_hex.to_ascii_lowercase(),
        new_recovery_verifying_key_hex.to_ascii_lowercase(),
        hex::encode(Sha256::digest(new_envelope))
    )
    .into_bytes()
}

fn recovery_rotation_message(
    account_id: Uuid,
    vault_id: Uuid,
    verifying_key_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/recovery-rotation/v1\n{}\n{}\n{}",
        account_id,
        vault_id,
        verifying_key_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

fn device_enrollment_message(
    device_id: Uuid,
    name: &str,
    verifying_key_hex: &str,
) -> Vec<u8> {
    format!(
        "dragonforge/device-enrollment/v1\n{}\n{}\n{}",
        device_id,
        name,
        verifying_key_hex.to_ascii_lowercase()
    )
    .into_bytes()
}

fn client() -> Result<Client, SyncError> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .https_only(false)
        .build()
        .map_err(|error| SyncError::Transport(error.to_string()))
}

fn validate_server_url(value: &str) -> Result<(), SyncError> {
    let parsed = url::Url::parse(value)
        .map_err(|_| SyncError::InvalidConfig("server URL is invalid".to_owned()))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if matches!(
            parsed.host_str().unwrap_or_default(),
            "127.0.0.1" | "::1" | "localhost"
        ) => Ok(()),
        "http" => Err(SyncError::InvalidConfig(
            "plaintext HTTP is allowed only for loopback development servers".to_owned(),
        )),
        _ => Err(SyncError::InvalidConfig(
            "sync server URL must use HTTPS, except loopback HTTP for development".to_owned(),
        )),
    }
}

fn now_seconds() -> Result<u64, SyncError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(|_| SyncError::InvalidResponse)
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

fn atomic_install(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| SyncError::Io(error.to_string()))?;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("dfvault");
    let temp = path.with_extension(format!("{extension}.recovery.tmp"));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
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
    fs::rename(&temp, path).map_err(|error| SyncError::Io(error.to_string()))
}

impl From<ServerDeviceSummary> for DeviceSummary {
    fn from(value: ServerDeviceSummary) -> Self {
        Self {
            device_id: value.device_id.to_string(),
            name: value.name,
            status: value.status,
            created_at_ms: value.created_at_ms,
            approved_at_ms: value.approved_at_ms,
            revoked_at_ms: value.revoked_at_ms,
        }
    }
}
