# DragonForge Password Manager

DragonForge Password Manager is a security-first, zero-knowledge password manager project designed for long-term cryptographic agility and post-quantum migration.

> **Development status:** Phase 1 — Cryptographic Foundation

## Phase 1 scope

Phase 1 establishes the reusable Rust cryptographic core that future vault, synchronization, sharing, desktop, browser, and mobile components will depend on.

Implemented in this phase:

- Argon2id password-based key derivation with explicit, validated parameters.
- AES-256-GCM authenticated encryption with fresh 96-bit nonces.
- HKDF-SHA-512 key derivation with caller-supplied domain separation.
- Secure 256-bit secret-key type with memory zeroization on drop.
- OS-backed cryptographically secure random generation.
- Constant-time byte comparison helper.
- Versioned encrypted-envelope format with explicit cipher-suite identifier.
- Domain-separated key wrapping and unwrapping.
- Abstraction traits for KDF, AEAD, key derivation, and random sources.
- Integration tests for round trips, tamper rejection, AAD binding, domain separation, and parameter validation.
- GitHub Actions checks for formatting, Clippy, and tests.

Post-quantum KEMs and signatures are intentionally **not** part of Phase 1. They belong to Phase 2 so that the symmetric/password foundation can be reviewed independently.

## Workspace

```text
.
├── crates/
│   └── dragonforge-crypto/
├── docs/
│   └── CRYPTOGRAPHY.md
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

## Security status

This project is under active development and has **not** undergone an independent cryptographic/security audit. It should not yet be trusted with production secrets.

See [SECURITY.md](SECURITY.md) and [docs/CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md).

## Design principle

DragonForge treats cryptographic agility as a core requirement. Encrypted records carry explicit version and suite identifiers, while application code depends on narrow cryptographic interfaces instead of scattering primitive-specific calls throughout the codebase.

Future phases will add the vault key hierarchy, post-quantum/hybrid KEMs and signatures, local encrypted vault storage, synchronization, device enrollment, recovery, secure sharing, and user-facing applications.
