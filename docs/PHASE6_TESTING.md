# Phase 6 Browser Extension Verification

## Automated Windows verification

Run:

```powershell
.\scripts\run-phase6-tests.ps1
```

The runner creates `test-logs\dragonforge-phase6-YYYYMMDD-HHMMSS.log` and its `.sha256` companion.

It runs Rustfmt, Clippy, browser JavaScript syntax checks, extension unit/static tests, the full Rust workspace, desktop/browser scoping tests, native-host framing tests, Phase 4 regressions, release tests/builds, and dependency visibility.

## Extension static tests

```powershell
node --check apps/browser-extension/src/core.js
node --check apps/browser-extension/src/background.js
node --check apps/browser-extension/ui/popup.js
node --test apps/browser-extension/tests/core.test.mjs apps/browser-extension/tests/manifest.test.mjs
```

These verify Manifest V3, exact permissions, lack of broad host permissions/content scripts, CSP, protocol versioning, and site matching.

## Manual Chrome test

1. Run the DragonForge desktop app and unlock a development vault.
2. Add a login whose URL matches a test website.
3. Open `chrome://extensions`, enable Developer mode, and choose **Load unpacked**.
4. Select `apps/browser-extension` and copy its extension ID.
5. Run `.\scripts\install-browser-native-host.ps1 -ChromeExtensionId <ID>`.
6. Restart Chrome.
7. Visit the saved website, open DragonForge, and confirm the login appears without revealing its password.
8. Click **Fill** and confirm username/password fields are populated without submitting the form.
9. Lock DragonForge and confirm the extension reports the vault as locked.

## Manual Edge test

Repeat with `edge://extensions` and `.\scripts\install-browser-native-host.ps1 -EdgeExtensionId <ID>`.

## Security negative tests

- unrelated domains must not receive a login;
- subdomains are not treated as interchangeable;
- `file://`, `chrome://`, and `edge://` are rejected;
- changing sites before Fill must fail;
- locking or closing DragonForge must stop credential access.

Do not use real credentials during development testing.