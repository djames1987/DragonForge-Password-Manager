# Third-Party Notices

This repository is proprietary DragonForge source. The DragonForge `LICENSE` governs original DragonForge material only. Third-party software and material retain their own copyright and license terms and independently granted rights.

## Rust dependencies

The workspace resolves third-party Rust crates through Cargo and does not vendor their source in the current tree. Security-sensitive dependency areas include cryptography, post-quantum cryptography, password/key derivation, TLS/networking, database/storage, Tauri/desktop integration, and serialization/runtime support.

Reproducible owner-side inventory command:

```powershell
cargo metadata --locked --format-version 1 > dependency-metadata.json
```

Before distributing a public binary, inspect every resolved package whose `source` is non-null and record its `license` or `license_file`. Missing license metadata, an uncertain SPDX expression, or redistribution terms not reviewed for the intended artifact are release blockers. A compatible tool such as `cargo-deny` may be used in addition, with its configuration and output retained as release evidence.

Common permissive Rust license families such as MIT, Apache-2.0, BSD-style, ISC, and Unicode-related licenses may occur in Cargo graphs; this notice does not assert that a family is present unless the generated metadata for the exact release shows it.

## Browser extension

`apps/browser-extension/package.json` declares no npm runtime or development dependencies. No bundled third-party JavaScript library, web font, or icon package was identified in the extension tree during the Phase 3 inspection. The extension source remains subject to the repository license except where a file expressly says otherwise.

## Visual and documentation assets

The current tree contains `apps/desktop/icons/icon.png` and SVG diagrams under `docs/assets/product-overview/`. The repository files do not contain sufficient creator/source/license metadata to independently establish their redistribution provenance.

**Publication gate `DF-P3-ASSET-001`:** before making this repository public, the owner must confirm and record that these visual assets were created by DragonForge/the owner, validly commissioned/assigned, or otherwise redistributable under terms compatible with publication. If an asset came from an external source, record its source and license here and preserve required attribution; otherwise replace/remove it before publication.

## Relationship to DragonForge Security Suite

This standalone repository is the historical pre-migration source for the Password Manager. Active Password Manager development is maintained in DragonForge Security Suite, whose migration record identifies the frozen source baseline imported from this repository. That first-party migration does not erase any independently licensed third-party rights.

## Release rule

Do not distribute a public binary, installer, browser-extension archive, or server package until the exact resolved dependency graph has been reviewed and all required upstream notices/license texts for the distributed artifact have been collected. Uncertain or missing license information is a release blocker, not an implied permission grant.
