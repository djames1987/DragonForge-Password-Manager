# DragonForge Password Manager

DragonForge Password Manager is a security-first, zero-knowledge password manager project designed for long-term cryptographic agility and post-quantum migration.

> **Development status:** Phase 4 — Secure Storage Architecture / Vault Hardening

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
- Vault/item UUID validation.
- Duplicate item-ID rejection.
- Zero-revision and revision-overflow rejection.
- Wrapped-key and encrypted-payload envelope-length validation.
- Timestamp-order validation.
- Version inspection API and explicit migration-status scaffolding.
- Current-format rejection of unknown/future versions.
- Hardened atomic writes with parent-directory synchronization on Unix.
- Recovery from the last complete backup if the live file is missing.
- Refusal to promote orphan temporary files as valid vaults.
- Adversarial serialized-vault mutation tests.
- 100-item encrypted storage/search stress test.
- 50-consecutive-update revision stress test.
- Automated Windows verification script with timestamped uploadable logs.
- CI now runs debug workspace tests, Phase 4 hardening serially, and optimized release tests.

The post-quantum implementation uses the pure-Rust RustCrypto `ml-kem` and `ml-dsa` crates rather than the older unmaintained `pqcrypto-*` bindings.

## Workspace

```text
.
├── crates/
│   ├── dragonforge-crypto/
│   └── dragonforge-vault/
│       ├── src/
│       │   ├── error.rs
│       │   ├── format.rs
│       │   ├── limits.rs
│       │   ├── model.rs
│       │   ├── password.rs
│       │   ├── storage.rs
│       │   └── vault.rs
│       └── tests/
│           ├── hardening.rs
│           └── local_vault.rs
├── scripts/
│   ├── run-phase4-tests.cmd
│   └── run-phase4-tests.ps1
├── docs/
│   ├── CRYPTOGRAPHY.md
│   ├── PHASE2_TESTING.md
│   ├── PHASE3_TESTING.md
│   ├── PHASE4_TESTING.md
│   └── VAULT_FORMAT.md
└── .github/workflows/ci.yml
```

## Automated Windows verification

After pulling the repository, run:

```powershell
.\scripts\run-phase4-tests.ps1
```

or double-click/run:

```text
scripts\run-phase4-tests.cmd
```

The script runs all noninteractive quality gates and tests, repeats the Phase 4 hardening suite three times by default, runs release-mode tests, and creates:

```text
test-logs\dragonforge-phase4-YYYYMMDD-HHMMSS.log
test-logs\dragonforge-phase4-YYYYMMDD-HHMMSS.log.sha256
```

Upload the `.log` file when you want the results reviewed.

See [docs/PHASE4_TESTING.md](docs/PHASE4_TESTING.md).

## Manual commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p dragonforge-vault --test hardening -- --test-threads=1
cargo test --workspace --all-features --release
```

## Security status

DragonForge is still under active development and has **not** undergone an independent cryptographic or application-security audit. It should not yet be trusted with production credentials or other high-value secrets.

See [SECURITY.md](SECURITY.md), [docs/CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md), and [docs/VAULT_FORMAT.md](docs/VAULT_FORMAT.md).
