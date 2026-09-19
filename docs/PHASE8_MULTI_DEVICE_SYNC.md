# Phase 8 — Multi-Device Sync

**Status: COMPLETE — implemented, multi-device tested, tamper-regression tested, and verified on Windows/Linux CI.**

## Status

Phase 8 connects the DragonForge desktop application to the Phase 7 zero-knowledge sync server.

Synchronization operates on the already-encrypted `.dfvault` file. The sync server never receives the master password, Account Secret, VMK, item-wrap keys, or decrypted vault items.

## Client state

Each synchronized local vault has a sidecar:

```text
<name>.dfvault.sync.json
```

It contains:

- sync configuration version;
- sync-server URL;
- 256-bit bearer sync token;
- last synchronized server revision;
- SHA-256 fingerprint of the last synchronized encrypted vault.

The sync token is a secret. Phase 8 stores it in a per-vault sidecar protected by local filesystem permissions. It is not stored inside the vault file and is never sent to the browser extension.

Hardware/OS-backed secret storage is still a future hardening step.

## Safe synchronization decision model

DragonForge compares three pieces of state:

1. current local encrypted-vault SHA-256;
2. last synchronized encrypted-vault SHA-256;
3. current server revision/hash.

This produces four normal outcomes.

### Up to date

Local ciphertext matches the last synchronized hash and the server has the same revision.

No changes are made.

### Upload

Local ciphertext changed but the server revision did not.

The client uploads the encrypted vault using the last synchronized revision as `X-DragonForge-Base-Revision`.

### Download

Local ciphertext did not change but the server has a newer revision.

The client:

1. downloads the opaque encrypted file;
2. verifies the server SHA-256 header;
3. performs full structural vault validation and verifies the embedded vault UUID;
4. authenticates every encrypted item with the currently unlocked vault's item-wrap key/AEAD context;
5. locks the currently unlocked local vault;
6. atomically replaces the local encrypted file;
7. records the new synchronized revision/hash.

The user must unlock the vault again after a remote pull. This prevents stale decrypted in-memory state from surviving replacement of the encrypted file.

### Conflict

Both local ciphertext and the server revision changed since the previous synchronization.

Phase 8 does **not** guess which copy is correct. It stops and requires an explicit choice:

- **Keep local** — upload the current local encrypted vault over the latest server revision.
- **Keep remote** — lock and replace the local encrypted vault with the latest server copy.

Phase 8 does not attempt field-level conflict merging.

## First sync

If the server has no matching vault:

- the current encrypted local vault is uploaded as revision 1.

If the server already has the vault but this device has no synchronization history:

- DragonForge reports an `initialConflict`;
- the user explicitly chooses Keep Local or Keep Remote.

This prevents a newly configured device from silently overwriting an existing synchronized vault.

## Rollback / consistency checks

The client stops synchronization when:

- the server revision is lower than the last synchronized revision;
- the server ciphertext hash changes without a revision change;
- a previously synchronized remote vault disappears.

These are surfaced as safety states rather than automatically modifying local data.

Phase 8 does not yet provide a cryptographically signed append-only rollback log; that remains future work.

## Server URL policy

Remote sync servers must use HTTPS.

Plain HTTP is accepted only for loopback development endpoints:

- `127.0.0.1`
- `::1`
- `localhost`

## Desktop commands

Tauri commands added in Phase 8:

- `sync_status`
- `configure_sync`
- `sync_now`
- `resolve_sync_conflict`
- `remove_sync`

Network synchronization runs off the UI thread.

## Desktop UI

Phase 8 adds:

- **Sync vault** sidebar action;
- sync server URL configuration;
- sync token configuration;
- current synchronized revision display;
- **Sync now**;
- **Remove sync**;
- conflict warning;
- **Keep local**;
- **Keep remote**.

## Development setup

Start the Phase 7 server:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "phase8-development-admin-token-32bytes-minimum"
cargo run -p dragonforge-sync-server
```

Provision a development sync account:

```powershell
$headers = @{ "X-DragonForge-Admin-Token" = "phase8-development-admin-token-32bytes-minimum" }
$account = Invoke-RestMethod -Method Post -Uri http://127.0.0.1:8787/v1/accounts -Headers $headers
$account
```

The returned `syncToken` is entered into DragonForge desktop sync settings on each device using the same synchronized vault.

Do not use real credentials during development testing.

## Atomic local replacement

Remote pulls are staged before replacement. The existing encrypted vault is moved to a temporary sync backup, the new encrypted file is moved into place, and the backup is removed only after replacement succeeds.

The synchronization sidecar is also updated through a temporary/backup replacement sequence so repeated state updates work correctly on Windows.

## Phase 8 non-goals

> Phase 9 supersedes the device-identity, enrollment-approval, and revocation items below. This list records what Phase 8 itself intentionally did not include.

Phase 8 intentionally does not yet include:

- automatic background/push synchronization;
- per-item or field-level merge;
- device identity/signatures;
- device enrollment approval;
- revocation UI;
- signed rollback-resistant state history;
- OS secure storage for sync tokens;
- server push/WebSocket notifications;
- offline operation queues;
- mobile synchronization.

Those remain later roadmap work.


## Completion verification

Phase 8 was closed after the following verification completed successfully:

- Rust formatting and Clippy with warnings denied;
- full debug workspace tests;
- browser-extension regression tests;
- Phase 7 sync-server API regressions;
- Phase 4 vault-hardening regressions;
- Phase 8 multi-device integration tests using two independent desktop services;
- initial upload and safe onboarding;
- newer-remote pull with local lock before replacement;
- explicit Keep Local and Keep Remote conflict resolution;
- tampered remote encrypted snapshot rejection before local replacement;
- release workspace tests;
- Windows desktop release build;
- Windows native-host release build;
- PostgreSQL-enabled sync-server release build;
- successful clean release build on a second Windows PC.

The CI cache step is explicitly non-blocking because cache upload/cleanup is not a product verification gate and may fail when hosted runners exhaust ephemeral disk after release builds.
