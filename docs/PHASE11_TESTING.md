# Phase 11 Credential Protection Verification

## Automated Windows verification

Run:

```powershell
.\scripts\run-phase11-tests.ps1
```

or:

```text
scripts\run-phase11-tests.cmd
```

For a faster debug-only pass:

```powershell
.\scripts\run-phase11-tests.ps1 -SkipRelease
```

The runner writes:

```text
test-logs\dragonforge-phase11-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase11-YYYYMMDD-HHMMSS.log.sha256
```

## Focused Phase 11 test

On Windows:

```powershell
cargo test -p dragonforge-desktop --test secret_storage -- --nocapture
```

This test uses the real Windows Credential Manager for a disposable randomly referenced sync credential and removes it before completion.

## Required coverage

Phase 11 verification includes:

- Rust formatting;
- Clippy with warnings denied;
- desktop/browser JavaScript syntax;
- browser-extension tests;
- full workspace tests;
- protected-v3 sidecar serialization tests;
- Windows Credential Manager integration test;
- Phase 10 server and desktop recovery regressions;
- Phase 9 device-enrollment regressions;
- Phase 8 multi-device synchronization regressions;
- Phase 7 sync-server regressions;
- vault-hardening and native-host regressions;
- PostgreSQL-enabled server tests;
- release tests/builds unless skipped.

## Manual inspection

After configuring sync on Windows, inspect:

```text
<your vault>.dfvault.sync.json
```

Confirm:

- `version` is `3`;
- `credentialId` exists;
- `syncToken` does not exist;
- `deviceSigningSeedHex` does not exist.

The desktop sync-status line should report:

```text
secrets: Windows Credential Manager
```

## Legacy migration exercise

Use test credentials only.

1. Start with a Phase 10/version-2 sync sidecar containing a test token and signing seed.
2. Launch the Phase 11 desktop or request sync status.
3. Confirm the sidecar becomes version 3.
4. Confirm the token and seed are absent from the rewritten sidecar.
5. Confirm synchronization still works.
6. Confirm device enrollment/listing still works.
7. Confirm Phase 10 recovery still rotates credentials and produces a protected v3 sidecar.
8. Use **Remove sync** and confirm the sidecar disappears.

## Security checks

Do not put real passwords or production tokens in test fixtures.

A Phase 11 Windows sidecar must never persist:

- sync bearer token;
- ML-DSA device signing seed;
- master password;
- Account Secret;
- Vault Master Key;
- recovery private seed.
