# Security Policy

## Current status

DragonForge Password Manager is under active development through **Phase 8 — Multi-Device Sync**.

The project contains working cryptographic primitives, post-quantum/hybrid components, a hardened persistent local vault, a Tauri desktop application, a Chromium browser-extension bridge verified on Windows with Microsoft Edge and Google Chrome, and a zero-knowledge sync-server foundation. It has **not** received an independent cryptographic or application-security audit and must not yet be trusted with production credentials or other high-value secrets.

## Core security principles

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
- Backup imports verify credentials and full vault integrity before creating the destination.
- Logs and debug output must never expose secret keys, master passwords, Account Secrets, or decrypted vault payloads.

## Desktop security boundary

- The unlocked Vault/VMK remains in Rust-owned application state, not JavaScript application state.
- The desktop frontend does not retain the master password or Account Secret after an operation completes.
- Credential-bearing command inputs are zeroized after use.
- Locking drops the Rust vault session and clears decrypted item/editor/settings values from the webview DOM.
- Vault-controlled text is assigned through DOM text APIs instead of HTML injection.
- The desktop CSP blocks arbitrary remote scripts/styles and has no remote content dependency.

## Browser-extension security boundary

- Phase 6 uses Manifest V3.
- The Chromium extension requests only `activeTab`, `scripting`, and `nativeMessaging`.
- The manifest has no broad `host_permissions` and no persistent `content_scripts`.
- Browser search responses never contain passwords.
- A password is requested only after the user explicitly clicks **Fill**.
- Immediately before password retrieval/fill, the active tab URL is re-checked.
- The desktop service independently verifies that the requested login belongs to the active site.
- The extension never receives the master password, Account Secret, VMK, item-wrap key, or the desktop bridge authentication token.
- The native messaging host does not open vault files or derive vault keys.
- The native host communicates with the running desktop process through a localhost-only endpoint authenticated with a fresh random 256-bit token generated on each desktop launch.
- Native bridge messages are size-limited and protocol-versioned.
- The injected fill function is one-shot and does not automatically submit forms.

## Sync-server security boundary

- The sync server stores opaque encrypted vault bytes and does not parse vault plaintext.
- Master passwords, Account Secrets, VMKs, item-wrap keys, and browser bridge tokens are not sync-server authentication material.
- Sync authentication uses a separate random 256-bit bearer token.
- Only the SHA-256 digest of the sync token is persisted.
- Vault writes require the caller's last-known server revision.
- Stale writes are rejected with HTTP 409 instead of overwriting newer state.
- PostgreSQL writes lock the target vault row before revision comparison/update.
- Synchronized payloads are limited to 64 MiB.
- The default server bind is loopback-only.
- Remote deployment requires HTTPS/TLS termination; Phase 7 does not include built-in TLS.
- Account provisioning is disabled unless a server admin token is configured.
- Server logs must not contain bearer tokens, admin tokens, or synchronized ciphertext bodies.
- Phase 8 desktop synchronization compares local ciphertext hashes with the last synchronized hash and the server revision before modifying either side.
- A remote pull verifies the server SHA-256, validates the encrypted vault structure and vault UUID, authenticates every encrypted item with the unlocked vault keys, locks the active desktop vault, then atomically replaces the local encrypted file.
- If both local and remote state changed, automatic synchronization stops and requires an explicit Keep Local or Keep Remote choice.
- Server revision rollback, same-revision ciphertext mismatch, and disappearance of a previously synchronized remote vault are treated as safety failures rather than normal updates.
- Non-loopback sync endpoints must use HTTPS. Plain HTTP is accepted only for local development on loopback addresses.

## Desktop-specific limitations

- Decrypted item values necessarily exist in webview memory while displayed or edited. Locking clears application references but cannot prove physical erasure of every runtime copy.
- Copying a password or Account Secret to the system clipboard may expose it to other local applications or clipboard-history features. Timed clipboard clearing is not yet implemented.
- Automatic inactivity lock is not implemented yet.
- Phase 8 stores the per-vault sync bearer token in a local sidecar file. On Unix the sidecar is created with mode 0600; on Windows it inherits the current user's filesystem ACLs. Hardware/OS-backed secret storage is not implemented yet.
- A user or malware process with access to the current OS account may be able to read the sync sidecar token and impersonate that device to the sync service.
- Hardware-backed unlock (Windows Hello/TPM, Keychain/Secure Enclave, etc.) is not yet implemented.
- The desktop app is not yet code-signed or distributed through a production installer.

## Browser-specific limitations

- Passwords necessarily exist briefly inside the browser extension service worker and the target page process during an explicit fill.
- A compromised or malicious webpage can read credentials after they have been intentionally filled into that page.
- Phase 6 does not automatically submit forms.
- Phase 6 does not support save/update prompts after login submission.
- Phase 6 does not yet support Firefox native-host packaging or Safari.
- The native-host registration script is intended for development/sideloaded extension IDs. Store-published IDs will need production installer manifests.
- The bridge endpoint file is protected by per-user filesystem placement/permissions, but malware running as the same OS user remains inside the endpoint-compromise threat model.
- A compromised browser, extension process, desktop process, or operating-system account can defeat browser integration protections.

## Known limitations

- Phase 8 does not yet provide rollback-resistant signed multi-device history, device signatures, automatic background synchronization, or field-level conflict merging.
- An attacker who can replace a valid vault with an older valid copy can still cause local rollback.
- Filesystem atomicity and durability guarantees still depend on OS/filesystem behavior.
- Zeroization is defense in depth and cannot guarantee erasure of every historical runtime/compiler copy.
- A compromised endpoint can read secrets while the vault is legitimately unlocked.
- No independent audit, penetration test, fuzzing campaign, or formal side-channel review has been completed.
- The Account Secret recovery/storage experience has not yet been integrated with hardware-backed secure storage.

## Reporting vulnerabilities

Until a dedicated private security-reporting channel is configured, do not publish exploit details in a public issue.

When reporting a vulnerability, include the affected commit/version, reproduction steps, expected versus observed behavior, and impact. Do not include real passwords, recovery keys, Account Secrets, or production secrets.

## Supported versions

There is no production-supported release yet. Security fixes apply to the current development branch until a formal release policy is introduced.
