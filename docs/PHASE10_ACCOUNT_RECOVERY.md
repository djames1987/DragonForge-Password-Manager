# Phase 10 — Secure Account and Device Recovery

Phase 10 adds an offline recovery path for synchronized DragonForge vaults without giving the sync server the master password, Account Secret, Vault Master Key, recovery private key, or decrypted vault data.

## Goals

Phase 10 solves the Phase 9 lockout case where every active device is lost or its local signing identity is unavailable.

A recovery operation can:

- authenticate with a separately stored offline recovery kit;
- retrieve the already-encrypted synchronized vault;
- recover the Account Secret client-side;
- verify the user's master password locally before changing server trust state;
- create a fresh trusted replacement device;
- revoke every previous device;
- rotate the sync token;
- rotate the recovery credential so the old kit is one-time use.

## Recovery credential

DragonForge generates a dedicated ML-DSA-65 key pair for recovery.

The server receives only the ML-DSA-65 verifying key. The private seed is placed in the offline recovery kit:

```text
DFRK1:<account-id>:<vault-id>:<generation>:<ml-dsa-seed-hex>
```

The kit is a high-value secret and should be stored offline and separately from the synchronized devices.

## Account Secret recovery envelope

The Account Secret is never uploaded in plaintext.

The desktop derives a 256-bit recovery encryption key from:

- the recovery private seed;
- the vault UUID;
- the domain string `dragonforge/recovery-envelope/v1`.

It then encrypts the Account Secret with AES-256-GCM and vault-bound authenticated data. The server stores only the resulting encrypted envelope.

The recovery server cannot decrypt this envelope because it never receives the recovery seed.

## Recovery authorization

Recovery operations use domain-separated ML-DSA signatures rather than the normal sync bearer token.

Signed recovery requests bind:

- action;
- account UUID;
- vault UUID;
- recovery generation;
- current timestamp;
- fresh 256-bit nonce.

The server accepts timestamps only inside a five-minute freshness window.

Recovery generations prevent a successfully used or rotated recovery kit from authorizing future recovery operations.

## Recovery sequence

1. An active synchronized device creates or rotates a recovery kit.
2. The active device signs the recovery configuration request.
3. The server stores the recovery verifying key and encrypted Account Secret envelope.
4. After device loss, a replacement desktop supplies the recovery kit and sync-server URL.
5. The recovery key signs a fresh recovery authorization request.
6. The server returns the encrypted recovery envelope.
7. The desktop decrypts the Account Secret locally.
8. The desktop retrieves the opaque encrypted vault through the recovery-only endpoint.
9. The desktop validates the vault ID, SHA-256, structure, and attempts to open it with the supplied master password.
10. Only after local vault unlock succeeds does the desktop submit recovery completion.
11. Recovery completion atomically:
   - revokes all historical devices that are not already revoked;
   - installs one fresh active replacement-device identity;
   - rotates the account sync token;
   - installs a fresh recovery verifying key/envelope;
   - increments the recovery generation.
12. The desktop persists the rotated sync credentials and presents the recovered Account Secret and replacement recovery kit to the user.

A wrong master password therefore does not rotate trust or consume the current recovery kit.

## Server endpoints

Phase 10 adds:

- `POST /v1/recovery/configure`
- `POST /v1/recovery/begin`
- `POST /v1/recovery/vault`
- `POST /v1/recovery/complete`

Normal synchronized vault access remains protected by the Phase 9 active-device signature protocol.

## PostgreSQL persistence

Migration:

```text
apps/sync-server/migrations/0003_phase10_secure_recovery.sql
```

The server stores:

- account UUID;
- vault UUID;
- recovery ML-DSA verifying key;
- encrypted recovery envelope;
- monotonically increasing recovery generation;
- update timestamp.

## Desktop behavior

Vault settings include a **Secure account recovery** section for creating or rotating the recovery kit.

The welcome screen includes **Recover synchronized vault**, which accepts:

- a new local destination;
- sync-server URL;
- recovery kit;
- master password;
- replacement-device name.

After a successful recovery, DragonForge displays the recovered Account Secret and newly rotated recovery kit and requires the user to acknowledge saving both before continuing.

## Security properties

Phase 10 is designed so that:

- the server cannot decrypt the vault or Account Secret;
- possession of only the sync token cannot perform disaster recovery;
- possession of only the recovery verifying key cannot perform recovery;
- the recovery private seed never reaches the server;
- a successful recovery invalidates the old recovery kit;
- a successful recovery invalidates the old sync token;
- previous devices are revoked;
- a stale recovery generation cannot race a newer recovery;
- replacement devices prove possession of their own ML-DSA private key;
- newly rotated recovery keys prove possession before being installed;
- a wrong master password does not consume the recovery kit.

## Current limits

Phase 10 does not yet add:

- TPM / Windows Hello protection for device or recovery keys;
- social or multi-party recovery;
- hardware security-key recovery;
- offline recovery from a local backup without the sync server;
- durable distributed rate limiting across multiple sync-server instances;
- mobile recovery UI;
- external security audit.

The password manager remains under active development and must not yet be treated as independently audited production security software.
