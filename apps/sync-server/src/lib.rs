#![forbid(unsafe_code)]
//! Zero-knowledge sync-server foundation for DragonForge Password Manager.

mod api;
mod auth;
mod error;
mod model;
mod store;

pub use api::{AppState, build_router};
pub use auth::{hash_sync_token, new_sync_token};
pub use error::{ApiError, StoreError};
pub use model::{
    AccountRecord, AccountResponse, HealthResponse, StoredVault, SyncMetadata, MAX_SYNC_BLOB_BYTES,
    SYNC_PROTOCOL_VERSION,
};
pub use store::{InMemoryStore, SyncStore};

#[cfg(feature = "postgres")]
pub use store::PostgresStore;
