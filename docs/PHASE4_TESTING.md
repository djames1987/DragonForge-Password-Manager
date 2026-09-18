# Phase 4 Automated Verification

Phase 4 includes a noninteractive Windows verification runner that creates a logfile suitable for uploading for review.

## Run it

From PowerShell in the repository root:

```powershell
.\scripts\run-phase4-tests.ps1
```

A command wrapper is also provided:

```powershell
.\scripts\run-phase4-tests.cmd
```

No prompts or manual input are required.

## What it runs

The default run records environment/tool versions and executes:

1. Git commit identification.
2. Rust compiler/Cargo/Rustfmt/Clippy versions.
3. `cargo fmt --all --check`.
4. Clippy across all workspace targets/features with warnings denied.
5. Full debug workspace tests.
6. Phase 1 foundation regression tests.
7. Phase 2 post-quantum regression tests.
8. Phase 3 local-vault tests.
9. Phase 4 hardening tests three times, serially.
10. Full optimized release workspace tests.
11. Duplicate dependency visibility with `cargo tree -d`.
12. `cargo audit` if `cargo-audit` is already installed.

A missing `cargo-audit` installation is logged as informational and does not fail the run.

## Log output

Logs are written to:

```text
test-logs\dragonforge-phase4-YYYYMMDD-HHMMSS.log
```

A SHA-256 companion file is also written:

```text
test-logs\dragonforge-phase4-YYYYMMDD-HHMMSS.log.sha256
```

The script exits with a nonzero code if any required test or quality gate fails.

## Optional parameters

Repeat the hardening suite more times:

```powershell
.\scripts\run-phase4-tests.ps1 -HardeningRepeats 10
```

Skip release-mode testing for a quicker diagnostic pass:

```powershell
.\scripts\run-phase4-tests.ps1 -SkipRelease
```

The default full run is the recommended verification before uploading the logfile.

## Phase 4 hardening coverage

The hardening suite includes:

- format inspection without unlocking
- future-version rejection
- malformed vault UUID rejection
- zero-revision rejection
- duplicate item-ID rejection
- hostile oversized KDF parameter rejection before Argon2
- restoration from the last complete backup
- rejection/non-promotion of orphan temporary files
- 100-item encrypted CRUD/search persistence stress
- 50 consecutive encrypted updates to a single item
- final integrity verification after stress operations

Passing this suite is a development milestone, not a security audit.
