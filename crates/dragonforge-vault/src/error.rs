use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("cryptographic operation failed: {0}")]
    Crypto(#[from] dragonforge_crypto::CryptoError),
    #[error("vault serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("vault I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported vault format version: {0}")]
    UnsupportedFormatVersion(u16),
    #[error("vault item not found: {0}")]
    ItemNotFound(String),
    #[error("refusing to overwrite existing vault: {0}")]
    VaultAlreadyExists(PathBuf),
    #[error("master password must not be empty")]
    EmptyMasterPassword,
    #[error("invalid account secret length")]
    InvalidAccountSecret,
    #[error("invalid password policy: {0}")]
    InvalidPasswordPolicy(&'static str),
    #[error("vault integrity verification failed for item {0}")]
    IntegrityFailure(String),
}

pub type Result<T> = std::result::Result<T, VaultError>;
