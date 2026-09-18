# DragonForge Password Manager

DragonForge Password Manager is a security-first, zero-knowledge password manager project designed for long-term cryptographic agility and post-quantum migration.

> **Development status:** Phase 3 — Local Encrypted Vault

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

- New `dragonforge-vault` crate separated from the primitive crypto layer.
- Random 256-bit Vault Master Key (VMK).
- Master password + external 256-bit Account Secret unlock model.
- Argon2id password derivation followed by HKDF-SHA-512 account-secret binding.
- VMK wrapping under the derived unlock key.
- HKDF-derived item-wrapping key from the VMK.
- Per-item random 256-bit data-encryption keys.
- AES-256-GCM encryption for each item payload.
- Per-item key wrapping underneath the vault item-wrap key.
- Authenticated item identity/revision binding through AAD.
- Login and secure-note records.
- Encrypted names, usernames, passwords, URLs, notes, and tags.
- Local decrypt-then-search behavior; no plaintext search index.
- Add, read, update, delete, list, and search APIs.
- Automatic persistence after mutations.
- Master-password changes by rewrapping the VMK rather than re-encrypting every item.
- Lock/unlock flow.
- Password generator using OS CSPRNG and unbiased rejection sampling.
- Encrypted backup export/import with credential and integrity validation.
- Crash-recoverable temp/backup file replacement.
- Refusal to silently overwrite an existing vault.
- Ciphertext tamper detection and full-vault integrity verification.

The post-quantum implementation uses the pure-Rust RustCrypto `ml-kem` and `ml-dsa` crates rather than the older unmaintained `pqcrypto-*` bindings.

## Workspace

```text
.
├── crates/
│   ├── dragonforge-crypto/
│   │   ├── src/
│   │   └── tests/
│   └── dragonforge-vault/
│       ├── src/
│       │   ├── error.rs
│       │   ├── lib.rs
│       │   ├── model.rs
│       │   ├── password.rs
│       │   ├── storage.rs
│       │   └── vault.rs
│       └── tests/
│           └── local_vault.rs
├── docs/
│   ├── CRYPTOGRAPHY.md
│   ├── PHASE2_TESTING.md
│   ├── PHASE3_TESTING.md
│   └── VAULT_FORMAT.md
├── .github/workflows/
│   └── ci.yml
├── Cargo.toml
├── SECURITY.md
└── README.md
```

## Build and test

Install a current stable Rust toolchain, then run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --all-features --release
```

Phase-specific suites:

```bash
cargo test -p dragonforge-crypto --test foundation
cargo test -p dragonforge-crypto --test post_quantum
cargo test -p dragonforge-vault --test local_vault
```

See [docs/PHASE3_TESTING.md](docs/PHASE3_TESTING.md) for the recommended Windows/local verification procedure and [docs/VAULT_FORMAT.md](docs/VAULT_FORMAT.md) for the Phase 3 key hierarchy and file format.

## Security status

DragonForge is still under active development and has **not** undergone an independent cryptographic or application-security audit. It should not yet be trusted with production credentials or other high-value secrets.

See [SECURITY.md](SECURITY.md) and [docs/CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md).

## Design principle

The primitive cryptography and the vault/application layer are deliberately separated. The vault crate consumes narrow APIs from `dragonforge-crypto`, which keeps future algorithm migrations and independent reviews tractable.

Future phases will add richer item types, attachments/TOTP, desktop applications, browser integration, synchronization, device enrollment, recovery, secure sharing, and user-facing security controls.
