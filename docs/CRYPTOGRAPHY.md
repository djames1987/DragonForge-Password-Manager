# Phase 1 Cryptographic Architecture

## Scope

Phase 1 builds the symmetric/password cryptographic foundation. It deliberately does **not** implement post-quantum KEMs or signatures; those are Phase 2 work and will be integrated through crypto-agile interfaces.

## Password-based derivation

The Phase 1 password KDF is Argon2id v1.3 with a 32-byte output. `Argon2idConfig` exposes memory, iteration, and lane parameters and rejects configurations below the project's current baseline:

- memory: at least 19 MiB (19,456 KiB)
- iterations: at least 2
- lanes: at least 1
- salt: at least 16 bytes

The library default is intentionally stronger than the minimum: 64 MiB, 3 iterations, and 1 lane. Product clients should eventually benchmark devices and persist an explicit KDF profile rather than assume one cost is appropriate forever.

A password-derived key will eventually unwrap a randomly generated vault master key. Vault records should never be encrypted directly with a master password.

## Symmetric encryption

Phase 1 uses AES-256-GCM through the RustCrypto `aes-gcm` crate. It requires a 256-bit key, generates a fresh 96-bit nonce from the operating-system CSPRNG, authenticates caller-provided associated data, and rejects modified ciphertext or mismatched AAD.

Nonce uniqueness is a hard requirement for AES-GCM. Future persistence layers must never reuse an envelope nonce for a new encryption under the same key.

## Key derivation and domain separation

HKDF-SHA-512 derives independent 256-bit subkeys. Callers must supply a non-empty `info` value naming the purpose of the derived key, such as:

```text
dragonforge/vault/items/v1
dragonforge/vault/files/v1
dragonforge/vault/backups/v1
```

## Secret key type

`SecretKey` wraps exactly 32 bytes and intentionally does not implement `Clone`, `Copy`, `Serialize`, or `Deserialize`. Its `Debug` output is redacted, and its backing array is zeroized on drop.

Zeroization is defense in depth; Rust and the operating system cannot guarantee erasure of every historical compiler/runtime copy.

## Versioned envelopes

Every `EncryptedEnvelope` carries a format version, cipher-suite identifier, AEAD nonce, and authenticated ciphertext. Decryption checks version and suite before invoking a cipher, enabling controlled future migration.

AAD is deliberately supplied by the containing protocol rather than stored as trusted envelope data.

## Key wrapping

Phase 1 key wrapping encrypts one 256-bit `SecretKey` under another using AEAD and domain-separated associated data:

```text
dragonforge/key-wrap/v1 || context_length || context
```

## Randomness

`OsRandom` delegates to the operating system through `rand_core::OsRng`.

## Crypto agility

Higher layers should depend on the crate's narrow traits (`PasswordKdf`, `AeadCipher`, `KeyDeriver`, `RandomSource`) rather than scattering primitive calls throughout the application.

## Non-goals in Phase 1

Phase 1 does not yet provide a persistent vault format, post-quantum KEM/signatures, synchronization, account authentication, device enrollment, recovery, sharing, browser/mobile integration, or rollback-resistant state logs.
