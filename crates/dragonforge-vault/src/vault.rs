use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use dragonforge_crypto::{
    AeadCipher, Aes256GcmCipher, Argon2idConfig, Argon2idKdf, EncryptedEnvelope, HkdfSha512,
    KeyDeriver, OsRandom, PasswordKdf, RandomSource, SecretKey, generate_salt, generate_secret_key,
    unwrap_key, wrap_key,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
    LoginItem, Result, SecureNoteItem, VaultError, VaultItem, VaultItemData, VaultItemSummary,
    storage::{atomic_write, read_file},
};

const VAULT_FORMAT_VERSION: u16 = 1;
const ACCOUNT_SECRET_LEN: usize = 32;
const UNLOCK_INFO: &[u8] = b"dragonforge/vault/unlock/v1";
const ITEM_WRAP_INFO: &[u8] = b"dragonforge/vault/item-wrap/v1";

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct AccountSecret([u8; ACCOUNT_SECRET_LEN]);

impl AccountSecret {
    pub fn generate() -> Result<Self> {
        let mut bytes = [0_u8; ACCOUNT_SECRET_LEN];
        OsRandom.fill_bytes(&mut bytes)?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let array: [u8; ACCOUNT_SECRET_LEN] = bytes
            .try_into()
            .map_err(|_| VaultError::InvalidAccountSecret)?;
        Ok(Self(array))
    }

    #[must_use]
    pub fn export(&self) -> Zeroizing<[u8; ACCOUNT_SECRET_LEN]> {
        Zeroizing::new(self.0)
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; ACCOUNT_SECRET_LEN] {
        &self.0
    }
}

impl core::fmt::Debug for AccountSecret {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("AccountSecret([REDACTED])")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KdfMetadata {
    salt: Vec<u8>,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedItemRecord {
    id: String,
    revision: u64,
    created_at: u64,
    updated_at: u64,
    wrapped_item_key: EncryptedEnvelope,
    payload: EncryptedEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultFile {
    version: u16,
    vault_id: String,
    created_at: u64,
    updated_at: u64,
    kdf: KdfMetadata,
    wrapped_vmk: EncryptedEnvelope,
    items: Vec<EncryptedItemRecord>,
}

pub struct Vault {
    path: PathBuf,
    file: VaultFile,
    vmk: SecretKey,
    item_wrap_key: SecretKey,
}

impl core::fmt::Debug for Vault {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Vault")
            .field("path", &self.path)
            .field("vault_id", &self.file.vault_id)
            .field("item_count", &self.file.items.len())
            .field("vmk", &"[REDACTED]")
            .field("item_wrap_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct LockedVault {
    path: PathBuf,
}

impl LockedVault {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn unlock(self, master_password: &str, account_secret: &AccountSecret) -> Result<Vault> {
        Vault::open(self.path, master_password, account_secret)
    }
}

impl Vault {
    pub fn create(
        path: impl Into<PathBuf>,
        master_password: &str,
    ) -> Result<(Self, AccountSecret)> {
        let path = path.into();
        let account_secret = AccountSecret::generate()?;
        let kdf_config = Argon2idConfig::default();
        let salt = generate_salt(&OsRandom, 32)?;
        let password_key = Argon2idKdf::new(kdf_config)?.derive_key(master_password.as_bytes(), &salt)?;
        let unlock_key = derive_unlock_key(&password_key, &account_secret)?;
        let vmk = generate_secret_key(&OsRandom)?;
        let vault_id = Uuid::new_v4().to_string();
        let wrapped_vmk = wrap_key(
            &Aes256GcmCipher,
            &OsRandom,
            &unlock_key,
            &vmk,
            vmk_context(&vault_id).as_bytes(),
        )?;
        let item_wrap_key = HkdfSha512.derive_key(&vmk, None, ITEM_WRAP_INFO)?;
        let now = unix_time();

        let file = VaultFile {
            version: VAULT_FORMAT_VERSION,
            vault_id,
            created_at: now,
            updated_at: now,
            kdf: KdfMetadata {
                salt,
                memory_kib: kdf_config.memory_kib,
                iterations: kdf_config.iterations,
                lanes: kdf_config.lanes,
            },
            wrapped_vmk,
            items: Vec::new(),
        };

        let vault = Self {
            path,
            file,
            vmk,
            item_wrap_key,
        };
        vault.save()?;
        Ok((vault, account_secret))
    }

    pub fn open(
        path: impl Into<PathBuf>,
        master_password: &str,
        account_secret: &AccountSecret,
    ) -> Result<Self> {
        let path = path.into();
        let bytes = read_file(&path)?;
        let file: VaultFile = serde_json::from_slice(&bytes)?;
        if file.version != VAULT_FORMAT_VERSION {
            return Err(VaultError::UnsupportedFormatVersion(file.version));
        }

        let kdf_config =
            Argon2idConfig::new(file.kdf.memory_kib, file.kdf.iterations, file.kdf.lanes)?;
        let password_key =
            Argon2idKdf::new(kdf_config)?.derive_key(master_password.as_bytes(), &file.kdf.salt)?;
        let unlock_key = derive_unlock_key(&password_key, account_secret)?;
        let vmk = unwrap_key(
            &Aes256GcmCipher,
            &unlock_key,
            &file.wrapped_vmk,
            vmk_context(&file.vault_id).as_bytes(),
        )?;
        let item_wrap_key = HkdfSha512.derive_key(&vmk, None, ITEM_WRAP_INFO)?;

        Ok(Self {
            path,
            file,
            vmk,
            item_wrap_key,
        })
    }

    #[must_use]
    pub fn vault_id(&self) -> &str {
        &self.file.vault_id
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.file.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.file.items.is_empty()
    }

    pub fn add_login(
        &mut self,
        name: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        url: impl Into<String>,
        notes: impl Into<String>,
        tags: Vec<String>,
    ) -> Result<String> {
        self.add_item(
            name.into(),
            false,
            tags,
            VaultItemData::Login(LoginItem {
                username: username.into(),
                password: password.into(),
                url: url.into(),
                notes: notes.into(),
            }),
        )
    }

    pub fn add_secure_note(
        &mut self,
        name: impl Into<String>,
        notes: impl Into<String>,
        tags: Vec<String>,
    ) -> Result<String> {
        self.add_item(
            name.into(),
            false,
            tags,
            VaultItemData::SecureNote(SecureNoteItem {
                notes: notes.into(),
            }),
        )
    }

    pub fn add_item(
        &mut self,
        name: String,
        favorite: bool,
        tags: Vec<String>,
        data: VaultItemData,
    ) -> Result<String> {
        let now = unix_time();
        let item = VaultItem {
            id: Uuid::new_v4().to_string(),
            name,
            favorite,
            tags,
            data,
            created_at: now,
            updated_at: now,
        };
        let id = item.id.clone();
        let record = self.encrypt_item(&item, 1)?;
        self.file.items.push(record);
        self.file.updated_at = now;
        self.save()?;
        Ok(id)
    }

    pub fn get_item(&self, id: &str) -> Result<VaultItem> {
        let record = self
            .file
            .items
            .iter()
            .find(|record| record.id == id)
            .ok_or_else(|| VaultError::ItemNotFound(id.to_owned()))?;
        self.decrypt_item(record)
    }

    pub fn update_item(
        &mut self,
        id: &str,
        name: String,
        favorite: bool,
        tags: Vec<String>,
        data: VaultItemData,
    ) -> Result<()> {
        let position = self
            .file
            .items
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| VaultError::ItemNotFound(id.to_owned()))?;
        let old = self.decrypt_item(&self.file.items[position])?;
        let revision = self.file.items[position].revision.saturating_add(1);
        let now = unix_time();
        let item = VaultItem {
            id: id.to_owned(),
            name,
            favorite,
            tags,
            data,
            created_at: old.created_at,
            updated_at: now,
        };
        self.file.items[position] = self.encrypt_item(&item, revision)?;
        self.file.updated_at = now;
        self.save()
    }

    pub fn delete_item(&mut self, id: &str) -> Result<()> {
        let position = self
            .file
            .items
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| VaultError::ItemNotFound(id.to_owned()))?;
        self.file.items.remove(position);
        self.file.updated_at = unix_time();
        self.save()
    }

    pub fn search(&self, query: &str) -> Result<Vec<VaultItemSummary>> {
        let needle = query.to_lowercase();
        let mut matches = Vec::new();

        for record in &self.file.items {
            let item = self.decrypt_item(record)?;
            if item_matches(&item, &needle) {
                matches.push(VaultItemSummary {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    kind: item.data.kind(),
                    favorite: item.favorite,
                    tags: item.tags.clone(),
                    updated_at: item.updated_at,
                });
            }
        }

        matches.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(matches)
    }

    pub fn list(&self) -> Result<Vec<VaultItemSummary>> {
        self.search("")
    }

    pub fn verify_integrity(&self) -> Result<()> {
        for record in &self.file.items {
            let item = self
                .decrypt_item(record)
                .map_err(|_| VaultError::IntegrityFailure(record.id.clone()))?;
            if item.id != record.id {
                return Err(VaultError::IntegrityFailure(record.id.clone()));
            }
        }
        Ok(())
    }

    pub fn change_master_password(
        &mut self,
        new_master_password: &str,
        account_secret: &AccountSecret,
    ) -> Result<()> {
        let config = Argon2idConfig::default();
        let salt = generate_salt(&OsRandom, 32)?;
        let password_key =
            Argon2idKdf::new(config)?.derive_key(new_master_password.as_bytes(), &salt)?;
        let unlock_key = derive_unlock_key(&password_key, account_secret)?;
        self.file.wrapped_vmk = wrap_key(
            &Aes256GcmCipher,
            &OsRandom,
            &unlock_key,
            &self.vmk,
            vmk_context(&self.file.vault_id).as_bytes(),
        )?;
        self.file.kdf = KdfMetadata {
            salt,
            memory_kib: config.memory_kib,
            iterations: config.iterations,
            lanes: config.lanes,
        };
        self.file.updated_at = unix_time();
        self.save()
    }

    pub fn export_backup(&self, destination: impl AsRef<Path>) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&self.file)?;
        atomic_write(destination.as_ref(), &bytes)
    }

    pub fn import_backup(
        source: impl AsRef<Path>,
        destination: impl Into<PathBuf>,
        master_password: &str,
        account_secret: &AccountSecret,
    ) -> Result<Self> {
        let source = source.as_ref();
        let bytes = read_file(source)?;
        let temporary = destination.into();
        atomic_write(&temporary, &bytes)?;
        match Self::open(&temporary, master_password, account_secret) {
            Ok(vault) => {
                vault.verify_integrity()?;
                Ok(vault)
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&self.file)?;
        atomic_write(&self.path, &bytes)
    }

    #[must_use]
    pub fn lock(self) -> LockedVault {
        LockedVault { path: self.path.clone() }
    }

    fn encrypt_item(&self, item: &VaultItem, revision: u64) -> Result<EncryptedItemRecord> {
        let item_key = generate_secret_key(&OsRandom)?;
        let payload = serde_json::to_vec(item)?;
        let payload_envelope = Aes256GcmCipher.seal(
            &OsRandom,
            &item_key,
            &payload,
            item_aad(&self.file.vault_id, &item.id, revision).as_bytes(),
        )?;
        let wrapped_item_key = wrap_key(
            &Aes256GcmCipher,
            &OsRandom,
            &self.item_wrap_key,
            &item_key,
            item_key_context(&self.file.vault_id, &item.id).as_bytes(),
        )?;

        Ok(EncryptedItemRecord {
            id: item.id.clone(),
            revision,
            created_at: item.created_at,
            updated_at: item.updated_at,
            wrapped_item_key,
            payload: payload_envelope,
        })
    }

    fn decrypt_item(&self, record: &EncryptedItemRecord) -> Result<VaultItem> {
        let item_key = unwrap_key(
            &Aes256GcmCipher,
            &self.item_wrap_key,
            &record.wrapped_item_key,
            item_key_context(&self.file.vault_id, &record.id).as_bytes(),
        )?;
        let payload = Aes256GcmCipher.open(
            &item_key,
            &record.payload,
            item_aad(&self.file.vault_id, &record.id, record.revision).as_bytes(),
        )?;
        let item: VaultItem = serde_json::from_slice(&payload)?;
        if item.id != record.id
            || item.created_at != record.created_at
            || item.updated_at != record.updated_at
        {
            return Err(VaultError::IntegrityFailure(record.id.clone()));
        }
        Ok(item)
    }
}

fn derive_unlock_key(password_key: &SecretKey, account_secret: &AccountSecret) -> Result<SecretKey> {
    Ok(HkdfSha512.derive_key(
        password_key,
        Some(account_secret.as_bytes()),
        UNLOCK_INFO,
    )?)
}

fn vmk_context(vault_id: &str) -> String {
    format!("dragonforge/vault/vmk/v1:{vault_id}")
}

fn item_key_context(vault_id: &str, item_id: &str) -> String {
    format!("dragonforge/vault/item-key/v1:{vault_id}:{item_id}")
}

fn item_aad(vault_id: &str, item_id: &str, revision: u64) -> String {
    format!("dragonforge/vault/item/v1:{vault_id}:{item_id}:{revision}")
}

fn item_matches(item: &VaultItem, needle: &str) -> bool {
    if needle.is_empty()
        || item.name.to_lowercase().contains(needle)
        || item
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(needle))
    {
        return true;
    }

    match &item.data {
        VaultItemData::Login(login) => {
            login.username.to_lowercase().contains(needle)
                || login.url.to_lowercase().contains(needle)
                || login.notes.to_lowercase().contains(needle)
        }
        VaultItemData::SecureNote(note) => note.notes.to_lowercase().contains(needle),
    }
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
