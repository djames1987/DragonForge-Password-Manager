# Phase 3 Vault Format

## Purpose

The Phase 3 `.dfvault` file is the first persistent DragonForge vault format. It is designed for local correctness, authenticated encryption, testability, and future migration.

The current format version is `1`.

## High-level file structure

The serialized file contains:

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

Item titles, credentials, URLs, notes, tags, and favorite state are not stored in plaintext.

## Unlock material

A vault needs two user-controlled inputs:

1. the master password;
2. the 32-byte Account Secret created alongside the vault.

The Account Secret is **not stored in the vault file**.

Losing both the Account Secret and every external copy of it makes the vault unrecoverable by design.

## Key hierarchy

```text
Master Password
  │
  └─ Argon2id + vault salt
        │
        ▼
   Password Key
        │
        └─ HKDF-SHA-512 + Account Secret
              │
              ▼
          Unlock Key
              │
              └─ unwraps VMK
                    │
                    └─ HKDF-SHA-512
                          │
                          ▼
                    Item-Wrap Key
                          │
                          ├─ unwrap Item Key 1 -> decrypt Item 1
                          ├─ unwrap Item Key 2 -> decrypt Item 2
                          └─ ...
```

## Record revisions

New records start at revision 1.

An update increments the revision and re-encrypts the payload using a fresh random item key. The revision is included in payload AAD, so changing the revision without creating matching ciphertext invalidates authentication.

## Search

There is no plaintext index.

Once the vault is unlocked, search decrypts records locally and matches against:

- title/name
- tags
- login username
- login URL
- login notes
- secure-note text

Passwords are intentionally not searched.

## Persistence and recovery

Mutating operations save automatically.

The write path:

1. writes the complete new serialized vault to a same-directory temporary file;
2. synchronizes the temporary file;
3. renames the old vault to a backup name when present;
4. renames the temporary file into the live path;
5. removes the temporary backup after success.

If the live file is absent and a backup exists, the read path restores the backup.

This is a crash-recovery mechanism for Phase 3, not a transactional multi-writer design.

## Backups

A backup is an encrypted copy of the current vault representation.

It does not contain the Account Secret.

Import validates the source credentials and decrypts all item records before creating the destination copy.

## Format migration

Code must reject unknown vault format versions.

Future phases should add explicit migration functions rather than changing the meaning of version 1 fields in place.
