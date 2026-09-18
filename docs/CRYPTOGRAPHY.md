# DragonForge Cryptographic Architecture

## Current phase

Phase 2 adds the post-quantum and hybrid public-key layer on top of the Phase 1 symmetric/password foundation.

## Password-based derivation

The password KDF is Argon2id v1.3 with a 32-byte output. `Argon2idConfig` rejects configurations below the project's current baseline:

- memory: at least 19 MiB (19,456 KiB)
- iterations: at least 2
- lanes: at least 1
- salt: at least 16 bytes

The current library default is 64 MiB, 3 iterations, and 1 lane.

A password-derived key is intended to unwrap a randomly generated vault master key. Vault records should not be encrypted directly with a master password.

## Symmetric encryption

AES-256-GCM is used for authenticated symmetric encryption.

Requirements:

- 256-bit key.
- Fresh 96-bit nonce for every encryption under a given key.
- Associated data for record identity and protocol binding.
- Authentication failure on modified ciphertext or mismatched AAD.

## HKDF

HKDF-SHA-512 is used for domain-separated subkey derivation and for the Phase 2 hybrid combiner.

## Secret key handling

`SecretKey` wraps 32 bytes and intentionally does not implement ordinary serialization, `Clone`, or `Copy`. Its debug output is redacted and its backing memory is zeroized on drop.

Phase 2 PQ private-key containers also avoid Serde serialization. Explicit seed export returns `Zeroizing<Vec<u8>>` so callers must opt into handling private material.

Zeroization is defense in depth and cannot guarantee elimination of every historical compiler/runtime copy.

## ML-KEM-768

Phase 2 implements ML-KEM-768 through RustCrypto `ml-kem`.

Current serialized sizes:

- public/encapsulation key: 1184 bytes
- private seed: 64 bytes
- ciphertext: 1088 bytes
- resulting shared secret: 32 bytes

Public keys are validated during import. Private state is reconstructed from the 64-byte seed form rather than persisting expanded internal key state.

Standalone ML-KEM APIs are exposed for testing and future protocol integration.

## ML-DSA-65

Phase 2 implements ML-DSA-65 through RustCrypto `ml-dsa`.

Current serialized sizes:

- private seed: 32 bytes
- verifying key: 1952 bytes
- signature: 3309 bytes

The API supports:

- key generation
- seed restoration
- signing
- signature parsing
- verification
- rejection of modified messages/signatures

ML-DSA-65 is intended for future device identity and authorization records.

## Hybrid X25519 + ML-KEM-768

DragonForge Phase 2 defines an application-level hybrid KEM for future device enrollment and sharing protocols.

Recipient state:

```text
long-term X25519 private key
+
ML-KEM-768 decapsulation key
```

Published recipient material:

```text
X25519 public key
+
ML-KEM-768 encapsulation key
```

Sender operation:

```text
fresh ephemeral X25519 key
        │
        ├── X25519 shared secret
        │
recipient X25519 public key

recipient ML-KEM public key
        │
        └── ML-KEM shared secret + ML-KEM ciphertext
```

The two 32-byte shared secrets are concatenated and processed through HKDF-SHA-512.

The HKDF transcript includes length-delimited fields for:

- DragonForge hybrid protocol domain
- caller-supplied protocol context
- recipient X25519 public key
- recipient ML-KEM public key
- sender ephemeral X25519 public key
- ML-KEM ciphertext

This prevents the final key from being independent of the handshake transcript or intended protocol use.

The X25519 result must be contributory. All-zero/non-contributory peer values are rejected.

### Important interoperability note

This Phase 2 application-level hybrid construction is **not claimed to be the TLS 1.3 X25519MLKEM768 wire format**. TLS hybrid negotiation belongs to the transport layer and will be implemented through a standards-compliant TLS stack in a later phase.

## Public and private serialization boundaries

Public keys and ciphertext structures can be serialized.

Private ML-KEM, ML-DSA, and X25519 key containers are intentionally not Serde-serializable. Future encrypted device-key persistence must wrap private seeds/keys under device or vault key encryption rather than place them directly in general application data structures.

## Versioned encrypted envelopes

Every symmetric `EncryptedEnvelope` carries a format version and cipher-suite identifier. Decryption checks both before invoking the cipher.

## Key wrapping

Phase 1 key wrapping remains the mechanism for encrypting one 256-bit secret key beneath another using authenticated encryption and domain-separated AAD.

## Dependency selection

Phase 2 deliberately does not use `pqcrypto-*` because that ecosystem became unmaintained after the upstream PQClean archival process. The current implementation uses pure-Rust RustCrypto ML-KEM and ML-DSA crates and `x25519-dalek`.

These upstream implementations still require independent review before DragonForge can claim production assurance.

## Tests

The integration test suite covers:

- ML-KEM encapsulate/decapsulate agreement
- ML-KEM seed restore
- ML-KEM malformed public/ciphertext lengths
- ML-DSA sign/verify
- ML-DSA modified-message rejection
- ML-DSA modified-signature rejection
- ML-DSA seed restore
- invalid ML-DSA public key lengths
- hybrid agreement
- protocol-context binding
- empty-context rejection
- non-contributory X25519 rejection
- hybrid ciphertext serialization round trip
- private-key debug redaction

See `docs/PHASE2_TESTING.md`.

## Remaining non-goals

Phase 2 still does not provide:

- persistent vault storage
- a vault master-key hierarchy
- account authentication
- sync/server protocols
- device enrollment workflows
- recovery
- secure item sharing
- browser/mobile integration
- rollback-resistant state logs
- standards-compliant PQ/T TLS configuration
- independent cryptographic audit
