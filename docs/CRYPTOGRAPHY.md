# DragonForge Cryptographic Architecture

## Current phase

Phase 3 adds the persistent local encrypted vault on top of the Phase 1 symmetric/password foundation and the Phase 2 post-quantum/hybrid layer.

## Password and Account Secret unlock derivation

The password KDF is Argon2id v1.3 with a 32-byte output. `Argon2idConfig` rejects configurations below the project's baseline:

- memory: at least 19 MiB (19,456 KiB)
- iterations: at least 2
- lanes: at least 1
- salt: at least 16 bytes

The current vault default is 64 MiB, 3 iterations, and 1 lane with a fresh 32-byte salt.

Phase 3 uses the following unlock construction:

```text
Master Password
      │
      │ Argon2id(vault salt)
      ▼
Password Key (256-bit)
      │
      │ HKDF-SHA-512
      │ salt = external 256-bit Account Secret
      │ info = "dragonforge/vault/unlock/v1"
      ▼
Unlock Key (256-bit)
      │
      └── AES-256-GCM wraps random Vault Master Key
```

The Account Secret is returned separately at vault creation and is not serialized inside the vault file.

## Vault Master Key hierarchy

Each vault gets a random 256-bit VMK.

```text
Vault Master Key
      │
      │ HKDF-SHA-512
      │ info = "dragonforge/vault/item-wrap/v1"
      ▼
Item-Wrap Key
```

Every vault item gets a new random 256-bit item key:

```text
Item-Wrap Key
      │
      ├── wraps random Item Key A
      │        └── AES-256-GCM encrypts Item A payload
      │
      ├── wraps random Item Key B
      │        └── AES-256-GCM encrypts Item B payload
      │
      └── ...
```

Updates generate a fresh item key and fresh AEAD nonce before replacing the encrypted record.

## Authenticated record binding

The wrapped item key is context-bound to the vault ID and item ID.

The encrypted item payload uses AAD containing:

```text
vault ID
item ID
record revision
```

The encrypted payload also contains its own item ID and timestamps. Decryption checks those values against the outer authenticated record metadata.

This makes record/ciphertext substitution or revision tampering fail authentication or integrity checks.

## Metadata protection

Phase 3 keeps the following inside the encrypted item payload:

- item name/title
- username
- password
- URL
- notes
- tags
- favorite flag
- item-specific data

The local vault file necessarily exposes structural metadata such as:

- vault format version
- vault ID
- KDF parameters/salt
- wrapped VMK envelope
- number of encrypted records
- opaque item UUIDs
- record revisions/timestamps
- ciphertext sizes

Search is therefore performed locally by decrypting records after unlock. Phase 3 deliberately does not create a plaintext or server-searchable index.

## Master-password changes

Changing the master password generates a new Argon2id salt and derives a new unlock key using the same external Account Secret.

Only the VMK envelope is rewrapped. Existing item ciphertexts and item keys do not need to be re-encrypted merely because the master password changed.

## Backups

Phase 3 backup export writes the already-encrypted versioned vault representation to another file.

Backup import:

1. opens the source using the supplied master password and Account Secret;
2. performs full item integrity verification;
3. refuses to overwrite an existing destination;
4. only then writes the encrypted backup to the destination.

The Account Secret is intentionally not embedded into the backup file.

## Persistent storage

The local vault is serialized as versioned JSON containing encrypted envelopes and ciphertext byte arrays.

JSON is used in Phase 3 for inspectability and testability, not because ciphertext requires text encoding. A later phase can move to a compact binary container while retaining the versioned cryptographic semantics.

Writes use a same-directory temporary file, file synchronization, backup rename, and recovery path. On Unix the temporary file is created with mode `0600`.

## Symmetric encryption

AES-256-GCM is used for authenticated symmetric encryption.

Requirements:

- 256-bit keys
- fresh 96-bit nonce for every encryption under a given key
- associated data for record/protocol identity
- authentication failure on modified ciphertext or mismatched AAD

## Secret key handling

`SecretKey` wraps 32 bytes and intentionally does not implement ordinary serialization, `Clone`, or `Copy`. Its debug output is redacted and its backing memory is zeroized on drop.

The Phase 3 `AccountSecret` is also a redacted, zeroizing 32-byte type. Explicit export returns a `Zeroizing<[u8; 32]>`.

Login passwords and secure-note bodies are held in types that zeroize their backing strings on drop.

Zeroization remains defense in depth and cannot guarantee elimination of every historical compiler/runtime copy.

## ML-KEM-768

Phase 2 implements ML-KEM-768 through RustCrypto `ml-kem`.

Current serialized sizes:

- public/encapsulation key: 1184 bytes
- private seed: 64 bytes
- ciphertext: 1088 bytes
- resulting shared secret: 32 bytes

## ML-DSA-65

Phase 2 implements ML-DSA-65 through RustCrypto `ml-dsa`.

Current serialized sizes:

- private seed: 32 bytes
- verifying key: 1952 bytes
- signature: 3309 bytes

## Hybrid X25519 + ML-KEM-768

DragonForge defines an application-level X25519 + ML-KEM-768 hybrid construction for future device enrollment and sharing protocols.

Both 32-byte shared secrets are fed into HKDF-SHA-512, and the transcript binds the caller context, recipient public keys, sender ephemeral X25519 public key, and ML-KEM ciphertext.

Non-contributory X25519 peers are rejected.

This application-level construction is **not claimed to be the TLS 1.3 X25519MLKEM768 wire format**.

## Versioning and crypto agility

Symmetric `EncryptedEnvelope` objects contain a format version and cipher-suite identifier.

The vault file itself also carries a separate vault format version. Future migrations must explicitly parse and migrate older formats rather than silently reinterpret ciphertext.

## Tests

Phase 3 integration tests cover:

- create/save/lock/unlock/reload
- login and secure-note round trips
- plaintext metadata leakage checks
- wrong-password rejection
- wrong-Account-Secret rejection
- local search
- update and delete persistence
- VMK rewrap on master-password change
- backup export/import
- ciphertext tamper detection
- password-generator character-class guarantees
- item-type round trips

Earlier Phase 1 and Phase 2 regression suites remain part of the workspace CI gate.

## Remaining non-goals

Phase 3 still does not provide:

- desktop or mobile UI
- browser extension/autofill
- attachments
- TOTP records
- passkeys
- sync/server protocols
- rollback-resistant multi-device state logs
- device enrollment workflow
- account recovery UX
- secure sharing
- standards-compliant PQ/T TLS configuration
- independent cryptographic audit
