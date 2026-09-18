#![forbid(unsafe_code)]
//! Local encrypted vault for DragonForge Password Manager.

mod error;
mod model;
mod password;
mod storage;
mod vault;

pub use error::{Result, VaultError};
pub use model::{
    LoginItem, SecureNoteItem, VaultItem, VaultItemData, VaultItemKind, VaultItemSummary,
};
pub use password::{PasswordPolicy, generate_password};
pub use vault::{AccountSecret, LockedVault, Vault};
