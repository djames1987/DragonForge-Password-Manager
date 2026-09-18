# DragonForge Vault Format — Phase 4 Hardening

## Current format

The persistent `.dfvault` format remains **version 1**. Phase 4 hardens parsing, validation, persistence, and migration readiness without silently changing the cryptographic meaning of version 1.

## File structure

```text
version
vault_id
created_at
updated_at
kdf:
  salt
  memory_kib
  iterations
  lanes
wrapped_vmk:
  envelope version
  cipher suite
  nonce
  ciphertext
items[]:
  id
  revision
  created_at
  updated_at
  wrapped_item_key:
    envelope...
  payload:
    envelope...
```

Sensitive item fields remain inside authenticated ciphertext.

## Phase 4 defensive limits

Before expensive cryptographic work, DragonForge enforces:

- vault file: at most 64 MiB
- vault items: at most 100,000
- encrypted item payload: at most 1 MiB
- KDF salt: at most 64 bytes and at least the cryptographic minimum
- Argon2 memory: at most 1 GiB
- Argon2 iterations: at most 20
- Argon2 lanes: at most 16
- wrapped 256-bit key ciphertext: exactly 48 bytes for the current AES-256-GCM envelope
- encrypted payload ciphertext: at least one authentication tag in length

These are defensive implementation limits and can be migrated explicitly in future format/application versions.

## Structural validation

Before Argon2 derivation and VMK unwrap, Phase 4 validates:

- supported vault format version
- valid vault UUID
- vault timestamp ordering
- KDF salt and parameter bounds
- wrapped VMK envelope version and ciphertext length
- item-count bound
- valid item UUIDs
- unique item IDs
- revision greater than zero
- item timestamp ordering
- wrapped item-key envelope shape
- encrypted payload envelope version and size

A malformed file therefore cannot freely drive expensive Argon2 parameters.

## Revisions

New records begin at revision 1. Updates use checked arithmetic; an impossible `u64` revision overflow is rejected rather than wrapping or saturating.

The revision remains part of payload AAD.

## Persistence

The save sequence is:

1. serialize and validate the complete new vault;
2. write a same-directory temporary file;
3. synchronize the temporary file;
4. move the previous live file to the backup path when present;
5. synchronize the parent directory on Unix;
6. move the temporary file into the live path;
7. synchronize the parent directory on Unix;
8. remove the backup after success.

If the live vault is missing but the backup exists, the backup is restored before reading.

An orphan temporary file alone is never promoted automatically.

## Version inspection and migration readiness

`inspect_vault_file` reports:

- format version
- item count
- file size
- migration status

For version 1, migration status is `Current`.

Unknown/future versions are rejected. Because DragonForge has no pre-v1 persistent format, Phase 4 does not contain a synthetic migration. Future migrations must be explicit transforms rather than reinterpretation of existing fields.

## Remaining rollback limitation

Authentication detects modification, but a complete older valid copy is still cryptographically valid. Local rollback detection requires an external monotonic state anchor or synchronized signed state and remains a later-phase feature.
