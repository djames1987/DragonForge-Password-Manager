# Phase 7 Sync Server Verification

## Automated Windows verification

Run:

```powershell
.\scripts\run-phase7-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase7-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase7-YYYYMMDD-HHMMSS.log.sha256
```

The Phase 7 runner verifies:

1. PowerShell script syntax;
2. Rust formatting;
3. Clippy with warnings denied;
4. full debug workspace tests;
5. explicit sync-server API tests;
6. sync-server tests with PostgreSQL feature enabled;
7. Phase 4 storage regressions;
8. Phase 6 browser regressions;
9. release workspace tests;
10. release sync-server build with PostgreSQL support;
11. desktop/native-host release builds;
12. dependency visibility;
13. `cargo audit` when installed.

No PostgreSQL instance is required for the automated API regression suite because it uses the same `SyncStore` boundary with `InMemoryStore`. CI still compiles/tests the PostgreSQL implementation through `--all-features`.

## Focused server tests

```powershell
cargo test -p dragonforge-sync-server --test sync_api -- --nocapture
cargo test -p dragonforge-sync-server --features postgres
```

The API suite verifies:

- protocol health response;
- admin-gated account provisioning;
- 256-bit hexadecimal sync-token generation;
- unauthorized token rejection;
- account isolation;
- exact opaque-byte preservation;
- revision 0 to revision 1 creation;
- stale revision conflict rejection;
- successful revision increments;
- empty-payload rejection;
- missing base-revision rejection.

## Development server smoke test

Start the server:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "phase7-local-admin-token-32bytes-minimum"
cargo run -p dragonforge-sync-server
```

From another PowerShell terminal:

```powershell
Invoke-RestMethod http://127.0.0.1:8787/v1/health
```

Expected:

```text
ok              : True
protocolVersion : 1
```

The in-memory backend is intentionally volatile; restarting the server removes test accounts and vault blobs.

## PostgreSQL smoke test

When PostgreSQL is available:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "<long-random-admin-token>"
$env:DRAGONFORGE_SYNC_DATABASE_URL = "postgres://user:password@127.0.0.1/dragonforge"
cargo run -p dragonforge-sync-server --features postgres
```

Confirm:

- migration completes;
- `/v1/health` returns protocol version 1;
- account provisioning works with the configured admin token;
- opaque upload/download survives a server restart;
- stale writes return 409.

Do not use production credentials or a production vault during Phase 7 development testing.
