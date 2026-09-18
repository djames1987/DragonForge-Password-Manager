# DragonForge Password Manager

DragonForge Password Manager is a security-first, zero-knowledge password manager project designed for long-term cryptographic agility and post-quantum migration.

> **Development status:** Phase 2 — Post-Quantum Cryptographic Layer

## Completed foundation

### Phase 1

- Argon2id password-based key derivation with validated parameters.
- AES-256-GCM authenticated encryption with fresh nonces.
- HKDF-SHA-512 domain-separated key derivation.
- Secure 256-bit secret-key type with zeroization on drop.
- OS-backed cryptographically secure random generation.
- Constant-time comparison helpers.
- Versioned encrypted envelopes.
- Domain-separated key wrapping.
- Crypto-agility traits.

### Phase 2

- ML-KEM-768 key generation, public-key validation, encapsulation, decapsulation, and private-seed restoration.
- ML-DSA-65 signing, verification, public-key validation, signature parsing, and private-seed restoration.
- X25519 + ML-KEM-768 hybrid key establishment.
- Ephemeral X25519 sender keys.
- Long-term recipient hybrid public keys.
- Rejection of non-contributory X25519 exchanges.
- HKDF-SHA-512 hybrid secret combiner.
- Transcript binding of the recipient key, sender ephemeral key, ML-KEM ciphertext, and protocol context.
- Serializable public/ciphertext types while keeping secret-key containers non-serializable.
- Redacted debug output for private key containers.
- Integration tests for successful and hostile/failure cases.

The post-quantum implementation uses the current pure-Rust RustCrypto `ml-kem` and `ml-dsa` crates rather than the older unmaintained `pqcrypto-*` bindings.

## Workspace

```text
.
├── crates/
│   └── dragonforge-crypto/
│       ├── src/
│       │   ├── pq_kem.rs
│       │   ├── pq_sign.rs
│       │   └── hybrid.rs
│       └── tests/
│           ├── foundation.rs
│           └── post_quantum.rs
├── docs/
│   ├── CRYPTOGRAPHY.md
│   └── PHASE2_TESTING.md
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
```

For Phase 2-specific tests:

```bash
cargo test -p dragonforge-crypto --test post_quantum
```

See [docs/PHASE2_TESTING.md](docs/PHASE2_TESTING.md) for the recommended local verification procedure.

## Security status

This project is under active development and has **not** undergone an independent cryptographic/security audit. The upstream RustCrypto ML-KEM and ML-DSA crates also document that they have not been independently audited. DragonForge should therefore not yet be trusted with production secrets.

See [SECURITY.md](SECURITY.md) and [docs/CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md).

## Design principle

DragonForge treats cryptographic agility as a core requirement. Higher-level application code should depend on the narrow cryptographic APIs exposed by `dragonforge-crypto`, allowing future algorithm migrations without rewriting the vault and application layers.

Future phases will build the local vault, vault key hierarchy, encrypted persistence, synchronization, device enrollment, recovery, secure sharing, and user-facing applications on top of this foundation.
