# Workspace Cleanup and Disk Usage

DragonForge's Rust/Tauri test and release builds can consume many gigabytes under the workspace-level `target` directory. Cargo build output is reproducible and is already ignored by Git.

The cleanup helper is intentionally conservative. It does not delete vaults, sync sidecars, source files, Git metadata, or Windows Credential Manager entries.

## Inspect disk usage without deleting anything

```powershell
.\scripts\cleanup-workspace.ps1
```

## Preview Cargo cleanup

```powershell
.\scripts\cleanup-workspace.ps1 -CleanCargoTarget
```

This is a dry run. It reports how much space is in `target`.

## Remove Cargo build artifacts

```powershell
.\scripts\cleanup-workspace.ps1 -CleanCargoTarget -Apply
```

Internally this uses:

```powershell
cargo clean
```

The next Rust build/test will take longer because dependencies must be rebuilt.

## Preview old test-log pruning

The default retention period is 30 days:

```powershell
.\scripts\cleanup-workspace.ps1 -PruneTestLogs
```

Choose another retention period:

```powershell
.\scripts\cleanup-workspace.ps1 -PruneTestLogs -LogRetentionDays 14
```

Apply it:

```powershell
.\scripts\cleanup-workspace.ps1 -PruneTestLogs -LogRetentionDays 14 -Apply
```

## Clean both

```powershell
.\scripts\cleanup-workspace.ps1 -CleanCargoTarget -PruneTestLogs -LogRetentionDays 30 -Apply
```

## What is deliberately preserved

The helper never removes:

- `.git`;
- Rust/JavaScript/HTML/CSS source;
- documentation;
- `Cargo.toml` or lockfiles;
- `*.dfvault`;
- `*.dfvault.bak`;
- `*.dfvault.tmp`;
- sync sidecars;
- Windows Credential Manager records.

Vault-related temporary/backup files are deliberately excluded from automatic cleanup because they may be needed for recovery after an interrupted write.

## Why the workspace gets large

The top-level Cargo workspace uses one `target` directory. Over repeated phases it can contain:

- debug binaries;
- release binaries;
- test binaries;
- incremental compilation state;
- build-script output;
- multiple feature-set builds;
- duplicate dependency artifacts produced by different profiles/features.

A full `cargo clean` is therefore the safest way to reclaim most development disk usage.
