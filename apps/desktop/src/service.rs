use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use dragonforge_vault::{
    AccountSecret, LoginItem, PasswordPolicy, SecureNoteItem, Vault, VaultItemData, VaultItemKind,
    VaultItemSummary, generate_password,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("{0}")]
    Vault(#[from] dragonforge_vault::VaultError),
    #[error("the vault is locked")]
    Locked,
    #[error("desktop session state is unavailable")]
    StateUnavailable,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invalid Account Secret")]
    InvalidAccountSecret,
}

pub type DesktopResult<T> = Result<T, DesktopError>;

struct Session {
    vault: Vault,
    path: PathBuf,
}

#[derive(Default)]
pub struct DesktopService {
    session: Mutex<Option<Session>>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub unlocked: bool,
    pub vault_path: Option<String>,
    pub vault_id: Option<String>,
    pub item_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVaultResponse {
    pub account_secret_hex: String,
    pub status: AppStatus,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummaryDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct ItemDraft {
    #[zeroize(skip)]
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
}

impl DesktopService {
    pub fn status(&self) -> DesktopResult<AppStatus> {
        let session = self.lock_session()?;
        Ok(status_from_session(session.as_ref()))
    }

    pub fn create_vault(
        &self,
        path: impl Into<PathBuf>,
        master_password: &str,
    ) -> DesktopResult<CreateVaultResponse> {
        let path = normalize_vault_path(path.into())?;
        let (vault, account_secret) = Vault::create(&path, master_password)?;
        let secret = account_secret.export();
        let account_secret_hex = hex::encode(secret.as_slice());

        let status = AppStatus {
            unlocked: true,
            vault_path: Some(path.display().to_string()),
            vault_id: Some(vault.vault_id().to_owned()),
            item_count: vault.len(),
        };

        *self.lock_session()? = Some(Session { vault, path });
        Ok(CreateVaultResponse {
            account_secret_hex,
            status,
        })
    }

    pub fn unlock_vault(
        &self,
        path: impl Into<PathBuf>,
        master_password: &str,
        account_secret_hex: &str,
    ) -> DesktopResult<AppStatus> {
        let path = path.into();
        let account_secret = decode_account_secret(account_secret_hex)?;
        let vault = Vault::open(&path, master_password, &account_secret)?;
        let status = AppStatus {
            unlocked: true,
            vault_path: Some(path.display().to_string()),
            vault_id: Some(vault.vault_id().to_owned()),
            item_count: vault.len(),
        };
        *self.lock_session()? = Some(Session { vault, path });
        Ok(status)
    }

    pub fn lock_vault(&self) -> DesktopResult<AppStatus> {
        self.lock_session()?.take();
        Ok(AppStatus {
            unlocked: false,
            vault_path: None,
            vault_id: None,
            item_count: 0,
        })
    }

    pub fn list_items(&self, query: Option<&str>) -> DesktopResult<Vec<ItemSummaryDto>> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        let summaries = match query.map(str::trim).filter(|value| !value.is_empty()) {
            Some(query) => session.vault.search(query)?,
            None => session.vault.list()?,
        };
        Ok(summaries.into_iter().map(summary_to_dto).collect())
    }

    pub fn get_item(&self, id: &str) -> DesktopResult<ItemDto> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        let item = session.vault.get_item(id)?;

        let (kind, username, password, url, notes) = match item.data {
            VaultItemData::Login(login) => (
                "login".to_owned(),
                login.username,
                login.password,
                login.url,
                login.notes,
            ),
            VaultItemData::SecureNote(note) => (
                "secure_note".to_owned(),
                String::new(),
                String::new(),
                String::new(),
                note.notes,
            ),
        };

        Ok(ItemDto {
            id: item.id,
            name: item.name,
            kind,
            favorite: item.favorite,
            tags: item.tags,
            username,
            password,
            url,
            notes,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }

    pub fn save_item(&self, draft: &ItemDraft) -> DesktopResult<String> {
        validate_draft(draft)?;
        let mut session = self.lock_session()?;
        let session = session.as_mut().ok_or(DesktopError::Locked)?;
        let data = draft_data(draft)?;

        match draft.id.as_deref() {
            Some(id) => {
                session.vault.update_item(
                    id,
                    draft.name.trim().to_owned(),
                    draft.favorite,
                    normalized_tags(&draft.tags),
                    data,
                )?;
                Ok(id.to_owned())
            }
            None => Ok(session.vault.add_item(
                draft.name.trim().to_owned(),
                draft.favorite,
                normalized_tags(&draft.tags),
                data,
            )?),
        }
    }

    pub fn delete_item(&self, id: &str) -> DesktopResult<()> {
        let mut session = self.lock_session()?;
        let session = session.as_mut().ok_or(DesktopError::Locked)?;
        session.vault.delete_item(id)?;
        Ok(())
    }

    pub fn generate_password(&self, length: usize) -> DesktopResult<String> {
        let policy = PasswordPolicy {
            length,
            ..PasswordPolicy::default()
        };
        Ok(generate_password(policy)?)
    }

    pub fn change_master_password(
        &self,
        new_master_password: &str,
        account_secret_hex: &str,
    ) -> DesktopResult<()> {
        let account_secret = decode_account_secret(account_secret_hex)?;
        let mut session = self.lock_session()?;
        let session = session.as_mut().ok_or(DesktopError::Locked)?;
        session
            .vault
            .change_master_password(new_master_password, &account_secret)?;
        Ok(())
    }

    pub fn verify_vault(&self) -> DesktopResult<()> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        session.vault.verify_integrity()?;
        Ok(())
    }

    pub fn export_backup(&self, destination: impl AsRef<Path>) -> DesktopResult<()> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        session.vault.export_backup(destination)?;
        Ok(())
    }

    pub fn import_backup(
        &self,
        source: impl AsRef<Path>,
        destination: impl Into<PathBuf>,
        master_password: &str,
        account_secret_hex: &str,
    ) -> DesktopResult<AppStatus> {
        let account_secret = decode_account_secret(account_secret_hex)?;
        let destination = normalize_vault_path(destination.into())?;
        let vault = Vault::import_backup(
            source,
            &destination,
            master_password,
            &account_secret,
        )?;
        let status = AppStatus {
            unlocked: true,
            vault_path: Some(destination.display().to_string()),
            vault_id: Some(vault.vault_id().to_owned()),
            item_count: vault.len(),
        };
        *self.lock_session()? = Some(Session {
            vault,
            path: destination,
        });
        Ok(status)
    }

    fn lock_session(&self) -> DesktopResult<MutexGuard<'_, Option<Session>>> {
        self.session
            .lock()
            .map_err(|_| DesktopError::StateUnavailable)
    }
}

fn status_from_session(session: Option<&Session>) -> AppStatus {
    match session {
        Some(session) => AppStatus {
            unlocked: true,
            vault_path: Some(session.path.display().to_string()),
            vault_id: Some(session.vault.vault_id().to_owned()),
            item_count: session.vault.len(),
        },
        None => AppStatus {
            unlocked: false,
            vault_path: None,
            vault_id: None,
            item_count: 0,
        },
    }
}

fn normalize_vault_path(mut path: PathBuf) -> DesktopResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(DesktopError::InvalidInput(
            "a vault file location is required".to_owned(),
        ));
    }
    if path.extension().is_none() {
        path.set_extension("dfvault");
    }
    Ok(path)
}

fn decode_account_secret(value: &str) -> DesktopResult<AccountSecret> {
    let compact: String = value.chars().filter(|character| !character.is_whitespace()).collect();
    let mut bytes = hex::decode(compact).map_err(|_| DesktopError::InvalidAccountSecret)?;
    let result = AccountSecret::from_bytes(&bytes).map_err(|_| DesktopError::InvalidAccountSecret);
    bytes.zeroize();
    result
}

fn validate_draft(draft: &ItemDraft) -> DesktopResult<()> {
    if draft.name.trim().is_empty() {
        return Err(DesktopError::InvalidInput(
            "item name cannot be empty".to_owned(),
        ));
    }
    match draft.kind.as_str() {
        "login" | "secure_note" => Ok(()),
        _ => Err(DesktopError::InvalidInput(
            "unsupported item type".to_owned(),
        )),
    }
}

fn draft_data(draft: &ItemDraft) -> DesktopResult<VaultItemData> {
    match draft.kind.as_str() {
        "login" => Ok(VaultItemData::Login(LoginItem {
            username: draft.username.clone(),
            password: draft.password.clone(),
            url: draft.url.clone(),
            notes: draft.notes.clone(),
        })),
        "secure_note" => Ok(VaultItemData::SecureNote(SecureNoteItem {
            notes: draft.notes.clone(),
        })),
        _ => Err(DesktopError::InvalidInput(
            "unsupported item type".to_owned(),
        )),
    }
}

fn normalized_tags(tags: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn summary_to_dto(summary: VaultItemSummary) -> ItemSummaryDto {
    ItemSummaryDto {
        id: summary.id,
        name: summary.name,
        kind: match summary.kind {
            VaultItemKind::Login => "login",
            VaultItemKind::SecureNote => "secure_note",
        }
        .to_owned(),
        favorite: summary.favorite,
        tags: summary.tags,
        updated_at: summary.updated_at,
    }
}
