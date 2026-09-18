# Phase 7 — Sync Server Foundation

## Status

Phase 7 introduces DragonForge's first zero-knowledge sync-server boundary.

The server is deliberately unable to decrypt synchronized vault data. It stores opaque encrypted bytes plus the minimum metadata required for synchronization and conflict detection.

## Security boundary

The sync server must never receive or store:

- master passwords;
- Account Secrets;
- Vault Master Keys (VMKs);
- item-wrap keys;
- plaintext usernames, passwords, notes, tags, or URLs;
- browser bridge tokens.

Server authentication uses a separate random 256-bit sync token. The token is returned once at account provisioning time; only its SHA-256 digest is persisted by the server.

The sync token is **not** derived from the master password.

## Architecture

```text
Future DragonForge sync client (Phase 8)
       |
       | HTTPS required in deployment
       | Authorization: Bearer <256-bit sync token>
       v
DragonForge Sync API v1
       |
       +-- account authentication
       +-- payload size limits
       +-- optimistic revision checks
       +-- SHA-256 opaque-blob fingerprint
       |
       v
SyncStore trait
       |
       +-- InMemoryStore (tests/development only)
       |
       +-- PostgresStore
              |
              v
          PostgreSQL
```

## API version

Current sync protocol version: **1**.

### Health

```http
GET /v1/health
```

Returns:

```json
{
  "ok": true,
  "protocolVersion": 1
}
```

### Provision sync account

```http
POST /v1/accounts
X-DragonForge-Admin-Token: <server-admin-token>
```

Returns an account UUID and a random 256-bit hexadecimal sync token.

The endpoint is disabled unless `DRAGONFORGE_SYNC_ADMIN_TOKEN` is configured.

This admin-token provisioning mechanism is a Phase 7 bootstrap mechanism. User-facing registration, passkeys, device enrollment, and recovery are later phases.

### Upload encrypted vault state

```http
PUT /v1/vaults/{vault_id}
Authorization: Bearer <sync-token>
X-DragonForge-Base-Revision: <revision>
Content-Type: application/octet-stream

<opaque encrypted bytes>
```

For the first upload, `X-DragonForge-Base-Revision` must be `0`.

A successful first upload creates revision 1. Every later successful write increments the revision by exactly one.

If the supplied base revision does not match the current server revision, the server returns HTTP `409 Conflict` and does not overwrite the newer state.

### Download encrypted vault state

```http
GET /v1/vaults/{vault_id}
Authorization: Bearer <sync-token>
```

The response body is the exact opaque byte sequence that was stored.

Response headers include:

- `X-DragonForge-Revision`
- `X-DragonForge-Content-SHA256`
- `X-DragonForge-Updated-At-Ms`
- `Content-Type: application/octet-stream`

## Conflict model

Phase 7 provides optimistic concurrency control, not automatic conflict merging.

Example:

1. Client A and Client B both know revision 7.
2. Client A uploads with base revision 7.
3. The server stores revision 8.
4. Client B uploads with base revision 7.
5. The server rejects Client B with HTTP 409 and reports current revision 8.

Phase 8 will use this primitive to implement multi-device synchronization and conflict resolution.

## Payload limits

The maximum opaque synchronized blob is 64 MiB, matching the current local vault-file limit.

Empty payloads are rejected.

The server does not parse the encrypted payload format.

## PostgreSQL persistence

Build the server with:

```bash
cargo build -p dragonforge-sync-server --features postgres --release
```

Set:

```text
DRAGONFORGE_SYNC_DATABASE_URL=postgres://user:password@host/database
```

At startup, the PostgreSQL implementation runs the checked-in migration in:

```text
apps/sync-server/migrations/
```

The schema stores:

- account UUID;
- sync-token digest;
- vault UUID;
- monotonic server revision;
- SHA-256 opaque-blob fingerprint;
- opaque encrypted bytes;
- server update timestamp.

## Running for development

Without PostgreSQL support:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "phase7-development-admin-token-32bytes-minimum"
cargo run -p dragonforge-sync-server
```

The server binds to `127.0.0.1:8787` by default and uses volatile in-memory storage.

Override the bind address with:

```text
DRAGONFORGE_SYNC_BIND=127.0.0.1:8787
```

## TLS requirement

Phase 7 does not implement TLS termination inside the Rust process.

For any non-local deployment, place the service behind a properly configured HTTPS reverse proxy/load balancer. A bearer sync token must never be sent over plaintext HTTP across an untrusted network.

The default loopback bind is intentionally safe for local development.

## Logging

The server does not log:

- Authorization headers;
- sync tokens;
- admin tokens;
- encrypted payload contents.

Current startup output is limited to bind/persistence configuration warnings and fatal startup errors.

## PostgreSQL transaction behavior

The PostgreSQL store locks an existing vault row with `SELECT ... FOR UPDATE` before comparing the current revision and updating the blob.

This makes revision comparison and update atomic for a single vault row.

## Phase 7 non-goals

Phase 7 does not yet implement:

- desktop sync-client integration;
- multi-device merge logic;
- device enrollment/signatures;
- passkey account authentication;
- push notifications;
- rollback-resistant signed state history;
- deletion tombstones;
- sharing;
- recovery;
- production account-management UI;
- production TLS termination;
- rate limiting across a distributed fleet.

Those belong to later phases.
