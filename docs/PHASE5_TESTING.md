# Phase 5 Desktop Verification

## Windows prerequisites

Use a current stable Rust toolchain with the MSVC target. Tauri on Windows also requires Microsoft C++ build tools and WebView2.

## Launch the app

From the repository root:

```powershell
cargo run -p dragonforge-desktop
```

The application should open the DragonForge welcome screen without a separate frontend build.

## Automated verification

Run:

```powershell
.\scripts\run-phase5-tests.ps1
```

or:

```powershell
.\scripts\run-phase5-tests.cmd
```

The runner produces a UTF-8 logfile and SHA-256 companion file under `test-logs\`.

It executes:

1. Git and Rust tool/version capture.
2. Rustfmt.
3. Clippy with warnings denied.
4. Full debug workspace tests.
5. Desktop service integration tests.
6. Phase 4 hardening regression tests serially.
7. Full optimized release tests.
8. Release desktop build.
9. Duplicate dependency report.
10. `cargo audit` when installed.

## Manual UI smoke test

After automated tests pass:

1. Create a new vault in a temporary folder.
2. Copy/store the displayed Account Secret.
3. Add a login and a secure note.
4. Generate a password.
5. Search for the login.
6. Mark it as a favorite.
7. Edit it and verify the change persists.
8. Export an encrypted backup.
9. Run Verify Vault.
10. Lock.
11. Confirm decrypted details disappear.
12. Unlock using the same vault, master password, and Account Secret.
13. Confirm both records are still present.
14. Change the master password and verify the old password no longer unlocks.

Do not use real credentials during development testing.

## Phase 5 acceptance

A Phase 5 technical verification is:

```text
rustfmt                      PASS
clippy -D warnings           PASS
workspace debug tests        PASS
desktop service tests        PASS
Phase 4 hardening regression PASS
workspace release tests      PASS
desktop release build        PASS
manual UI smoke test         PASS
```

This does not constitute a security audit.
