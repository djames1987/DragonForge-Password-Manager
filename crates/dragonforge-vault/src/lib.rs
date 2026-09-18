#![forbid(unsafe_code)]
//! Local encrypted vault for DragonForge Password Manager.

mod error;
mod format;
mod limits;
mod model;
mod password;
mod storage;
mod vault;

pub use error::{Result, VaultError};
pub use format::{
    CURRENT_VAULT_FORMAT_VERSION, MigrationStatus, VaultFormatInfo, inspect_vault_file,
};
pub use limits::{
    MAX_ITEM_CIPHERTEXT_BYTES, MAX_KDF_ITERATIONS, MAX_KDF_LANES, MAX_KDF_MEMORY_KIB,
    MAX_KDF_SALT_BYTES, MAX_VAULT_FILE_BYTES, MAX_VAULT_ITEMS,
};
pub use model::{
    LoginItem, SecureNoteItem, VaultItem, VaultItemData, VaultItemKind, VaultItemSummary,
};
pub use password::{PasswordPolicy, generate_password};
pub use vault::{AccountSecret, LockedVault, Vault, validate_encrypted_vault_bytes};
