param(
    [ValidateRange(1, 20)]
    [int]$HardeningRepeats = 3,
    [switch]$SkipRelease
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

$LogDirectory = Join-Path $RepoRoot "test-logs"
New-Item -ItemType Directory -Force -Path $LogDirectory | Out-Null

$Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$LogPath = Join-Path $LogDirectory "dragonforge-phase4-$Timestamp.log"
$HashPath = "$LogPath.sha256"

function Write-Log {
    param([string]$Message)
    $line = "[{0}] {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"), $Message
    $line | Tee-Object -FilePath $LogPath -Append
}

function Invoke-Logged {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Arguments
    )

    Write-Log "RUN: $Command $($Arguments -join ' ')"
    & $Command @Arguments 2>&1 | ForEach-Object {
        $_.ToString() | Tee-Object -FilePath $LogPath -Append
    }

    $exitCode = $LASTEXITCODE
    if ($null -eq $exitCode) {
        $exitCode = 0
    }

    if ($exitCode -ne 0) {
        Write-Log "FAIL ($exitCode): $Command $($Arguments -join ' ')"
        throw "Command failed with exit code $exitCode"
    }

    Write-Log "PASS: $Command $($Arguments -join ' ')"
}

$Succeeded = $false

try {
    Write-Log "DragonForge Password Manager - Phase 4 automated verification"
    Write-Log "Repository: $RepoRoot"
    Write-Log "Hardening repeats: $HardeningRepeats"
    Write-Log "Release tests skipped: $SkipRelease"
    Write-Log "OS: $([System.Environment]::OSVersion.VersionString)"
    Write-Log "PowerShell: $($PSVersionTable.PSVersion)"

    Invoke-Logged git "rev-parse" "HEAD"
    Invoke-Logged rustc "--version" "--verbose"
    Invoke-Logged cargo "--version"
    Invoke-Logged rustfmt "--version"
    Invoke-Logged cargo "clippy" "--version"

    Write-Log "=== Static quality gates ==="
    Invoke-Logged cargo "fmt" "--all" "--check"
    Invoke-Logged cargo "clippy" "--workspace" "--all-targets" "--all-features" "--" "-D" "warnings"

    Write-Log "=== Full debug workspace suite ==="
    Invoke-Logged cargo "test" "--workspace" "--all-features"

    Write-Log "=== Explicit regression suites ==="
    Invoke-Logged cargo "test" "-p" "dragonforge-crypto" "--test" "foundation" "--" "--nocapture"
    Invoke-Logged cargo "test" "-p" "dragonforge-crypto" "--test" "post_quantum" "--" "--nocapture"
    Invoke-Logged cargo "test" "-p" "dragonforge-vault" "--test" "local_vault" "--" "--nocapture"

    Write-Log "=== Phase 4 hardening suite ==="
    for ($run = 1; $run -le $HardeningRepeats; $run++) {
        Write-Log "Hardening pass $run of $HardeningRepeats"
        Invoke-Logged cargo "test" "-p" "dragonforge-vault" "--test" "hardening" "--" "--nocapture" "--test-threads=1"
    }

    if (-not $SkipRelease) {
        Write-Log "=== Optimized release suite ==="
        Invoke-Logged cargo "test" "--workspace" "--all-features" "--release"
    }

    Write-Log "=== Dependency visibility ==="
    Invoke-Logged cargo "tree" "-d"

    $CargoAudit = Get-Command cargo-audit -ErrorAction SilentlyContinue
    if ($null -ne $CargoAudit) {
        Write-Log "cargo-audit detected; running advisory scan"
        Invoke-Logged cargo "audit"
    }
    else {
        Write-Log "INFO: cargo-audit is not installed; advisory scan skipped. This does not fail the test run."
    }

    $Succeeded = $true
    Write-Log "PHASE 4 AUTOMATED VERIFICATION: PASS"
}
catch {
    Write-Log "PHASE 4 AUTOMATED VERIFICATION: FAIL"
    Write-Log "ERROR: $($_.Exception.Message)"
}
finally {
    Write-Log "Log file: $LogPath"
    $hash = (Get-FileHash -Algorithm SHA256 -Path $LogPath).Hash
    "$hash  $(Split-Path -Leaf $LogPath)" | Set-Content -Encoding ASCII $HashPath
    Write-Host ""
    Write-Host "Log:  $LogPath"
    Write-Host "SHA:  $HashPath"
}

if (-not $Succeeded) {
    exit 1
}
