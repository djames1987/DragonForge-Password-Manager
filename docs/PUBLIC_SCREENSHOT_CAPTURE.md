# Public Screenshot Capture Checklist

This historical repository contains existing icon and product-overview artwork whose public redistribution provenance remains unresolved under `DF-P3-ASSET-001`. Phase 5 therefore does not use those assets as the root landing-page visual and does not fabricate application screenshots.

## Capture set

1. `docs/assets/readme/desktop-vault-demo.png` — unlocked desktop layout with only synthetic logins/notes.
2. `docs/assets/readme/browser-fill-demo.png` — browser-extension popup on an `example.test` page using a synthetic account and no browser profile identity.
3. `docs/assets/readme/sync-devices-demo.png` — device/sync status with synthetic device names and no tokens, signing material, recovery generation values, or private endpoints.
4. Optional `docs/assets/readme/recovery-demo.png` — recovery workflow before any recovery kit, Account Secret, seed, token, or local private path is visible.

## Synthetic dataset

Use names such as `Example Bank`, `demo.user@example.test`, `https://login.example.test`, `Primary Demo Desktop`, and `Demo Laptop`. Password fields must stay masked. Do not enter or capture real credentials even temporarily.

## Sanitization and provenance

- Capture only application/browser content; exclude desktop notifications, terminal history, browser account chrome, and local filesystem dialogs.
- Never show master passwords, Account Secrets, vault plaintext beyond synthetic demo fields, recovery-kit strings, OTP seeds, sync tokens, ML-DSA private seeds, credential IDs, private server URLs, personal mailbox addresses, or real vault paths.
- Prefer PNG at 1600×900 or 1440×900 and optimize before committing.
- Record source commit, synthetic dataset, dimensions, optimized size, and reviewer in the public-readiness report.
- Existing `apps/desktop/icons/icon.png` and `docs/assets/product-overview/*.svg` remain blocked from public-facing reuse until `DF-P3-ASSET-001` is genuinely resolved.
