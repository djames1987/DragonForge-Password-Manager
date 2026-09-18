# Security Policy

## Current status

DragonForge Password Manager is under active development through **Phase 4 — Secure Storage Architecture / Vault Hardening**.

The project contains working cryptographic primitives, post-quantum/hybrid components, and a persistent local encrypted vault with defensive parsing/storage controls. It has **not** received an independent cryptographic or application-security audit and must not yet be trusted with production credentials or other high-value secrets.

## Security principles

- Cryptographic primitives come from established libraries; DragonForge does not implement AES, Argon2, SHA-2, HKDF, ML-KEM, ML-DSA, or X25519 primitives itself.
- The master password never directly encrypts vault items.
- A random VMK is wrapped by an unlock key derived from the password and external Account Secret.
- The Account Secret is not stored inside the vault file.
- Each item has an independent random encryption key.
- Sensitive item metadata remains encrypted.
- AEAD is mandatory and record identity/revision data is authenticated.
- Attacker-controlled KDF metadata is bounded before Argon2 execution.
- Vault file size, item count, ciphertext size, KDF salt size, memory, iteration, and lane counts are bounded.
- Vault and item identifiers are structurally validated before unlock work.
- Duplicate IDs, zero revisions, malformed wrapped-key sizes, and unsupported versions are rejected.
- Revision increments use checked arithmetic.
- Atomic persistence uses same-directory temporary and backup paths; Unix writes additionally synchronize the parent directory after rename operations.
- A missing live vault may recover from the last complete backup, but orphan temporary files are never trusted as vaults.
- Backup imports verify credentials and full vault integrity before creating the destination.
- Logs and debug output must never expose secret keys, master passwords, Account Secrets, or decrypted vault payloads.

## Known limitations

- Phase 4 remains local-only; there is no sync protocol or rollback-resistant multi-device state log.
- An attacker who can replace a valid vault with an older valid copy can still cause local rollback; authenticated remote state/version anchoring is a later-phase concern.
- JSON parsing necessarily allocates memory before all semantic checks, though Phase 4 caps the input file at 64 MiB.
- The current format supports inspection/migration dispatch scaffolding, but there is no older DragonForge vault format to migrate yet.
- Filesystem atomicity and durability guarantees still depend on operating-system/filesystem behavior.
- Zeroization is defense in depth and cannot guarantee erasure of every historical runtime/compiler copy.
- A compromised endpoint can read secrets while the vault is legitimately unlocked.
- No independent audit, penetration test, fuzzing campaign, or formal side-channel review has been completed.
- The Account Secret recovery/storage experience has not yet been integrated with hardware-backed secure storage.

## Reporting vulnerabilities

Until a dedicated private security-reporting channel is configured, do not publish exploit details in a public issue.

When reporting a vulnerability, include the affected commit/version, reproduction steps, expected versus observed behavior, and impact. Do not include real passwords, recovery keys, Account Secrets, or production secrets.

## Supported versions

There is no production-supported release yet. Security fixes apply to the current development branch until a formal release policy is introduced.
