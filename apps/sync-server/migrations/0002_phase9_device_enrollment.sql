CREATE TABLE IF NOT EXISTS sync_devices (
    account_id UUID NOT NULL REFERENCES sync_accounts(account_id) ON DELETE CASCADE,
    device_id UUID NOT NULL,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 120),
    verifying_key BYTEA NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'revoked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    approved_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (account_id, device_id)
);

CREATE INDEX IF NOT EXISTS sync_devices_account_status_idx
    ON sync_devices (account_id, status, created_at);
