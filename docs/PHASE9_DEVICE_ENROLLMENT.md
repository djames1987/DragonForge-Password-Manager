# Phase 9 — Device Enrollment

## Status

**Status: COMPLETE — implementation and regression coverage are in place.**

Phase 9 adds cryptographic device identities and explicit device authorization to DragonForge multi-device synchronization.

The zero-knowledge vault model remains unchanged: the sync server still stores only encrypted vault bytes and synchronization metadata. Phase 9 changes **who is allowed to access those encrypted bytes**.

## Device identity

Each synchronized desktop device receives:

- a random device UUID;
- a human-readable device name;
- an ML-DSA-65 signing keypair;
- a server-side enrollment state: `pending`, `active`, or `revoked`.

The ML-DSA private seed remains local to the device. The sync server stores only the corresponding ML-DSA verifying key.

Enrollment and display-name refresh requests carry an ML-DSA proof-of-possession signature. The signed enrollment transcript binds the device UUID, normalized display name, and verifying key, so the server verifies that the enrolling client actually controls the private key corresponding to the identity it registers.

The current desktop implementation stores the private seed in the per-vault sync sidecar. This is an improvement over bearer-token-only authorization, but OS-backed secure key storage is still a future hardening item.

## Enrollment lifecycle

### First device

The first device ever enrolled for a sync account becomes `active` immediately.

This bootstraps the device trust graph from possession of the high-entropy account sync token.

Once any device record exists, the account can never silently bootstrap a new first device again. If every active device is later lost or revoked, recovery must use a future recovery flow rather than auto-trusting a new key.

### Additional devices

Later devices are created as `pending`.

A pending device:

- can query its own enrollment state;
- cannot download encrypted vault state;
- cannot upload encrypted vault state;
- cannot approve or revoke other devices.

An already-active device must explicitly approve the pending device.

### Approval

Approval is signed by the approving device's ML-DSA-65 private key.

The signed decision binds:

- the action (`approve`);
- the approving device UUID;
- the target device UUID.

The server verifies the signature against the stored verifying key of the active approving device before activating the target.

### Revocation

Revocation uses the same signed-decision model.

A revoked device is denied synchronized vault access even if it still possesses the account sync token.

Revocation is terminal in both the in-memory and PostgreSQL stores. Even if an approval request races with revocation, storage refuses a `revoked -> active` transition.

A device cannot revoke itself through the Phase 9 API.

## Signed synchronization requests

After an account has any device enrollment record, vault GET/PUT requests require:

- the normal account bearer sync token;
- `X-DragonForge-Device-Id`;
- `X-DragonForge-Device-Timestamp`;
- `X-DragonForge-Device-Signature`.

The signature covers a domain-separated canonical transcript containing:

- HTTP method;
- API path;
- request timestamp;
- SHA-256 of the request body;
- base revision when applicable.

The server accepts only active devices and rejects signatures whose timestamps are more than five minutes from server time.

This binds upload authorization to the exact ciphertext body and base revision being written.

## Migration from Phase 8

Phase 8 sidecars used sync configuration version 1 and had no device identity.

Phase 9 sidecars use version 2.

Existing version-1 sidecars remain readable. The desktop client upgrades them when it generates the local device identity.

For migration safety, accounts with **zero device records** still accept the Phase 8 bearer-only vault protocol. As soon as the first device record exists, signed active-device authorization becomes mandatory.

## PostgreSQL schema

Migration:

```text
apps/sync-server/migrations/0002_phase9_device_enrollment.sql
```

The server stores:

- account UUID;
- device UUID;
- display name;
- ML-DSA-65 verifying key;
- enrollment state;
- creation timestamp;
- approval timestamp;
- revocation timestamp.

Private device keys are never stored by the server.

## Desktop controls

Vault settings now include a Device Enrollment section that can:

- display this device's ID/name/status;
- enroll or refresh this device;
- rename the same cryptographic device identity;
- list account devices;
- approve pending devices;
- revoke other active devices.

Tauri commands:

- `enroll_device`
- `own_device_status`
- `list_devices`
- `approve_device`
- `revoke_device`

## Security properties

Phase 9 prevents a copied sync bearer token by itself from accessing a vault after device enrollment has begun.

It also prevents:

- pending devices from syncing;
- revoked devices from syncing;
- reusing a device UUID with a different verifying key;
- auto-rebootstrap after all previously enrolled devices are revoked;
- unsigned approval or revocation decisions;
- registration of a device identity without proving possession of its ML-DSA private key;
- reactivation of a revoked device identity.

## Limitations / later phases

Phase 9 does not yet provide:

- TPM / Windows Hello / Keychain / Secure Enclave storage for device private keys;
- a recovery path when every authorized device is lost;
- push notifications for pending enrollment requests;
- QR-code or proximity enrollment;
- signed append-only rollback history;
- device attestation;
- mobile device enrollment UI;
- per-request nonce replay storage beyond timestamp freshness.

Those belong to later hardening/recovery/mobile phases.


## Local sync removal caution

Phase 9's **Remove sync** action removes the local sync sidecar, including the local ML-DSA signing seed. It does not automatically revoke the server-side device record.

Before removing sync from an active device, ensure another active device is available to approve a replacement. Removing the only active device's local identity can leave the account without a usable approval key until a future recovery flow is implemented.


## Completion verification coverage

Phase 9 automated coverage includes:

- initial-device bootstrap to active;
- additional-device pending enrollment;
- ML-DSA enrollment proof-of-possession verification;
- invalid enrollment-proof rejection;
- pending-device sync denial;
- active-device signed vault access;
- bearer-only vault access denial after enrollment begins;
- stale signed-request rejection outside the five-minute freshness window;
- active-device signed approval;
- active-device signed revocation;
- revoked-device sync denial;
- terminal revocation / no reactivation;
- prevention of first-device re-bootstrap after all historical devices are revoked;
- Phase 8 sidecar v1 to v2 migration while preserving server URL, bearer token, last revision, and ciphertext hash;
- existing Phase 8 multi-device synchronization and conflict regressions.
