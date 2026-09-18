use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use dragonforge_vault::{
    AccountSecret, LoginItem, PasswordPolicy, SecureNoteItem, Vault, VaultItemData, VaultItemKind,
    VaultItemSummary, generate_password,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
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
    #[error("{0}")]
    Sync(#[from] crate::sync::SyncError),
}

pub type DesktopResult<T> = Result<T, DesktopError>;

struct Session {
    vault: Vault,
    path: PathBuf,
}

#[derive(Clone, Default)]
pub struct DesktopService {
    session: Arc<Mutex<Option<Session>>>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub unlocked: bool,
    pub vault_path: Option<String>,
    pub vault_id: Option<String>,
    pub item_count: usize,
}

#[derive(Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct CreateVaultResponse {
    pub account_secret_hex: String,
    #[zeroize(skip)]
    pub status: AppStatus,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserLoginSummary {
    pub id: String,
    pub name: String,
    pub username: String,
    pub url: String,
    pub favorite: bool,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCredential {
    #[zeroize(skip)]
    pub id: String,
    #[zeroize(skip)]
    pub name: String,
    pub username: String,
    pub password: String,
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

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
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

    pub fn browser_search(
        &self,
        page_url: &str,
        query: Option<&str>,
    ) -> DesktopResult<Vec<BrowserLoginSummary>> {
        let requested_host = normalized_host(page_url).ok_or_else(|| {
            DesktopError::InvalidInput(
                "the active tab URL is not a supported web origin".to_owned(),
            )
        })?;
        let query = query.unwrap_or_default().trim().to_ascii_lowercase();

        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;

        let mut matches = Vec::new();
        for summary in session.vault.list()? {
            if summary.kind != VaultItemKind::Login {
                continue;
            }

            let item = session.vault.get_item(&summary.id)?;
            let VaultItemData::Login(login) = item.data else {
                continue;
            };

            if normalized_host(&login.url).as_deref() != Some(requested_host.as_str()) {
                continue;
            }

            if !query.is_empty() {
                let searchable = format!(
                    "{}\n{}\n{}",
                    item.name.to_ascii_lowercase(),
                    login.username.to_ascii_lowercase(),
                    login.url.to_ascii_lowercase()
                );
                if !searchable.contains(&query) {
                    continue;
                }
            }

            matches.push(BrowserLoginSummary {
                id: item.id,
                name: item.name,
                username: login.username.clone(),
                url: login.url.clone(),
                favorite: item.favorite,
            });

            if matches.len() >= 20 {
                break;
            }
        }

        matches.sort_by(|left, right| {
            right.favorite.cmp(&left.favorite).then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
        });
        Ok(matches)
    }

    pub fn browser_credential(
        &self,
        item_id: &str,
        page_url: &str,
    ) -> DesktopResult<BrowserCredential> {
        let requested_host = normalized_host(page_url).ok_or_else(|| {
            DesktopError::InvalidInput(
                "the active tab URL is not a supported web origin".to_owned(),
            )
        })?;

        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        let item = session.vault.get_item(item_id)?;
        let VaultItemData::Login(login) = item.data else {
            return Err(DesktopError::InvalidInput(
                "only login items can be sent to a browser".to_owned(),
            ));
        };

        let stored_host = normalized_host(&login.url).ok_or_else(|| {
            DesktopError::InvalidInput(
                "the saved login URL is not a supported web origin".to_owned(),
            )
        })?;
        if stored_host != requested_host {
            return Err(DesktopError::InvalidInput(
                "the requested credential does not belong to the active site".to_owned(),
            ));
        }

        Ok(BrowserCredential {
            id: item.id,
            name: item.name,
            username: login.username.clone(),
            password: login.password.clone(),
        })
    }

    pub fn get_item(&self, id: &str) -> DesktopResult<ItemDto> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        let item = session.vault.get_item(id)?;

        let (kind, username, password, url, notes) = match &item.data {
            VaultItemData::Login(login) => (
                "login".to_owned(),
                login.username.clone(),
                login.password.clone(),
                login.url.clone(),
                login.notes.clone(),
            ),
            VaultItemData::SecureNote(note) => (
                "secure_note".to_owned(),
                String::new(),
                String::new(),
                String::new(),
                note.notes.clone(),
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

    pub fn sync_status(&self) -> DesktopResult<crate::sync::SyncStatus> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        Ok(crate::sync::status(&session.path)?)
    }

    pub fn configure_sync(
        &self,
        server_url: &str,
        sync_token: &str,
    ) -> DesktopResult<crate::sync::SyncStatus> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        Ok(crate::sync::configure(
            &session.path,
            server_url,
            sync_token,
        )?)
    }

    pub fn remove_sync(&self) -> DesktopResult<crate::sync::SyncStatus> {
        let session = self.lock_session()?;
        let session = session.as_ref().ok_or(DesktopError::Locked)?;
        Ok(crate::sync::remove(&session.path)?)
    }

    pub fn sync_now(&self) -> DesktopResult<crate::sync::SyncOutcome> {
        let mut session_guard = self.lock_session()?;
        let session = session_guard.as_ref().ok_or(DesktopError::Locked)?;
        let path = session.path.clone();
        let vault_id = session.vault.vault_id().to_owned();
        let execution = crate::sync::sync(&path, &vault_id)?;

        if let Some(pull) = execution.pull {
            session_guard.take();
            crate::sync::commit_pull(&path, &vault_id, pull)?;
        }

        Ok(execution.outcome)
    }

    pub fn resolve_sync_conflict(
        &self,
        strategy: &str,
    ) -> DesktopResult<crate::sync::SyncOutcome> {
        let mut session_guard = self.lock_session()?;
        let session = session_guard.as_ref().ok_or(DesktopError::Locked)?;
        let path = session.path.clone();
        let vault_id = session.vault.vault_id().to_owned();
        let execution = crate::sync::resolve(&path, &vault_id, strategy)?;

        if let Some(pull) = execution.pull {
            session_guard.take();
            crate::sync::commit_pull(&path, pull)?;
        }

        Ok(execution.outcome)
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
        let vault = Vault::import_backup(source, &destination, master_password, &account_secret)?;
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
    let compact: String = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
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

fn normalized_host(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }

    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_owned())
}
