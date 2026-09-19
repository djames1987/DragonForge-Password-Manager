# Phase 11 — Windows Credential Protection & Sidecar Secret Migration

## Status

Phase 11 hardens DragonForge's local synchronization credentials on Windows.

Before Phase 11, the per-vault `.dfvault.sync.json` sidecar contained the 256-bit sync bearer token and the ML-DSA-65 device signing seed. The sidecar was restricted by local filesystem permissions where supported, but those credentials were still recoverable from the file itself.

Phase 11 changes the Windows storage boundary so those secrets are stored in Windows Credential Manager instead.

## Protected secrets

On Windows, DragonForge now stores these values in one Credential Manager record:

- the 256-bit synchronization bearer token;
- the local ML-DSA-65 device signing seed.

The sidecar keeps only non-secret synchronization metadata:

- configuration version;
- sync-server URL;
- last synchronized revision;
- last synchronized ciphertext hash;
- device UUID;
- device display name;
- a random credential reference UUID.

The Credential Manager service name is:

```text
DragonForge Password Manager
```

The random credential-reference UUID prevents the vault path, account ID, or server URL from being used directly as a credential-store lookup key.

## Sync sidecar version 3

Phase 11 upgrades the sidecar format to version 3.

A protected Windows sidecar resembles:

```json
{
  "version": 3,
  "serverUrl": "https://sync.example.com",
  "lastRevision": 12,
  "lastContentSha256": "...",
  "deviceId": "...",
  "deviceName": "Primary PC",
  "credentialId": "..."
}
```

It must not contain:

```text
syncToken
deviceSigningSeedHex
```

DragonForge rejects a version-3 Windows sidecar that attempts to contain either plaintext secret.

## Migration

Existing Phase 8/9/10 version-1 or version-2 sidecars are migrated automatically on Windows.

Migration is fail-closed:

1. DragonForge reads and validates the legacy sidecar.
2. A fresh random credential reference is generated.
3. The bearer token and device signing seed are written to Windows Credential Manager.
4. Only after the credential-store write succeeds is the sidecar rewritten as version 3.
5. The new sidecar omits both plaintext secrets.
6. The in-memory copies continue to be zeroized with the existing secret-handling types.

If Windows Credential Manager is unavailable or rejects the secret, DragonForge returns a secure-storage error instead of silently writing the credentials back to a version-3 plaintext sidecar.

## Reconfiguration and removal

When synchronization is reconfigured to a different account/server credential set:

- a new credential reference is created;
- the new credentials are stored first;
- the new sidecar is committed;
- the superseded Credential Manager entry is removed.

When **Remove sync** is used on Windows, DragonForge removes the Credential Manager record as well as the sidecar files.

## Recovery integration

Phase 10 recovery rotates the sync token and replacement-device signing seed.

The recovered configuration is now installed through the same Phase 11 protected-storage path, so a successful recovery does not reintroduce the rotated credentials into the plaintext sidecar.

## Platform behavior

### Windows

Storage label:

```text
windowsCredentialManager
```

The sync token and device signing seed are removed from the sidecar.

### Linux/macOS

Phase 11 does not claim OS-backed protection on these platforms yet.

Storage label:

```text
legacySidecar
```

The existing sidecar behavior remains for compatibility. A later phase can add Secret Service / Keychain support after platform-specific lifecycle and CI coverage are available.

The desktop UI reports which storage mode is active.

## Threat-model improvement

Phase 11 raises the cost of stealing synchronization credentials from a copied vault directory on Windows.

A copied set of:

```text
vault.dfvault
vault.dfvault.sync.json
```

no longer contains the bearer token or device signing seed.

This does not make a fully compromised, logged-in Windows session safe. Malware executing as the user may still be able to request credentials through operating-system APIs. Phase 11 is local secret-at-rest hardening, not endpoint-compromise protection.

## Testing

Windows integration coverage verifies that:

- configuring sync writes a version-3 sidecar;
- the sidecar does not contain `syncToken`;
- the sidecar does not contain `deviceSigningSeedHex`;
- the sidecar contains a random `credentialId`;
- DragonForge can reload the sync configuration through Windows Credential Manager;
- removing sync removes the sidecar and its Credential Manager entry.

Pure unit coverage verifies protected sidecar serialization independently of the operating-system credential store.

Previous Phase 8, 9, and 10 synchronization/recovery regressions remain part of the Phase 11 verification runner.

## Current limitations

Phase 11 does not yet add:

- Windows Hello user-presence prompts;
- TPM-bound non-exportable device signing keys;
- macOS Keychain support;
- Linux Secret Service support;
- hardware security-key storage;
- independent security audit.

DragonForge remains active-development security software and should not yet be trusted with production credentials.
