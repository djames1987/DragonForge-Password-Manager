# Security Policy

## Current status

DragonForge Password Manager is under active development. Phase 1 establishes cryptographic building blocks only. The project has not yet received an independent cryptographic or application-security audit and must not be trusted with production credentials or other high-value secrets.

## Security principles

- Cryptographic primitives come from established RustCrypto/password-hashing crates; DragonForge does not implement AES, Argon2, SHA-2, or HKDF itself.
- Secret key material uses dedicated types, is never serialized by the Phase 1 API, and is zeroized on drop where the language/runtime permits.
- AEAD is required for encrypted data. Unauthenticated encryption is not supported.
- Fresh nonces are obtained from the operating-system CSPRNG for every AES-256-GCM seal operation.
- Encrypted envelopes carry explicit format and algorithm identifiers to support controlled cryptographic migration.
- Algorithm fallback and downgrade behavior must be explicit; future phases must not silently weaken an existing vault.
- Logs, error values, and debug output must never contain secret material.

## Reporting vulnerabilities

Until a dedicated private security-reporting channel is configured, do not publish exploit details in a public issue. Repository owners should enable GitHub Private Vulnerability Reporting before a public release.

When reporting a vulnerability, include the affected commit/version, reproduction steps, expected versus observed behavior, and impact. Do not include real passwords, recovery keys, or other production secrets.

## Supported versions

There is no production-supported release yet. Security fixes apply to the current development branch until a formal release policy is introduced.
