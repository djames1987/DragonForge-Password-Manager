# Phase 10 Secure Recovery Verification

## Automated Windows verification

Run:

```powershell
.\scripts\run-phase10-tests.ps1
```

or:

```text
scripts\run-phase10-tests.cmd
```

The runner writes:

```text
test-logs\dragonforge-phase10-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase10-YYYYMMDD-HHMMSS.log.sha256
```

## Phase 10 focused tests

Server recovery lifecycle:

```powershell
cargo test -p dragonforge-sync-server --test account_recovery -- --nocapture
```

Desktop end-to-end disaster recovery:

```powershell
cargo test -p dragonforge-desktop --test account_recovery -- --nocapture
```

## Required automated coverage

Phase 10 verifies:

- recovery configuration from an active device;
- ML-DSA recovery authorization;
- recovery generation binding;
- encrypted recovery-envelope round trip;
- recovery-only encrypted vault retrieval;
- new replacement-device proof-of-possession;
- new recovery-key proof-of-possession;
- atomic revocation of previous devices;
- sync-token rotation;
- recovery-key rotation;
- old recovery-kit invalidation;
- replacement-device synchronization after recovery;
- full desktop recovery of an actual encrypted `.dfvault`;
- Account Secret recovery client-side;
- preservation of vault items through recovery;
- rejection of an incorrect master password before recovery completion;
- reuse of the still-valid kit after a failed wrong-master attempt;
- Phase 9 device enrollment regression coverage;
- Phase 8 multi-device sync regression coverage;
- Phase 7 sync API regression coverage.

## Manual disaster-recovery exercise

Use test data only.

1. Create a vault and save at least one test login.
2. Configure sync and complete one successful upload.
3. Open **Vault settings → Secure account recovery**.
4. Enter the Account Secret and choose **Create / rotate recovery kit**.
5. Store the generated kit outside the test machine.
6. Close DragonForge and simulate loss of the original machine.
7. On a clean installation choose **Recover synchronized vault**.
8. Select a new local vault path.
9. Enter the sync-server URL, recovery kit, master password, and replacement-device name.
10. Complete recovery.
11. Save the displayed recovered Account Secret and new recovery kit.
12. Confirm the expected vault items are available.
13. Confirm the old machine can no longer synchronize.
14. Confirm the old recovery kit can no longer begin recovery.
15. Confirm the new recovery kit is accepted.

## Security checks

During testing confirm that server logs and database rows never contain:

- master password;
- plaintext Account Secret;
- Vault Master Key;
- recovery private seed;
- replacement-device private seed;
- decrypted vault entries.

Do not use production credentials while DragonForge remains unaudited.
