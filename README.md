# DragonForge Password Manager

DragonForge Password Manager is a security-first, zero-knowledge password manager project designed for long-term cryptographic agility and post-quantum migration.

> 📘 **[View the detailed visual product overview](docs/PRODUCT_OVERVIEW.md)** — feature tour, architecture diagrams, recovery flow, security model, examples, and current limitations.

> **Development status:** Phase 11 — Windows Credential Protection & Sidecar Secret Migration **COMPLETE; FULL WINDOWS RELEASE VERIFICATION PASSED**

## License and repository status

Copyright © 2026 David James. All rights reserved.

This repository is the historical pre-migration source and verified baseline for DragonForge Password Manager. Active Password Manager development now occurs in [DragonForge Security Suite](https://github.com/djames1987/DragonForge-Security-Suite).

The original DragonForge material in this repository is source-visible for evaluation, portfolio review, security review, and reference, but it is **not open source**. Except for rights expressly required by GitHub's Terms of Service for public repositories, no general license is granted to use, copy, modify, redistribute, sublicense, sell, commercially exploit, or incorporate original DragonForge material into another work.

See [LICENSE](LICENSE). Third-party components retain their independent licenses and rights.

## Completed phases

### Phase 1 — Cryptographic Foundation

- Argon2id password-based key derivation with validated parameters.
- AES-256-GCM authenticated encryption with fresh nonces.
- HKDF-SHA-512 domain-separated key derivation.
- Secure 256-bit secret-key type with zeroization on drop.
- OS-backed cryptographically secure random generation.
- Constant-time comparison helpers.
- Versioned encrypted envelopes.
- Domain-separated key wrapping.
- Crypto-agility traits.

### Phase 2 — Post-Quantum Cryptographic Layer

- ML-KEM-768 key generation, validation, encapsulation, decapsulation, and seed restoration.
- ML-DSA-65 signing, verification, public-key validation, and seed restoration.
- X25519 + ML-KEM-768 hybrid application-level key establishment.
- Rejection of non-contributory X25519 exchanges.
- HKDF-SHA-512 hybrid secret combiner with transcript/context binding.
- Serializable public/ciphertext types while private key containers remain non-serializable.
- Negative tests for malformed keys, modified signatures, and invalid hybrid peers.

### Phase 3 — Local Encrypted Vault

- Random 256-bit Vault Master Key (VMK).
- Master password + external 256-bit Account Secret unlock model.
- Per-item random encryption keys and encrypted login/secure-note payloads.
- Encrypted titles, usernames, passwords, URLs, notes, and tags.
- Local decrypt-then-search behavior.
- CRUD, lock/unlock, password changes, password generation, and encrypted backup/import.
- Authenticated record identity/revision binding and tamper detection.

### Phase 4 — Secure Storage Architecture / Vault Hardening

- Defensive maximum vault-file size.
- Defensive maximum item count and per-item ciphertext size.
- Upper bounds on attacker-controlled Argon2 memory, iterations, lanes, and salt size.
- Structural validation before expensive password derivation or decryption.
- Vault/item UUID validation and duplicate-ID rejection.
- Zero-revision and revision-overflow rejection.
- Wrapped-key and encrypted-payload envelope-length validation.
- Timestamp-order validation.
- Version inspection and migration-status scaffolding.
- Hardened atomic writes and backup recovery.
- Adversarial serialized-vault mutation tests.
- Storage/search/revision stress testing.
- Automated Windows verification with uploadable logs.

### Phase 5 — Desktop Application Foundation

- Native Tauri 2 desktop shell over the existing Rust vault core.
- Modern dark three-pane UI with responsive navigation, searchable item list, and detail view.
- Create/open vault flows with native file pickers.
- One-time Account Secret recovery screen after vault creation.
- Login and secure-note create/edit/delete workflows.
- Favorites, search, password generation, and password reveal/copy controls.
- Encrypted backup export and full-vault integrity verification.
- Master-password change workflow requiring the Account Secret.
- Rust-owned unlocked vault session; the frontend never owns the vault object or VMK.
- Credential-bearing command inputs are zeroized after use.
- Strict Content Security Policy and no localStorage/sessionStorage secret persistence.
- Vault-controlled content is rendered with DOM text APIs rather than injected HTML.
- Locking drops the Rust session and scrubs decrypted UI state.
- Desktop service integration tests cover create/unlock/CRUD/search/password generation/backup.
- CI builds and tests the desktop app with the rest of the workspace.

### Phase 6 — Browser Extension Foundation

- Chromium Manifest V3 extension for Google Chrome and Microsoft Edge.
- Modern popup UI that shows only site-matching login summaries.
- No broad host permissions and no persistent content scripts.
- Explicit Fill action required before a password is requested.
- Passwords are never returned by background search results.
- One-shot `chrome.scripting` injection fills visible login fields without submitting forms.
- Rust native-messaging host using browser-standard length-prefixed JSON.
- Authenticated loopback bridge from the native host to the running desktop app.
- Fresh 256-bit bridge token generated on every desktop launch.
- Desktop-side site scoping and second host check before credential release.
- Master password, Account Secret, VMK, and item-wrap key never enter the extension.
- Windows native-host registration scripts for Chrome and Edge.
- Browser extension packaging script, static/unit tests, and Phase 6 uploadable-log runner.
- Native-host installer and diagnostics validated on Windows.
- Real end-to-end credential retrieval and form filling manually verified in Microsoft Edge.
- Real end-to-end credential retrieval and form filling manually verified in Google Chrome.
- Exact Chrome/Edge extension-origin allowlisting is validated by the diagnostic script.
- CI validates the browser PowerShell installer/diagnostic scripts before Windows builds.

### Phase 7 — Sync Server Foundation

- Dedicated Rust sync-server application.
- Zero-knowledge server boundary: opaque encrypted vault bytes only.
- Separate random 256-bit sync authentication token; never derived from the master password.
- Server stores only token digests, account/vault identifiers, revision metadata, blob fingerprints, timestamps, and opaque ciphertext.
- Versioned REST API under `/v1`.
- Admin-gated development account provisioning.
- Exact account isolation between synchronized vault objects.
- Optimistic concurrency using base revisions and HTTP 409 conflicts.
- Monotonic server revisions.
- 64 MiB encrypted sync-payload limit.
- In-memory store for deterministic tests/development.
- PostgreSQL persistence implementation and checked-in migration.
- PostgreSQL row locking for atomic revision compare/update.
- Loopback-only default bind.
- Deployment documentation requiring HTTPS/TLS termination for remote use.
- Phase 7 API integration tests and uploadable Windows verification runner.
- CI compiles/tests the sync server with PostgreSQL support on Linux and Windows.

### Phase 8 — Multi-Device Sync

- Desktop synchronization client for the Phase 7 zero-knowledge sync server.
- Synchronizes the already-encrypted `.dfvault` file; the server never receives decrypted vault items.
- Per-vault sync sidecar stores server URL, bearer token, last server revision, and last synchronized ciphertext hash.
- HTTPS required for remote servers; plaintext HTTP allowed only for loopback development.
- Safe decision engine distinguishes upload, download, up-to-date, rollback/mismatch, and true conflict states.
- First sync never overwrites an existing remote vault without an explicit user choice.
- Newer remote vaults must pass server-hash, structural, vault-ID, and AEAD item authentication before replacement.
- Remote pulls lock the local vault before encrypted-file replacement.
- Explicit **Keep Local** and **Keep Remote** conflict resolution.
- Atomic Windows-safe updates for sync metadata and remote vault replacement.
- Modern desktop sync controls integrated into the existing settings UI and sidebar.
- Real two-device integration tests against the actual Phase 7 Axum server.
- Phase 8 verification runner and Linux/Windows CI coverage.

### Phase 9 — Device Enrollment

- ML-DSA-65 device signing identities generated locally per synchronized desktop.
- Enrollment/rename requests include an ML-DSA proof-of-possession binding device UUID, name, and verifying key.
- Sync protocol version 2 with signed device authorization.
- First enrolled device establishes the initial trusted device.
- Additional devices enter a pending state and require approval by an active device.
- Active-device signed approval and revocation decisions.
- Pending and revoked devices are denied encrypted-vault access.
- Revocation is terminal at the persistence layer; a revoked device identity cannot be reactivated by a stale/concurrent approval.
- Vault GET/PUT signatures bind method, path, timestamp, ciphertext hash, and base revision.
- Five-minute request timestamp freshness window.
- Reusing a device UUID with a different public key is rejected.
- Revoking devices cannot reset the account into a new first-device bootstrap state.
- Existing Phase 8 sidecars migrate from configuration version 1 to version 2.
- Pre-enrollment bearer-only sync remains available only while an account has zero device records.
- PostgreSQL device-enrollment schema and in-memory test implementation.
- Desktop device enrollment, device listing, approval, rename, and revocation controls.
- Dedicated Phase 9 lifecycle test and Windows verification runner.

### Phase 10 — Secure Account & Device Recovery

- Offline ML-DSA-65 recovery identities with private recovery seeds kept only in user-held recovery kits.
- Server stores only the recovery verifying key and a client-encrypted Account Secret envelope.
- Recovery authorization binds account, vault, generation, timestamp, and a fresh 256-bit nonce.
- Recovery-only access retrieves the already-encrypted remote vault without exposing plaintext to the server.
- The recovered Account Secret is decrypted only on the replacement desktop.
- Master-password verification occurs locally before server trust is changed or the recovery kit is consumed.
- Successful recovery atomically revokes previous devices, activates a fresh replacement device, rotates the sync token, and rotates the recovery key.
- Recovery generations invalidate used or superseded recovery kits.
- Replacement devices and replacement recovery keys both prove possession of their ML-DSA private keys.
- PostgreSQL recovery persistence and in-memory deterministic test support.
- Desktop UI for recovery-kit creation/rotation and lost-device disaster recovery.
- Focused server and desktop end-to-end recovery tests plus a Phase 10 Windows verification runner.

### Phase 11 — Windows Credential Protection & Sidecar Secret Migration

- Sync-sidecar format upgraded to version 3.
- On Windows, the 256-bit sync bearer token is stored in Windows Credential Manager rather than the sidecar.
- On Windows, the ML-DSA-65 device signing seed is stored in the same protected credential bundle.
- Version-3 Windows sidecars contain only non-secret sync metadata and a random credential reference UUID.
- Version-3 Windows sidecars containing plaintext sync secrets are rejected.
- Existing Phase 8/9/10 sidecars migrate automatically and fail closed if Credential Manager storage is unavailable.
- Sync reconfiguration removes superseded Credential Manager entries.
- Remove Sync removes the corresponding Credential Manager entry.
- Phase 10 recovered/rotated sync credentials are installed through the protected Phase 11 storage path.
- The desktop sync status reports whether secrets are protected by Windows Credential Manager or still use legacy sidecar storage on unsupported platforms.
- Pure sidecar-redaction tests and a real Windows Credential Manager integration test were added.
- Phase 11 Windows verification runner and CI gates were added.

The post-quantum implementation uses the pure-Rust RustCrypto `ml-kem` and `ml-dsa` crates rather than the older unmaintained `pqcrypto-*` bindings.

## Workspace

```text
.
├── apps/
│   ├── desktop/
│   │   ├── src/
│   │   ├── tests/
│   │   └── ui/
│   ├── browser-extension/
│   │   ├── src/
│   │   ├── tests/
│   │   └── ui/
│   └── sync-server/
│       ├── migrations/
│       ├── src/
│       └── tests/
├── crates/
│   ├── dragonforge-crypto/
│   └── dragonforge-vault/
├── docs/
│   ├── CRYPTOGRAPHY.md
│   ├── PHASE2_TESTING.md
│   ├── PHASE3_TESTING.md
│   ├── PHASE4_TESTING.md
│   ├── PHASE5_DESKTOP.md
│   ├── PHASE5_TESTING.md
│   ├── PHASE6_BROWSER.md
│   ├── PHASE6_TESTING.md
│   ├── PHASE7_SYNC_SERVER.md
│   ├── PHASE7_TESTING.md
│   ├── PHASE8_MULTI_DEVICE_SYNC.md
│   ├── PHASE8_TESTING.md
│   ├── PHASE9_DEVICE_ENROLLMENT.md
│   ├── PHASE9_TESTING.md
│   ├── PHASE10_ACCOUNT_RECOVERY.md
│   ├── PHASE10_TESTING.md
│   ├── PHASE11_CREDENTIAL_PROTECTION.md
│   ├── PHASE11_TESTING.md
│   └── VAULT_FORMAT.md
├── scripts/
│   ├── run-phase4-tests.cmd
│   ├── run-phase4-tests.ps1
│   ├── run-phase5-tests.cmd
│   ├── run-phase5-tests.ps1
│   ├── run-phase6-tests.cmd
│   ├── run-phase6-tests.ps1
│   ├── run-phase7-tests.cmd
│   ├── run-phase7-tests.ps1
│   ├── run-phase8-tests.cmd
│   ├── run-phase8-tests.ps1
│   ├── run-phase9-tests.cmd
│   ├── run-phase9-tests.ps1
│   ├── run-phase10-tests.cmd
│   ├── run-phase10-tests.ps1
│   ├── run-phase11-tests.cmd
│   ├── run-phase11-tests.ps1
│   └── install-browser-native-host.ps1
└── .github/workflows/ci.yml
```

## Run the desktop application

On Windows, after installing the Rust toolchain and Microsoft C++ build tools/WebView2 prerequisites:

```powershell
cargo run -p dragonforge-desktop
```

The Phase 5 frontend is checked-in HTML/CSS/JavaScript, so there is no Node/npm build step.

See [docs/PHASE5_DESKTOP.md](docs/PHASE5_DESKTOP.md) for the desktop architecture.

## Automated Phase 5 verification

After pulling the repository:

```powershell
.\scripts\run-phase5-tests.ps1
```

or:

```text
scripts\run-phase5-tests.cmd
```

The runner performs the noninteractive quality gates, workspace tests, desktop integration tests, Phase 4 regression tests, optimized release tests, and a release desktop build. It creates:

```text
test-logs\dragonforge-phase5-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase5-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE5_TESTING.md](docs/PHASE5_TESTING.md) for the automated and manual verification checklist.

## Manual commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p dragonforge-desktop --test desktop_service -- --nocapture
cargo test -p dragonforge-vault --test hardening -- --test-threads=1
cargo test --workspace --all-features --release
cargo build -p dragonforge-desktop --release
```

## Security status

DragonForge is still under active development and has **not** undergone an independent cryptographic or application-security audit. It should not yet be trusted with production credentials or other high-value secrets.

See [SECURITY.md](SECURITY.md), [docs/CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md), and [docs/VAULT_FORMAT.md](docs/VAULT_FORMAT.md).


## Browser extension development

Load `apps/browser-extension` as an unpacked extension in Chrome or Edge. Then register the native host using the extension ID shown by the browser:

```powershell
.\scripts\install-browser-native-host.ps1 -EdgeExtensionId <ID>
```

For Chrome, use `-ChromeExtensionId`.

The desktop application must be running and the vault must be unlocked before browser credentials can be searched or filled.

See [docs/PHASE6_BROWSER.md](docs/PHASE6_BROWSER.md) for the security architecture.

## Automated Phase 6 verification

```powershell
.\scripts\run-phase6-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase6-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase6-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE6_TESTING.md](docs/PHASE6_TESTING.md).


## Run the Phase 7 sync server

Development/in-memory mode:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "phase7-development-admin-token-32bytes-minimum"
cargo run -p dragonforge-sync-server
```

PostgreSQL-backed mode:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "<long-random-admin-token>"
$env:DRAGONFORGE_SYNC_DATABASE_URL = "postgres://user:password@127.0.0.1/dragonforge"
cargo run -p dragonforge-sync-server --features postgres
```

The server binds to `127.0.0.1:8787` by default. Remote deployment requires HTTPS/TLS termination.

See [docs/PHASE7_SYNC_SERVER.md](docs/PHASE7_SYNC_SERVER.md).

## Automated Phase 7 verification

```powershell
.\scripts\run-phase7-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase7-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase7-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE7_TESTING.md](docs/PHASE7_TESTING.md).


## Phase 8 multi-device sync

Start a development sync server and provision a test account, then configure the server URL and returned sync token in the desktop application's **Vault settings → Multi-device sync** section.

See [docs/PHASE8_MULTI_DEVICE_SYNC.md](docs/PHASE8_MULTI_DEVICE_SYNC.md).

## Automated Phase 8 verification

```powershell
.\scripts\run-phase8-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase8-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase8-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE8_TESTING.md](docs/PHASE8_TESTING.md).


## Phase 9 device enrollment

After sync is configured, the first device can enroll itself. Additional devices using the same account sync token are created as pending and must be approved from an already-active device.

See [docs/PHASE9_DEVICE_ENROLLMENT.md](docs/PHASE9_DEVICE_ENROLLMENT.md).

## Automated Phase 9 verification

```powershell
.\scripts\run-phase9-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase9-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase9-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE9_TESTING.md](docs/PHASE9_TESTING.md).


## Phase 10 secure account recovery

After synchronization and device enrollment are configured, open **Vault settings → Secure account recovery** and create an offline recovery kit. The kit is required if every authorized device is lost.

To recover on a replacement computer, choose **Recover synchronized vault** on the welcome screen and provide the new local vault location, sync-server URL, recovery kit, and master password.

A successful recovery rotates the sync token and recovery kit and revokes all previous device authorizations.

See [docs/PHASE10_ACCOUNT_RECOVERY.md](docs/PHASE10_ACCOUNT_RECOVERY.md).

## Automated Phase 10 verification

```powershell
.\scripts\run-phase10-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase10-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase10-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE10_TESTING.md](docs/PHASE10_TESTING.md).

## Phase 11 Windows credential protection

On Windows, opening/configuring a synchronized vault migrates the sync bearer token and ML-DSA device signing seed out of the per-vault sidecar and into Windows Credential Manager. The sidecar is upgraded to version 3 and retains only non-secret synchronization metadata plus a random credential reference.

See [docs/PHASE11_CREDENTIAL_PROTECTION.md](docs/PHASE11_CREDENTIAL_PROTECTION.md).

## Automated Phase 11 verification

```powershell
.\scripts\run-phase11-tests.ps1
```

The runner creates:

```text
test-logs\dragonforge-phase11-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase11-YYYYMMDD-HHMMSS.log.sha256
```

See [docs/PHASE11_TESTING.md](docs/PHASE11_TESTING.md).

## Workspace cleanup

Repeated Rust debug/release/test builds can make the workspace `target` directory very large. DragonForge includes a conservative cleanup/report helper:

```powershell
.\scripts\cleanup-workspace.ps1
```

It is dry-run by default. To reclaim Cargo build artifacts:

```powershell
.\scripts\cleanup-workspace.ps1 -CleanCargoTarget -Apply
```

See [docs/WORKSPACE_CLEANUP.md](docs/WORKSPACE_CLEANUP.md) for retention and safety details.