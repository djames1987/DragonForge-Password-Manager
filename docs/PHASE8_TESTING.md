# Phase 8 Multi-Device Sync Verification

**Final status: PASS — implementation, two-device synchronization, conflict handling, tamper rejection, release builds, and cross-machine Windows build verification completed.**

## Automated Windows verification

Run:

```powershell
.\scripts\run-phase8-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase8-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase8-YYYYMMDD-HHMMSS.log.sha256
```

It verifies:

1. PowerShell script syntax;
2. Rust formatting;
3. Clippy with warnings denied;
4. browser-extension JavaScript regressions;
5. full debug workspace tests;
6. explicit Phase 8 two-device sync integration tests;
7. Phase 7 sync-server API tests;
8. PostgreSQL-feature server tests;
9. desktop/browser regressions;
10. Phase 4 vault-hardening regressions;
11. optimized release workspace tests;
12. release desktop/native-host/sync-server builds;
13. dependency visibility;
14. `cargo audit` when installed.

## Phase 8 system test

The integration suite:

```powershell
cargo test -p dragonforge-desktop --test multi_device_sync -- --nocapture
```

starts a real Phase 7 Axum sync server on an ephemeral loopback port and uses two independent `DesktopService` instances as separate devices.

It verifies:

- initial encrypted upload;
- safe onboarding when the server already has a vault;
- explicit Keep Remote onboarding;
- local-change upload;
- newer-remote download;
- automatic local lock before remote replacement;
- reopening the pulled vault with the same master password and Account Secret;
- two-device concurrent-edit conflict detection;
- Keep Remote conflict resolution;
- Keep Local conflict resolution;
- monotonic server revisions;
- resulting data visibility on the other device.

## Manual two-device test

For a manual test, use two copies of the same development vault on one or two Windows PCs.

1. Start the sync server.
2. Provision a test account/token.
3. Configure Device A and run Sync Now. It should upload revision 1.
4. Copy the same encrypted vault to Device B or use an existing matching copy.
5. Configure Device B with the same server/token.
6. Device B should report an initial conflict because the server already has state.
7. Choose Keep Remote.
8. Unlock Device B again.
9. Add an item on Device A and sync.
10. Sync Device B. It should download the new encrypted copy and lock.
11. Unlock Device B and verify the new item.
12. Edit both devices before either syncs.
13. Sync Device A.
14. Sync Device B and confirm DragonForge reports a conflict rather than overwriting either copy.
15. Test each conflict choice using development-only data.

## Security checks

Confirm that:

- the sync server cannot search or display vault item plaintext;
- non-loopback HTTP server URLs are rejected;
- invalid sync tokens are rejected;
- remote downloads with a bad hash are rejected;
- a remote pull locks the desktop vault;
- sync conflicts do not auto-overwrite;
- removing sync configuration does not delete the local vault.

Do not use production credentials during Phase 8 testing.
