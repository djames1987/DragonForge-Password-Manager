# Phase 9 Device Enrollment Verification

## Automated Windows verification

Run:

```powershell
.\scripts\run-phase9-tests.ps1
```

The runner writes:

```text
test-logs\dragonforge-phase9-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase9-YYYYMMDD-HHMMSS.log.sha256
```

It verifies:

1. PowerShell syntax;
2. Rust formatting;
3. Clippy with warnings denied;
4. desktop/browser JavaScript syntax;
5. browser extension regressions;
6. full debug workspace tests;
7. Phase 9 device enrollment lifecycle;
8. Phase 8 multi-device sync regressions;
9. Phase 7 server API regressions;
10. PostgreSQL-feature server compilation/tests;
11. desktop/native-host/vault regressions;
12. release workspace tests;
13. release desktop/native-host/sync-server builds;
14. dependency visibility;
15. `cargo audit` when installed.

## Focused Phase 9 test

```powershell
cargo test -p dragonforge-sync-server --test device_enrollment -- --nocapture
```

The lifecycle test proves:

- the first enrolled device becomes active;
- a second device remains pending;
- the pending device cannot upload;
- the first active device signs approval;
- the approved device can upload;
- the first active device signs revocation;
- the revoked device can no longer upload.

## Manual two-device enrollment test

Use development-only data.

1. Start the Phase 9 sync server.
2. Configure Device A with the server URL and account sync token.
3. Click **Enroll / refresh device** on Device A.
4. Confirm Device A reports `active`.
5. Configure Device B with the same server/token.
6. Enroll Device B.
7. Confirm Device B reports `pending` and cannot sync.
8. On Device A, refresh devices.
9. Approve Device B.
10. Refresh Device B and confirm it reports `active`.
11. Sync Device B and confirm normal Phase 8 behavior works.
12. From Device A, revoke Device B.
13. Attempt to sync Device B again.
14. Confirm the server rejects it as not active.

## Security checks

Confirm that:

- the server never receives a device private seed;
- reusing a device ID with another ML-DSA public key is rejected;
- pending and revoked devices cannot access encrypted vault bytes;
- request signatures cover ciphertext and base revision;
- timestamps outside the allowed skew window are rejected;
- bearer-only Phase 8 access is disabled after enrollment begins;
- revoking all devices does not permit automatic trust re-bootstrap.

Do not use real production credentials during Phase 9 testing.
