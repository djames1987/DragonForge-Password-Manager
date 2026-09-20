param(
    [switch]$Apply,
    [switch]$CleanCargoTarget,
    [switch]$PruneTestLogs,
    [ValidateRange(1, 3650)]
    [int]$LogRetentionDays = 30
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Format-Bytes {
    param([long]$Bytes)
    if ($Bytes -ge 1TB) { return "{0:N2} TB" -f ($Bytes / 1TB) }
    if ($Bytes -ge 1GB) { return "{0:N2} GB" -f ($Bytes / 1GB) }
    if ($Bytes -ge 1MB) { return "{0:N2} MB" -f ($Bytes / 1MB) }
    if ($Bytes -ge 1KB) { return "{0:N2} KB" -f ($Bytes / 1KB) }
    return "$Bytes B"
}

function Get-DirectorySize {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return 0L
    }

    $sum = (Get-ChildItem -LiteralPath $Path -File -Recurse -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0L }
    return [long]$sum
}

function Show-Entry {
    param(
        [string]$Label,
        [string]$Path
    )

    $size = Get-DirectorySize $Path
    Write-Host ("{0,-24} {1,12}  {2}" -f $Label, (Format-Bytes $size), $Path)
    return $size
}

Write-Host ""
Write-Host "DragonForge workspace cleanup"
Write-Host "Repository: $RepoRoot"
Write-Host "Mode: $(if ($Apply) { 'APPLY' } else { 'DRY RUN' })"
Write-Host ""

$targetPath = Join-Path $RepoRoot "target"
$logsPath = Join-Path $RepoRoot "test-logs"
$gitPath = Join-Path $RepoRoot ".git"

Write-Host "Current disk usage:"
$targetSize = Show-Entry "Cargo target" $targetPath
$logSize = Show-Entry "Test logs" $logsPath
$gitSize = Show-Entry "Git metadata" $gitPath

$rootFilesSize = (Get-ChildItem -LiteralPath $RepoRoot -File -Force -ErrorAction SilentlyContinue |
    Measure-Object -Property Length -Sum).Sum
if ($null -eq $rootFilesSize) { $rootFilesSize = 0 }
Write-Host ("{0,-24} {1,12}" -f "Root files", (Format-Bytes ([long]$rootFilesSize)))
Write-Host ""

$totalRecoverable = 0L

if ($CleanCargoTarget) {
    $totalRecoverable += $targetSize
    if (Test-Path -LiteralPath $targetPath) {
        if ($Apply) {
            Write-Host "Removing Cargo build artifacts with 'cargo clean'..."
            & cargo clean
            if ($LASTEXITCODE -ne 0) {
                throw "cargo clean failed with exit code $LASTEXITCODE"
            }
            Write-Host "Cargo build artifacts removed."
        }
        else {
            Write-Host ("DRY RUN: would remove Cargo target artifacts ({0})." -f (Format-Bytes $targetSize))
        }
    }
    else {
        Write-Host "Cargo target directory is already absent."
    }
}

if ($PruneTestLogs) {
    if (Test-Path -LiteralPath $logsPath) {
        $cutoff = (Get-Date).AddDays(-$LogRetentionDays)
        $oldLogs = @(Get-ChildItem -LiteralPath $logsPath -File -Force |
            Where-Object { $_.LastWriteTime -lt $cutoff })

        $oldLogBytes = ($oldLogs | Measure-Object -Property Length -Sum).Sum
        if ($null -eq $oldLogBytes) { $oldLogBytes = 0L }
        $totalRecoverable += [long]$oldLogBytes

        if ($oldLogs.Count -eq 0) {
            Write-Host "No test logs older than $LogRetentionDays days."
        }
        elseif ($Apply) {
            Write-Host ("Removing {0} old test-log file(s), totaling {1}..." -f $oldLogs.Count, (Format-Bytes ([long]$oldLogBytes)))
            $oldLogs | Remove-Item -Force
        }
        else {
            Write-Host ("DRY RUN: would remove {0} test-log file(s) older than {1} days ({2})." -f $oldLogs.Count, $LogRetentionDays, (Format-Bytes ([long]$oldLogBytes)))
        }
    }
    else {
        Write-Host "Test-log directory is already absent."
    }
}

if (-not $CleanCargoTarget -and -not $PruneTestLogs) {
    Write-Host "No cleanup action selected."
    Write-Host "Use -CleanCargoTarget and/or -PruneTestLogs."
}

Write-Host ""
if ($Apply) {
    Write-Host "Cleanup complete."
}
else {
    Write-Host ("Potential selected cleanup: {0}" -f (Format-Bytes $totalRecoverable))
    Write-Host "Nothing was deleted. Re-run with -Apply to perform the selected cleanup."
}

Write-Host ""
Write-Host "Safety boundaries:"
Write-Host "  - Never deletes .git or source files."
Write-Host "  - Never deletes .dfvault files, vault backups, or sync sidecars."
Write-Host "  - Never changes Windows Credential Manager."
Write-Host "  - Test logs are deleted only when -PruneTestLogs is explicitly selected."
