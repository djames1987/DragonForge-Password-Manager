CREATE TABLE IF NOT EXISTS sync_accounts (
    account_id UUID PRIMARY KEY,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS sync_vaults (
    account_id UUID NOT NULL REFERENCES sync_accounts(account_id) ON DELETE CASCADE,
    vault_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    content_sha256 BYTEA NOT NULL CHECK (octet_length(content_sha256) = 32),
    ciphertext BYTEA NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, vault_id)
);

CREATE INDEX IF NOT EXISTS sync_vaults_updated_at_idx
    ON sync_vaults (account_id, updated_at);
