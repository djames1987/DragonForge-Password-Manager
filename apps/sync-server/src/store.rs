use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use dragonforge_crypto::constant_time_eq;
use uuid::Uuid;

use crate::{AccountRecord, StoreError, StoredVault};

#[async_trait]
pub trait SyncStore: Send + Sync {
    async fn create_account(&self, account: AccountRecord) -> Result<(), StoreError>;

    async fn authenticate(&self, token_hash: [u8; 32]) -> Result<Option<Uuid>, StoreError>;

    async fn get_vault(&self, account_id: Uuid, vault_id: Uuid) -> Result<StoredVault, StoreError>;

    async fn put_vault(
        &self,
        account_id: Uuid,
        vault_id: Uuid,
        base_revision: u64,
        ciphertext: Vec<u8>,
        content_sha256: [u8; 32],
    ) -> Result<StoredVault, StoreError>;
}

#[derive(Clone, Default)]
pub struct InMemoryStore {
    inner: Arc<Mutex<MemoryState>>,
}

#[derive(Default)]
struct MemoryState {
    accounts: Vec<AccountRecord>,
    vaults: HashMap<(Uuid, Uuid), StoredVault>,
}

#[async_trait]
impl SyncStore for InMemoryStore {
    async fn create_account(&self, account: AccountRecord) -> Result<(), StoreError> {
        let mut state = self.inner.lock().map_err(|_| StoreError::Internal)?;
        if state
            .accounts
            .iter()
            .any(|existing| existing.account_id == account.account_id)
        {
            return Err(StoreError::AccountExists);
        }
        state.accounts.push(account);
        Ok(())
    }

    async fn authenticate(&self, token_hash: [u8; 32]) -> Result<Option<Uuid>, StoreError> {
        let state = self.inner.lock().map_err(|_| StoreError::Internal)?;
        Ok(state
            .accounts
            .iter()
            .find(|account| constant_time_eq(&account.token_hash, &token_hash))
            .map(|account| account.account_id))
    }

    async fn get_vault(&self, account_id: Uuid, vault_id: Uuid) -> Result<StoredVault, StoreError> {
        let state = self.inner.lock().map_err(|_| StoreError::Internal)?;
        state
            .vaults
            .get(&(account_id, vault_id))
            .cloned()
            .ok_or(StoreError::NotFound)
    }

    async fn put_vault(
        &self,
        account_id: Uuid,
        vault_id: Uuid,
        base_revision: u64,
        ciphertext: Vec<u8>,
        content_sha256: [u8; 32],
    ) -> Result<StoredVault, StoreError> {
        let mut state = self.inner.lock().map_err(|_| StoreError::Internal)?;
        let key = (account_id, vault_id);

        let next_revision = match state.vaults.get(&key) {
            Some(existing) if existing.revision != base_revision => {
                return Err(StoreError::Conflict {
                    current_revision: existing.revision,
                });
            }
            Some(existing) => existing
                .revision
                .checked_add(1)
                .ok_or(StoreError::Internal)?,
            None if base_revision == 0 => 1,
            None => {
                return Err(StoreError::Conflict {
                    current_revision: 0,
                });
            }
        };

        let stored = StoredVault {
            account_id,
            vault_id,
            revision: next_revision,
            content_sha256,
            ciphertext,
            updated_at_ms: now_ms(),
        };
        state.vaults.insert(key, stored.clone());
        Ok(stored)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(feature = "postgres")]
mod postgres {
    use async_trait::async_trait;
    use sqlx::{PgPool, Row};
    use uuid::Uuid;

    use crate::{AccountRecord, StoreError, StoredVault, SyncStore};

    #[derive(Clone)]
    pub struct PostgresStore {
        pool: PgPool,
    }

    impl PostgresStore {
        pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
            let pool = PgPool::connect(database_url).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            Ok(Self { pool })
        }
    }

    #[async_trait]
    impl SyncStore for PostgresStore {
        async fn create_account(&self, account: AccountRecord) -> Result<(), StoreError> {
            let result =
                sqlx::query("INSERT INTO sync_accounts (account_id, token_hash) VALUES ($1, $2)")
                    .bind(account.account_id)
                    .bind(account.token_hash.to_vec())
                    .execute(&self.pool)
                    .await;

            match result {
                Ok(_) => Ok(()),
                Err(error)
                    if error
                        .as_database_error()
                        .is_some_and(|db| db.is_unique_violation()) =>
                {
                    Err(StoreError::AccountExists)
                }
                Err(_) => Err(StoreError::Internal),
            }
        }

        async fn authenticate(&self, token_hash: [u8; 32]) -> Result<Option<Uuid>, StoreError> {
            let row =
                sqlx::query("SELECT account_id FROM sync_accounts WHERE token_hash = $1 LIMIT 1")
                    .bind(token_hash.to_vec())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|_| StoreError::Internal)?;

            row.map(|row| row.try_get("account_id").map_err(|_| StoreError::Internal))
                .transpose()
        }

        async fn get_vault(
            &self,
            account_id: Uuid,
            vault_id: Uuid,
        ) -> Result<StoredVault, StoreError> {
            let row = sqlx::query(
                "SELECT revision, content_sha256, ciphertext,                  (EXTRACT(EPOCH FROM updated_at) * 1000)::bigint AS updated_at_ms                  FROM sync_vaults WHERE account_id = $1 AND vault_id = $2",
            )
            .bind(account_id)
            .bind(vault_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| StoreError::Internal)?
            .ok_or(StoreError::NotFound)?;

            row_to_vault(account_id, vault_id, row)
        }

        async fn put_vault(
            &self,
            account_id: Uuid,
            vault_id: Uuid,
            base_revision: u64,
            ciphertext: Vec<u8>,
            content_sha256: [u8; 32],
        ) -> Result<StoredVault, StoreError> {
            let mut transaction = self.pool.begin().await.map_err(|_| StoreError::Internal)?;

            let existing = sqlx::query(
                "SELECT revision FROM sync_vaults                  WHERE account_id = $1 AND vault_id = $2 FOR UPDATE",
            )
            .bind(account_id)
            .bind(vault_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| StoreError::Internal)?;

            match existing {
                Some(row) => {
                    let current_i64: i64 =
                        row.try_get("revision").map_err(|_| StoreError::Internal)?;
                    let current = u64::try_from(current_i64).map_err(|_| StoreError::Internal)?;
                    if current != base_revision {
                        return Err(StoreError::Conflict {
                            current_revision: current,
                        });
                    }

                    let next_revision = current.checked_add(1).ok_or(StoreError::Internal)?;
                    let revision_i64 =
                        i64::try_from(next_revision).map_err(|_| StoreError::Internal)?;

                    sqlx::query(
                        "UPDATE sync_vaults SET revision = $3, content_sha256 = $4,                          ciphertext = $5, updated_at = NOW()                          WHERE account_id = $1 AND vault_id = $2",
                    )
                    .bind(account_id)
                    .bind(vault_id)
                    .bind(revision_i64)
                    .bind(content_sha256.to_vec())
                    .bind(ciphertext)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|_| StoreError::Internal)?;
                }
                None if base_revision == 0 => {
                    let inserted = sqlx::query(
                        "INSERT INTO sync_vaults                          (account_id, vault_id, revision, content_sha256, ciphertext, updated_at)                          VALUES ($1, $2, 1, $3, $4, NOW())                          ON CONFLICT (account_id, vault_id) DO NOTHING",
                    )
                    .bind(account_id)
                    .bind(vault_id)
                    .bind(content_sha256.to_vec())
                    .bind(ciphertext)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|_| StoreError::Internal)?;

                    if inserted.rows_affected() == 0 {
                        let current_row = sqlx::query(
                            "SELECT revision FROM sync_vaults                              WHERE account_id = $1 AND vault_id = $2",
                        )
                        .bind(account_id)
                        .bind(vault_id)
                        .fetch_one(&mut *transaction)
                        .await
                        .map_err(|_| StoreError::Internal)?;
                        let current_i64: i64 = current_row
                            .try_get("revision")
                            .map_err(|_| StoreError::Internal)?;
                        let current_revision =
                            u64::try_from(current_i64).map_err(|_| StoreError::Internal)?;
                        return Err(StoreError::Conflict { current_revision });
                    }
                }
                None => {
                    return Err(StoreError::Conflict {
                        current_revision: 0,
                    });
                }
            }

            transaction
                .commit()
                .await
                .map_err(|_| StoreError::Internal)?;

            self.get_vault(account_id, vault_id).await
        }
    }

    fn row_to_vault(
        account_id: Uuid,
        vault_id: Uuid,
        row: sqlx::postgres::PgRow,
    ) -> Result<StoredVault, StoreError> {
        let revision_i64: i64 = row.try_get("revision").map_err(|_| StoreError::Internal)?;
        let updated_i64: i64 = row
            .try_get("updated_at_ms")
            .map_err(|_| StoreError::Internal)?;
        let digest: Vec<u8> = row
            .try_get("content_sha256")
            .map_err(|_| StoreError::Internal)?;
        let content_sha256: [u8; 32] = digest.try_into().map_err(|_| StoreError::Internal)?;

        Ok(StoredVault {
            account_id,
            vault_id,
            revision: u64::try_from(revision_i64).map_err(|_| StoreError::Internal)?,
            content_sha256,
            ciphertext: row
                .try_get("ciphertext")
                .map_err(|_| StoreError::Internal)?,
            updated_at_ms: u64::try_from(updated_i64).map_err(|_| StoreError::Internal)?,
        })
    }
}

#[cfg(feature = "postgres")]
pub use postgres::PostgresStore;
