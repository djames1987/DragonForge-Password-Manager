# Phase 5 Desktop Application Foundation

## Scope

Phase 5 introduces the first user-facing DragonForge desktop application while keeping the cryptographic and vault layers unchanged beneath it.

The desktop application uses Tauri 2 with a checked-in HTML/CSS/JavaScript frontend. There is no npm, Node.js, bundler, or remote CDN dependency in this phase.

## Architecture

```text
Tauri WebView UI
      |
      | narrow invoke commands
      v
DesktopService (Rust)
      |
      | Mutex<Option<Session>>
      v
dragonforge-vault
      |
      v
dragonforge-crypto
```

The UI never receives the VMK or item-wrap key. The unlocked `Vault` remains inside Rust process memory.

## User flows

### New vault

1. Choose a local `.dfvault` path with the native save dialog.
2. Enter and confirm a master password.
3. Rust creates the encrypted vault and a random Account Secret.
4. The Account Secret is displayed exactly for the recovery workflow.
5. The user must acknowledge that it was stored safely before entering the vault.

The Account Secret is not retained by the desktop session.

### Unlock

Unlock requires:

- vault file
- master password
- 64-character hexadecimal Account Secret

Command strings containing credentials are zeroized after the Rust service call returns.

### Vault UI

The main window is a three-pane layout:

- navigation/security actions
- searchable item list
- selected item detail

Filters include all items, favorites, logins, and secure notes.

### Item editing

The modal editor supports:

- title/name
- username
- password
- URL
- tags
- notes
- favorite state
- secure-note mode
- CSPRNG password generation

### Locking

Locking:

1. drops the Rust `Vault` session;
2. clears selected decrypted item state;
3. clears password/note/settings/recovery DOM values;
4. closes item/settings/recovery dialogs;
5. returns to the locked welcome screen.

This is defense in depth; a general-purpose webview/runtime cannot guarantee forensic erasure of all historical copies.

## Frontend security

- strict CSP
- no remote JavaScript/CSS
- no localStorage/sessionStorage use for vault data
- vault content inserted with `textContent`, not `innerHTML`
- no master-password or Account-Secret logging
- explicit password reveal control
- clipboard-copy warnings

## Current desktop features

- create vault
- unlock/lock
- native open/save dialogs
- login CRUD
- secure-note CRUD
- favorites
- search
- password generation
- integrity verification
- encrypted backup export
- master-password change
- responsive dark UI
- Ctrl+K search shortcut
- Ctrl+L lock shortcut

## Deliberate non-goals for Phase 5

- browser autofill
- cloud sync
- device enrollment
- biometrics/Windows Hello
- automatic inactivity lock
- tray integration
- installer/code signing
- OS credential-store integration
- clipboard auto-clear
- import UI for backup restore
- multi-window support

Those belong in later phases after the desktop foundation is validated.
