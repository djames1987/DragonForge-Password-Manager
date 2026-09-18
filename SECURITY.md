# Security Policy

## Current status

DragonForge Password Manager is under active development through **Phase 3 — Local Encrypted Vault**.

The project now contains working cryptographic primitives, post-quantum/hybrid key-establishment components, and a persistent local encrypted vault. It has **not** received an independent cryptographic or application-security audit and must not yet be trusted with production credentials or other high-value secrets.

## Security principles

- Cryptographic primitives come from established RustCrypto/password-hashing crates; DragonForge does not implement AES, Argon2, SHA-2, HKDF, ML-KEM, ML-DSA, or X25519 primitives itself.
- The master password never directly encrypts vault items.
- A random Vault Master Key (VMK) is wrapped by an unlock key derived from the password and an external Account Secret.
- The Account Secret is not stored inside the vault file.
- Each vault item receives a fresh random item key; item payloads are encrypted independently.
- Item names, usernames, passwords, URLs, notes, and tags are encrypted rather than stored as plaintext metadata.
- AEAD is required for encrypted data. Unauthenticated encryption is not supported.
- Fresh nonces are obtained from the operating-system CSPRNG for AES-256-GCM encryption operations.
- Record identity and revision data are bound through authenticated associated data.
- Secret key material uses dedicated types and is zeroized on drop where the language/runtime permits.
- Login passwords and secure-note bodies use zeroizing payload types in memory.
- Private ML-KEM/ML-DSA/X25519 containers are not general-purpose Serde-serializable.
- Encrypted envelopes and vault files carry explicit versions for controlled migrations.
- Vault creation and backup import refuse to silently overwrite existing vault files.
- Backup credentials and vault integrity are checked before an imported backup is written to its destination.
- Algorithm fallback or downgrade behavior must be explicit; DragonForge must not silently weaken an existing vault.
- Logs, error values, and debug output must never contain secret key material, passwords, Account Secrets, or decrypted vault payloads.

## Known limitations

- Phase 3 is local-only; there is no hardened sync protocol or rollback-resistant multi-device state log yet.
- File-system crash recovery is implemented with same-directory temporary/backup replacement, but it is not a substitute for a transactional database or filesystem.
- Zeroization is defense in depth and cannot guarantee erasure of every historical compiler/runtime copy.
- A compromised endpoint can read secrets while the user legitimately has the vault unlocked.
- No independent audit, penetration test, or formal side-channel review has been completed.
- The current Account Secret storage/user-recovery experience has not yet been implemented in a GUI or hardware-backed secure store.

## Reporting vulnerabilities

Until a dedicated private security-reporting channel is configured, do not publish exploit details in a public issue. Repository owners should enable GitHub Private Vulnerability Reporting before a public release.

When reporting a vulnerability, include the affected commit/version, reproduction steps, expected versus observed behavior, and impact. Do not include real passwords, recovery keys, Account Secrets, or other production secrets.

## Supported versions

There is no production-supported release yet. Security fixes apply to the current development branch until a formal release policy is introduced.
