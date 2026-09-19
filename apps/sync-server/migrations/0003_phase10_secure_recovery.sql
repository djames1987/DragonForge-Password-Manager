CREATE TABLE IF NOT EXISTS sync_recovery (
    account_id UUID PRIMARY KEY REFERENCES sync_accounts(account_id) ON DELETE CASCADE,
    vault_id UUID NOT NULL,
    verifying_key BYTEA NOT NULL CHECK (octet_length(verifying_key) = 1952),
    envelope BYTEA NOT NULL CHECK (octet_length(envelope) BETWEEN 1 AND 16384),
    generation BIGINT NOT NULL CHECK (generation >= 1),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS sync_recovery_vault_idx
    ON sync_recovery (vault_id);
