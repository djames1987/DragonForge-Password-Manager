# DragonForge Password Manager Security Policy

## Current status

This repository is the **historical pre-migration standalone baseline** for DragonForge Password Manager. Active Password Manager development now occurs in [DragonForge Security Suite](https://github.com/djames1987/DragonForge-Security-Suite).

The standalone baseline contains substantial cryptographic, desktop, browser-integration, sync, and device-enrollment work, but it has not received an independent cryptographic or application-security audit and should not be treated as a production-supported password-manager release.

## Reporting vulnerabilities

For a vulnerability that specifically affects this historical repository, use GitHub's private security-advisory reporting flow when available:

`https://github.com/djames1987/DragonForge-Password-Manager/security/advisories/new`

For vulnerabilities affecting the current Password Manager implementation, report them privately to the canonical Security Suite repository instead:

`https://github.com/djames1987/DragonForge-Security-Suite/security/advisories/new`

If private advisory reporting is unavailable, contact the repository owner through a private GitHub channel. Do **not** publish exploit details in a public issue.

Do not include real passwords, recovery keys, Account Secrets, OTP seeds, private signing keys, sync bearer tokens, production vaults, personal data, or other live secrets. Use synthetic test material and sanitized evidence.

## Security boundaries preserved in this baseline

The historical implementation uses established cryptographic libraries, keeps the master password separate from direct item encryption, encrypts sensitive item metadata, authenticates encrypted records, bounds attacker-controlled KDF and payload parameters, keeps unlocked vault state in Rust-owned application state, limits browser-extension permissions, and treats remote sync storage as opaque encrypted vault state.

These design properties do not constitute an independent audit or security guarantee. A compromised endpoint can still access secrets while the vault is legitimately unlocked, and historical limitations documented in the repository remain applicable.

## Supported versions

There is no production-supported standalone release line. Security fixes and new development belong in the canonical Security Suite unless the owner explicitly resumes standalone development.