# DragonForge Password Manager — Visual Product Overview

<p align="center">
  <img src="assets/product-overview/hero.svg" alt="DragonForge Password Manager — security-first zero-knowledge password manager" width="100%">
</p>

<p align="center">
  <img src="../apps/desktop/icons/icon.png" alt="DragonForge application icon" width="96">
</p>

<p align="center">
  <strong>Security-first. Local-first. Zero-knowledge sync. Post-quantum-aware device trust.</strong>
</p>

DragonForge Password Manager is a Rust-based password manager built around a simple principle: **the systems that store or transport encrypted vault data should not need the secrets required to decrypt it**.

The project combines a hardened local encrypted vault, a native desktop application, Chrome/Edge browser integration, zero-knowledge multi-device synchronization, ML-DSA device identities, explicit device approval and revocation, offline disaster recovery, PostgreSQL-backed sync persistence, and Windows Credential Manager protection for local synchronization credentials.

> **Project status:** active development through Phase 11. Phase 11 Windows Credential Manager hardening has passed the Windows debug/fast automated verification suite. Optimized release verification remains a separate gate.
>
> **Security notice:** DragonForge has not undergone an independent cryptographic or application-security audit. It should not yet be used for production credentials or other high-value secrets.

---

## What DragonForge is designed to do

DragonForge is more than an encrypted password file. It is being developed as a complete password-management system with distinct security boundaries:

| Area | Current capability |
|---|---|
| Local vault | Encrypted logins and secure notes with per-item keys |
| Unlock | Master password **plus** external 256-bit Account Secret |
| Desktop | Native Tauri 2 app backed by Rust |
| Browser | Chrome and Edge Manifest V3 extension with explicit fill |
| Sync | Zero-knowledge encrypted-vault synchronization |
| Conflicts | Explicit Keep Local / Keep Remote resolution |
| Device trust | ML-DSA-65 identities with approval and revocation |
| Recovery | Offline recovery kit with credential and device rotation |
| Server storage | In-memory development store or PostgreSQL |
| Windows secret storage | Credential Manager for sync token and device signing seed |
| Post-quantum layer | ML-KEM-768 and ML-DSA-65 primitives plus hybrid X25519 + ML-KEM |
| Verification | Unit, integration, adversarial, regression, release and CI testing |

---

# 1. A local encrypted vault that keeps useful data encrypted

DragonForge stores vault items in a versioned `.dfvault` file. Sensitive fields remain inside authenticated ciphertext.

Examples of encrypted item content include:

- item title;
- username;
- password;
- URL;
- notes;
- tags;
- favorite flag;
- item-specific login or secure-note data.

Structural metadata such as vault identifiers, record identifiers, revisions, timestamps and ciphertext sizes necessarily remain visible to the file parser.

### Example login record — user view

```text
Name:       Example Bank
Username:   david@example.com
Password:   ••••••••••••••••
URL:        https://bank.example
Tags:       finance, personal
Favorite:   yes
Notes:      Primary checking account
```

### What the vault file stores instead

```text
Vault ID
  ├── KDF parameters + salt
  ├── wrapped Vault Master Key
  └── encrypted item record
       ├── opaque item UUID
       ├── revision
       ├── timestamps
       ├── wrapped random item key
       └── AES-256-GCM encrypted payload
             └── title / username / password / URL / notes / tags / favorite
```

Search happens locally after unlock by decrypting records on the trusted desktop. DragonForge does not create a plaintext searchable index for the sync server.

---

# 2. Multi-factor vault unlock without putting both factors in the vault

DragonForge's current local vault unlock model uses both:

1. the user's **master password**; and
2. an external random **256-bit Account Secret**.

The Account Secret is generated when the vault is created and is deliberately not serialized inside the `.dfvault` file.

### Unlock construction

```text
Master Password
      │
      │ Argon2id
      ▼
Password Key
      │
      │ HKDF-SHA-512
      │ salt = Account Secret
      ▼
Unlock Key
      │
      └── AES-256-GCM unwraps random Vault Master Key
```

This means possession of the vault file alone is not intended to provide every input required for unlock.

The project currently defaults to Argon2id parameters of 64 MiB memory, 3 iterations and 1 lane, with a fresh 32-byte salt. Defensive parser limits prevent attacker-controlled vault files from freely selecting unbounded Argon2 parameters.

---

# 3. Per-item encryption instead of one giant plaintext object

DragonForge generates a random 256-bit Vault Master Key when a vault is created.

That VMK derives an item-wrap key using HKDF-SHA-512. Each item then receives its own fresh random 256-bit item key.

```text
Vault Master Key
      │
      │ HKDF-SHA-512
      ▼
Item-Wrap Key
      │
      ├── wraps Item Key A → encrypts Login A
      ├── wraps Item Key B → encrypts Secure Note B
      └── wraps Item Key C → encrypts Login C
```

Updates generate a fresh item key and fresh AES-GCM nonce before replacing an encrypted record.

Authenticated data binds encrypted content to the vault ID, item ID and record revision, helping make ciphertext substitution and revision tampering fail authentication.

---

# 4. Native desktop experience

DragonForge includes a Tauri 2 desktop application backed by the Rust vault engine.

Current desktop capabilities include:

- create a vault;
- unlock an existing vault;
- lock and scrub the active session;
- native file selection;
- add, edit and delete login items;
- create secure notes;
- local search;
- favorites;
- password generation;
- reveal/copy controls;
- encrypted backup export;
- full-vault integrity verification;
- master-password changes;
- synchronization controls;
- device management;
- recovery-kit creation;
- disaster recovery.

### Conceptual desktop layout

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ DragonForge                                                       [Settings] │
├──────────────────┬──────────────────────────┬────────────────────────────────┤
│ Vault            │ Items                    │ Selected item                  │
│                  │                          │                                │
│ All Items        │ ★ Example Bank           │ Example Bank                   │
│ Favorites        │   david@example.com      │ david@example.com              │
│ Secure Notes     │                          │ ••••••••••••••                 │
│                  │   GitHub                 │ https://bank.example           │
│ Search           │   developer@example.com  │                                │
│                  │                          │ [Copy] [Reveal] [Edit]          │
│ Sync Vault       │   Recovery Note          │                                │
│ Lock             │                          │ Tags: finance, personal        │
└──────────────────┴──────────────────────────┴────────────────────────────────┘
```

The unlocked `Vault` object and Vault Master Key remain owned by the Rust process rather than JavaScript frontend state.

Credential-bearing Tauri command inputs are zeroized after use where practical, and locking drops the Rust session and scrubs decrypted UI state.

---

# 5. Browser filling without giving the extension the whole vault

DragonForge includes a Chromium Manifest V3 extension for **Google Chrome** and **Microsoft Edge**.

The extension is intentionally narrow:

- no broad host permissions;
- no persistent content script;
- no automatic submission;
- search returns only site-matching summaries;
- the password is requested only when the user explicitly chooses **Fill**;
- the desktop re-checks the destination host before releasing the selected credential.

### Browser fill example

Suppose the active tab is:

```text
https://accounts.example.com/login
```

The extension may show:

```text
DragonForge
────────────────────────────
Example Account
david@example.com

[ Fill ]
```

The search response does **not** need to contain the password.

Only after the user presses **Fill** does the native-host/desktop path request the selected credential. DragonForge performs a second host comparison before releasing it, then injects a one-shot script to populate visible login fields.

It does not automatically click the website's submit button.

### Browser security boundary

```text
Chrome / Edge extension
        │
        │ native messaging
        ▼
DragonForge native host
        │
        │ authenticated loopback bridge
        ▼
Running DragonForge desktop
        │
        ├── checks active site
        ├── reads unlocked vault
        └── returns only requested login
```

The master password, Account Secret, VMK and item-wrap key are not sent to the browser extension.

---

# 6. Zero-knowledge synchronization

<p align="center">
  <img src="assets/product-overview/architecture.svg" alt="DragonForge zero-knowledge architecture" width="100%">
</p>

DragonForge synchronizes the **already-encrypted vault file** rather than sending decrypted records to the server.

The server stores opaque encrypted bytes and synchronization metadata needed for conflict detection and authorization.

### Server-side data

The sync service may hold:

- account UUID;
- SHA-256 digest of the sync bearer token;
- vault UUID;
- encrypted vault bytes;
- revision;
- SHA-256 ciphertext fingerprint;
- timestamps;
- device UUID/name/status;
- device ML-DSA verifying keys;
- encrypted recovery envelope;
- recovery ML-DSA verifying key;
- recovery generation.

### Data intentionally kept away from the sync server

| Secret / plaintext | Sync server needs it? |
|---|---:|
| Master password | **No** |
| Plaintext Account Secret | **No** |
| Vault Master Key | **No** |
| Item encryption keys | **No** |
| Login usernames/passwords | **No** |
| Secure-note plaintext | **No** |
| Private ML-DSA device seed | **No** |
| Private recovery seed | **No** |

The server can therefore coordinate encrypted state without being designed as a decryption authority.

---

# 7. Conflict-aware multi-device synchronization

DragonForge does not blindly choose the newest-looking file.

Each synchronized local vault tracks:

1. the current local encrypted-vault SHA-256;
2. the last synchronized encrypted-vault SHA-256;
3. the current server revision/hash.

That allows the client to distinguish:

- **up to date**;
- **local changed** → upload;
- **remote changed** → download;
- **both changed** → conflict;
- suspicious rollback;
- same-revision ciphertext mismatch;
- previously synchronized remote vault unexpectedly missing.

### Example: safe conflict handling

Device A and Device B both know server revision 7.

```text
Device A changes a login
    ↓
uploads base revision 7
    ↓
server stores revision 8

Device B also changed locally
    ↓
sees remote revision 8
    ↓
DragonForge reports a conflict
```

DragonForge then requires an explicit choice:

```text
[ Keep Local ]     [ Keep Remote ]
```

It does not silently merge or overwrite the user's encrypted vault.

For a remote pull, the incoming snapshot must pass hash validation, vault structural validation, vault-ID validation and authenticated encrypted-item validation before replacing the local encrypted file.

---

# 8. Device enrollment, approval and revocation

Synchronization is not protected only by possession of the bearer token.

Each synchronized desktop has an **ML-DSA-65 device identity**:

- random device UUID;
- human-readable name;
- local private signing seed;
- server-side verifying key;
- server status: `pending`, `active` or `revoked`.

### First device

The first device ever enrolled for an account becomes active and establishes the initial device trust graph.

### Additional device

A second device enters the pending state:

```text
New Laptop
Status: pending
```

An already-active device must explicitly approve it.

### Signed synchronization

After device enrollment exists, vault requests require:

- bearer sync token;
- device UUID;
- request timestamp;
- ML-DSA signature.

The signed request transcript binds:

- HTTP method;
- API path;
- timestamp;
- SHA-256 of the body;
- base revision when relevant.

Requests more than five minutes outside server time are rejected.

### Revocation example

```text
Primary Desktop     active
Family Laptop       active
Old Laptop          revoked
```

A revoked device is denied synchronization even if it still has an old copy of the bearer token.

Revocation is terminal in the storage layer: a stale approval cannot transition a revoked identity back to active.

---

# 9. Secure lost-device recovery

<p align="center">
  <img src="assets/product-overview/recovery-flow.svg" alt="DragonForge lost-device recovery flow" width="100%">
</p>

Phase 10 solves the case where **every trusted synchronized device is lost**.

DragonForge creates a dedicated offline ML-DSA recovery identity. The private seed is placed in a user-held recovery kit:

```text
DFRK1:<account-id>:<vault-id>:<generation>:<ml-dsa-seed-hex>
```

The server receives only the corresponding verifying key.

The Account Secret is encrypted client-side using a key derived from the recovery seed and vault identity. The server stores the resulting encrypted recovery envelope, not the plaintext Account Secret.

### Recovery workflow example

A replacement computer is given:

```text
Destination:          C:\Vaults\recovered.dfvault
Sync server:          https://sync.example.com
Recovery kit:         DFRK1:...
Master password:      ***************
Replacement device:  New Desktop
```

DragonForge then:

1. signs a recovery-begin request;
2. retrieves the encrypted Account Secret envelope;
3. decrypts the Account Secret locally;
4. retrieves the opaque encrypted vault;
5. verifies its hash, structure and vault ID;
6. tries the supplied master password locally;
7. only after local unlock succeeds, completes server-side recovery.

Successful completion atomically:

- revokes previous devices;
- creates a fresh replacement-device identity;
- rotates the sync token;
- rotates the recovery key;
- increments the recovery generation.

A wrong master password does **not** consume the current recovery kit or modify server trust.

---

# 10. Windows Credential Manager protection

Phase 11 moves the remaining synchronization secrets out of the sidecar on Windows.

Before Phase 11, the sync sidecar contained:

```json
{
  "syncToken": "<256-bit bearer token>",
  "deviceSigningSeedHex": "<private ML-DSA seed>"
}
```

On Windows, the current version-3 sidecar instead contains a random credential reference:

```json
{
  "version": 3,
  "serverUrl": "https://sync.example.com",
  "lastRevision": 12,
  "lastContentSha256": "...",
  "deviceId": "...",
  "deviceName": "Primary Desktop",
  "credentialId": "..."
}
```

The sync token and ML-DSA device seed are stored in **Windows Credential Manager**.

Legacy Phase 8/9/10 sidecars migrate automatically. Migration is designed to fail closed if Credential Manager cannot accept the secret.

The desktop reports the current secret-storage mode:

```text
Sync configured
https://sync.example.com · revision 12 · secrets: Windows Credential Manager
```

Linux/macOS currently retain the legacy sidecar mechanism and are explicitly reported as such rather than being described as OS-protected.

---

# 11. Defense in depth

<p align="center">
  <img src="assets/product-overview/security-layers.svg" alt="DragonForge layered security model" width="100%">
</p>

DragonForge deliberately uses multiple security boundaries instead of treating one encryption key as the entire security model.

### Layer A — password hardening

Argon2id derives a password key using bounded, validated parameters.

### Layer B — external Account Secret

HKDF-SHA-512 combines the password-derived key with a separately stored random 256-bit Account Secret.

### Layer C — random VMK

The unlock key wraps a random 256-bit Vault Master Key.

### Layer D — per-item keys

A derived item-wrap key protects fresh random keys for each individual vault item.

### Layer E — authenticated record context

AES-256-GCM authenticates ciphertext together with vault/item/revision context.

### Layer F — device authorization

ML-DSA-65 device identities sign enrollment decisions and synchronization requests.

### Layer G — offline recovery

A separate recovery identity can replace trust after total device loss without giving the sync server the recovery private seed.

### Layer H — Windows OS credential storage

The bearer sync token and private device signing seed are moved out of copied sidecar files.

---

# 12. Cryptographic building blocks

DragonForge currently uses:

| Primitive | Role |
|---|---|
| Argon2id | Master-password key derivation |
| AES-256-GCM | Authenticated vault/item/envelope encryption |
| HKDF-SHA-512 | Domain-separated key derivation and secret combining |
| SHA-256 | Ciphertext fingerprints and token digests |
| ML-KEM-768 | Post-quantum KEM layer |
| ML-DSA-65 | Device and recovery signatures |
| X25519 | Classical half of the hybrid key-establishment construction |
| OS CSPRNG | Nonces, keys, tokens, IDs and secret material |

The post-quantum primitives use the pure-Rust RustCrypto `ml-kem` and `ml-dsa` crates.

DragonForge also implements an application-level **X25519 + ML-KEM-768** hybrid construction for future protocols. It is not claimed to be the TLS 1.3 X25519MLKEM768 wire format.

---

# 13. Hardened vault parser and storage behavior

The vault parser validates structure before performing expensive password derivation.

Current defensive limits include:

- maximum vault file: **64 MiB**;
- maximum items: **100,000**;
- maximum encrypted item payload: **1 MiB**;
- Argon2 memory cap: **1 GiB**;
- Argon2 iteration cap: **20**;
- Argon2 lane cap: **16**;
- KDF salt bounds;
- exact wrapped-key ciphertext shape;
- supported envelope versions;
- valid/unique UUIDs;
- non-zero record revisions;
- timestamp ordering.

Vault saves use same-directory temporary files, synchronization, backup renames and recovery logic.

If the live file is missing and a last complete backup exists, DragonForge can restore that backup before reading. An orphan temporary file alone is never automatically promoted to the live vault.

---

# 14. Encrypted backup and master-password changes

### Backup

Backup export writes the already-encrypted vault representation to a new file.

Backup import:

1. opens the source with the provided master password and Account Secret;
2. performs full item integrity verification;
3. refuses to overwrite an existing destination;
4. writes the encrypted representation only after validation succeeds.

The Account Secret is intentionally not embedded into the backup.

### Master-password change

Changing the master password creates a new Argon2id salt and derives a new unlock key.

DragonForge rewraps the VMK; it does not need to decrypt and re-encrypt every item solely because the master password changed.

---

# 15. Sync server deployment choices

The sync server is a separate Rust application.

### Development

For deterministic local testing it can run with volatile in-memory storage:

```powershell
$env:DRAGONFORGE_SYNC_ADMIN_TOKEN = "<32+ byte development token>"
cargo run -p dragonforge-sync-server
```

Default development bind:

```text
127.0.0.1:8787
```

### Persistent server

With the PostgreSQL feature enabled:

```powershell
$env:DRAGONFORGE_SYNC_DATABASE_URL = "postgres://user:password@127.0.0.1/dragonforge"
cargo run -p dragonforge-sync-server --features postgres
```

Checked-in migrations create storage for:

- sync accounts;
- encrypted vault snapshots;
- device enrollment records;
- recovery records.

Remote deployment requires HTTPS/TLS termination. Plain HTTP is accepted by the desktop only for loopback development addresses such as `127.0.0.1`, `localhost` and `::1`.

---

# 16. Example end-to-end user journey

A realistic DragonForge workflow looks like this:

### Day 1 — create the vault

```text
Create Vault
   ↓
Choose master password
   ↓
DragonForge generates Account Secret
   ↓
Save Account Secret separately
   ↓
Add logins + secure notes
```

### Day 2 — connect browser fill

```text
Install unpacked Chrome/Edge extension
   ↓
Register DragonForge native messaging host
   ↓
Unlock DragonForge desktop
   ↓
Visit matching website
   ↓
Select saved login
   ↓
Explicit Fill
```

### Day 3 — enable sync

```text
Start/provision sync server
   ↓
Configure sync token
   ↓
Enroll Primary Desktop
   ↓
Initial encrypted vault upload
```

### Day 4 — add a second computer

```text
Configure second device
   ↓
New device = pending
   ↓
Approve from active desktop
   ↓
Second device becomes active
   ↓
Encrypted state synchronizes
```

### Disaster scenario

```text
All devices lost
   ↓
Install DragonForge on replacement PC
   ↓
Provide offline DFRK1 recovery kit
   ↓
Provide master password
   ↓
Recover encrypted vault
   ↓
Old devices revoked
   ↓
Sync token rotated
   ↓
New recovery kit generated
```

---

# 17. Verification strategy

DragonForge's development process has accumulated regression coverage instead of replacing old tests whenever a new phase is added.

Current automated coverage includes areas such as:

- Argon2 parameter rejection;
- AES-GCM round trips and tamper rejection;
- HKDF domain separation;
- constant-time comparison behavior;
- ML-KEM-768 encapsulation/decapsulation;
- ML-DSA-65 signing and tamper rejection;
- hybrid X25519 + ML-KEM behavior;
- vault creation/unlock/reload;
- plaintext leakage checks;
- wrong-password and wrong-Account-Secret rejection;
- update/delete/search;
- encrypted backup/import;
- adversarial vault mutation;
- medium-vault stress;
- browser-origin validation;
- browser search/fill scoping;
- sync revision conflicts;
- remote snapshot tamper rejection;
- multi-device conflict resolution;
- device enrollment and proof-of-possession;
- pending/revoked device denial;
- recovery rotation;
- wrong-master recovery preservation;
- PostgreSQL feature coverage;
- Windows Credential Manager integration for Phase 11.

Windows verification runners create a log plus SHA-256 checksum so test evidence can be retained and reviewed independently from console output.

---

# 18. Technology stack

```text
Rust
├── dragonforge-crypto
├── dragonforge-vault
├── DragonForge desktop service
├── native browser host
└── sync server

Desktop
├── Tauri 2
├── HTML
├── CSS
└── JavaScript

Browser
├── Chromium Manifest V3
├── chrome.scripting
└── native messaging

Server
├── Axum
├── Tokio
├── Reqwest client on desktop
└── PostgreSQL / SQLx

Cryptography
├── Argon2id
├── AES-256-GCM
├── HKDF-SHA-512
├── SHA-256
├── X25519
├── ML-KEM-768
└── ML-DSA-65
```

---

# 19. Current boundaries and honest limitations

DragonForge is intentionally documented with its limitations.

Notable items not yet provided include:

- independent security audit;
- Windows Hello user-presence enforcement;
- TPM-bound non-exportable device private keys;
- macOS Keychain support;
- Linux Secret Service support;
- hardware security-key recovery;
- mobile client/recovery UI;
- social or multi-party recovery;
- signed append-only rollback history;
- automatic field-level conflict merging.

The current Windows Credential Manager hardening protects secrets at rest from simple copied-sidecar attacks, but it does not claim to protect a fully compromised logged-in Windows session from same-user malware.

---

# 20. Project documentation map

For deeper technical detail:

- [Cryptographic architecture](CRYPTOGRAPHY.md)
- [Vault format and parser hardening](VAULT_FORMAT.md)
- [Desktop architecture](PHASE5_DESKTOP.md)
- [Browser integration](PHASE6_BROWSER.md)
- [Zero-knowledge sync server](PHASE7_SYNC_SERVER.md)
- [Multi-device synchronization](PHASE8_MULTI_DEVICE_SYNC.md)
- [Device enrollment and revocation](PHASE9_DEVICE_ENROLLMENT.md)
- [Secure account and device recovery](PHASE10_ACCOUNT_RECOVERY.md)
- [Windows Credential Manager hardening](PHASE11_CREDENTIAL_PROTECTION.md)

---

## Product identity in one sentence

> **DragonForge Password Manager is a Rust-native, local-first password manager that combines a hardened encrypted vault with zero-knowledge synchronization, post-quantum-aware device trust, explicit browser filling, recoverable multi-device security and OS-backed protection of local sync credentials.**

The goal is not simply to encrypt passwords. It is to build a system in which **vault encryption, device authorization, synchronization, browser access and recovery each have separate, inspectable security boundaries**.
