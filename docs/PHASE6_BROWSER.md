# Phase 6 — Browser Extension Foundation

## Scope

Phase 6 adds the first DragonForge browser integration for Chromium-family browsers (Google Chrome and Microsoft Edge).

The extension is deliberately conservative:

- Manifest V3
- no broad host permissions
- no persistent content scripts
- no automatic form submission
- no background password harvesting
- explicit user action is required before a credential is released
- the extension never receives the master password, Account Secret, VMK, or item-wrap key

Firefox uses a different native-host allowlist format and is deferred until a dedicated compatibility pass.

## Architecture

```text
Website
   ^
   | one-shot scripting injection after explicit Fill
   |
Browser popup
   |
   v
MV3 service worker
   | nativeMessaging
   v
dragonforge-native-host
   | authenticated loopback JSON
   v
DragonForge desktop process
   | site-scoped DesktopService API
   v
Unlocked Vault
```

The native messaging process does not open vault files and does not have the master password or Account Secret. It forwards narrowly scoped requests to the running desktop application.

## Browser permissions

The Chromium manifest requests only `activeTab`, `scripting`, and `nativeMessaging`. There are no `host_permissions` and no registered `content_scripts`.

Page access therefore comes from the temporary `activeTab` grant created when the user invokes the extension.

## Native bridge authentication

When DragonForge desktop starts, it binds to `127.0.0.1` on an OS-assigned port, generates a fresh 256-bit random bridge token, and writes port/token/PID/protocol metadata to the per-user DragonForge application-data location. The native messaging host attaches that token when forwarding browser requests. The extension never sees it.

The endpoint is recreated on each desktop launch. On Unix it is created with mode 0600. On Windows it is kept under the current user's LocalAppData tree.

## Protocol version 1

Supported actions are `status`, `search`, and `credential`.

`search` returns only item ID, name, username, saved URL, and favorite state. Passwords are not returned during search.

`credential` performs a second site-host check immediately before returning the username/password.

## Site matching

Phase 6 accepts only HTTP and HTTPS URLs. Hostnames are case-normalized and a leading `www.` is normalized away. Unrelated hosts and non-web schemes are rejected. Subdomain wildcard matching is intentionally not enabled.

## Explicit fill behavior

The popup shows matching login entries for the active site. Only after the user clicks **Fill** does the service worker request the password. Immediately before injection, the service worker re-reads the active tab and checks that it is still the same site.

The one-shot injected function finds visible login fields, sets values through the native input setter, dispatches `input` and `change` events, does not submit the form, and is not persistently installed on the page.

## Native host installation on Windows

1. Load `apps/browser-extension` as an unpacked extension.
2. Copy the extension ID shown by the browser.
3. Register the native host.

Chrome:

```powershell
.\scripts\install-browser-native-host.ps1 -ChromeExtensionId <ID>
```

Edge:

```powershell
.\scripts\install-browser-native-host.ps1 -EdgeExtensionId <ID>
```

For both browsers, pass both parameters on one command line.

The installer builds `dragonforge-native-host.exe`, copies it under the current user's LocalAppData DragonForge directory, creates the Chromium native-host manifest, and writes HKCU registration keys. Administrator rights are not required.

## Packaging

```powershell
.\scripts\package-browser-extension.ps1
```

Output: `dist\dragonforge-browser-extension.zip`.

## Current limitations

Phase 6 intentionally does not yet include automatic page-load autofill, automatic form submission, passkeys, TOTP, save/update prompts, browser-side vault editing, Firefox packaging, Safari support, extension-store publishing/signing, or enterprise deployment.