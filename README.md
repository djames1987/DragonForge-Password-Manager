# DragonForge Password Manager

DragonForge Password Manager is a security-first, zero-knowledge password manager project designed for long-term cryptographic agility and post-quantum migration.

> **Development status:** Phase 6 — Browser Extension Foundation

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

The post-quantum implementation uses the pure-Rust RustCrypto `ml-kem` and `ml-dsa` crates rather than the older unmaintained `pqcrypto-*` bindings.

## Workspace

```text
.
├── apps/
│   ├── desktop/
│   │   ├── src/
│   │   ├── tests/
│   │   └── ui/
│   └── browser-extension/
│       ├── src/
│       ├── tests/
│       └── ui/
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
│   └── VAULT_FORMAT.md
├── scripts/
│   ├── run-phase4-tests.cmd
│   ├── run-phase4-tests.ps1
│   ├── run-phase5-tests.cmd
│   ├── run-phase5-tests.ps1
│   ├── run-phase6-tests.cmd
│   ├── run-phase6-tests.ps1
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
