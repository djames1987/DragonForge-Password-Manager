param(
    [switch]$SkipRelease
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

$LogDirectory = Join-Path $RepoRoot "test-logs"
New-Item -ItemType Directory -Force -Path $LogDirectory | Out-Null

$Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$LogPath = Join-Path $LogDirectory "dragonforge-phase11-$Timestamp.log"
$HashPath = "$LogPath.sha256"

function Write-RawLog {
    param([string]$Message)
    Write-Host $Message
    Add-Content -LiteralPath $LogPath -Value $Message -Encoding UTF8
}

function Write-Log {
    param([string]$Message)
    Write-RawLog ("[{0}] {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"), $Message)
}

function Invoke-Logged {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Arguments
    )

    Write-Log "RUN: $Command $($Arguments -join ' ')"
    $previous = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $Command @Arguments 2>&1 | ForEach-Object { Write-RawLog $_.ToString() }
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previous
    }

    if ($null -eq $exitCode) { $exitCode = 0 }
    if ($exitCode -ne 0) {
        Write-Log "FAIL ($exitCode): $Command $($Arguments -join ' ')"
        throw "Command failed with exit code $exitCode"
    }
    Write-Log "PASS: $Command $($Arguments -join ' ')"
}

$Succeeded = $false
try {
    Write-Log "DragonForge Password Manager - Phase 11 automated verification"
    Write-Log "Repository: $RepoRoot"
    Write-Log "Release tests skipped: $SkipRelease"
    Write-Log "OS: $([System.Environment]::OSVersion.VersionString)"
    Write-Log "PowerShell: $($PSVersionTable.PSVersion)"

    Invoke-Logged git "rev-parse" "HEAD"
    Invoke-Logged rustc "--version" "--verbose"
    Invoke-Logged cargo "--version"
    Invoke-Logged node "--version"

    Write-Log "=== PowerShell syntax ==="
    $PowerShellFiles = @(
        "scripts/install-browser-native-host.ps1",
        "scripts/uninstall-browser-native-host.ps1",
        "scripts/test-browser-native-host.ps1",
        "scripts/run-phase6-tests.ps1",
        "scripts/run-phase7-tests.ps1",
        "scripts/run-phase8-tests.ps1",
        "scripts/run-phase9-tests.ps1",
        "scripts/run-phase10-tests.ps1",
        "scripts/run-phase11-tests.ps1"
    )
    foreach ($PowerShellFile in $PowerShellFiles) {
        $Tokens = $null
        $ParseErrors = $null
        [System.Management.Automation.Language.Parser]::ParseFile(
            (Resolve-Path $PowerShellFile),
            [ref]$Tokens,
            [ref]$ParseErrors
        ) | Out-Null

        if ($ParseErrors.Count -gt 0) {
            foreach ($ParseError in $ParseErrors) {
                Write-Log ("PowerShell parse error in {0}: {1}" -f $PowerShellFile, $ParseError.Message)
            }
            throw "PowerShell syntax validation failed."
        }
        Write-Log "PASS: PowerShell syntax $PowerShellFile"
    }

    Write-Log "=== Rust quality gates ==="
    Invoke-Logged cargo "fmt" "--all" "--check"
    Invoke-Logged cargo "clippy" "--workspace" "--all-targets" "--all-features" "--" "-D" "warnings"

    Write-Log "=== Desktop and browser JavaScript regression ==="
    Invoke-Logged node "--check" "apps/desktop/ui/app.js"
    Invoke-Logged node "--check" "apps/browser-extension/src/core.js"
    Invoke-Logged node "--check" "apps/browser-extension/src/background.js"
    Invoke-Logged node "--check" "apps/browser-extension/ui/popup.js"
    Invoke-Logged node "--test" "apps/browser-extension/tests/core.test.mjs" "apps/browser-extension/tests/manifest.test.mjs"

    Write-Log "=== Full debug workspace suite ==="
    Invoke-Logged cargo "test" "--workspace" "--all-features"

    Write-Log "=== Phase 11 Windows credential protection ==="
    if ($env:OS -eq "Windows_NT") {
        Invoke-Logged cargo "test" "-p" "dragonforge-desktop" "--test" "secret_storage" "--" "--nocapture"
    }
    else {
        Write-Log "INFO: Windows Credential Manager integration test skipped on non-Windows host."
    }

    Write-Log "=== Phase 10 secure recovery regression ==="
    Invoke-Logged cargo "test" "-p" "dragonforge-sync-server" "--test" "account_recovery" "--" "--nocapture"
    Invoke-Logged cargo "test" "-p" "dragonforge-desktop" "--test" "account_recovery" "--" "--nocapture"

    Write-Log "=== Phase 9 device enrollment regression ==="
    Invoke-Logged cargo "test" "-p" "dragonforge-sync-server" "--test" "device_enrollment" "--" "--nocapture"

    Write-Log "=== Phase 8 multi-device sync regression ==="
    Invoke-Logged cargo "test" "-p" "dragonforge-desktop" "--test" "multi_device_sync" "--" "--nocapture"

    Write-Log "=== Phase 7 sync server API ==="
    Invoke-Logged cargo "test" "-p" "dragonforge-sync-server" "--test" "sync_api" "--" "--nocapture"
    Invoke-Logged cargo "test" "-p" "dragonforge-sync-server" "--features" "postgres"

    Write-Log "=== Previous-phase regressions ==="
    Invoke-Logged cargo "test" "-p" "dragonforge-desktop" "--test" "desktop_service" "--" "--nocapture"
    Invoke-Logged cargo "test" "-p" "dragonforge-desktop" "--bin" "dragonforge-native-host" "--" "--nocapture"
    Invoke-Logged cargo "test" "-p" "dragonforge-vault" "--test" "hardening" "--" "--nocapture" "--test-threads=1"

    if (-not $SkipRelease) {
        Write-Log "=== Optimized release suite ==="
        Invoke-Logged cargo "test" "--workspace" "--all-features" "--release"

        Write-Log "=== Release binaries ==="
        Invoke-Logged cargo "build" "-p" "dragonforge-sync-server" "--features" "postgres" "--release"
        Invoke-Logged cargo "build" "-p" "dragonforge-desktop" "--release" "--bin" "dragonforge-desktop"
        Invoke-Logged cargo "build" "-p" "dragonforge-desktop" "--release" "--bin" "dragonforge-native-host"
    }

    Write-Log "=== Dependency visibility ==="
    Invoke-Logged cargo "tree" "-d"

    $CargoAudit = Get-Command cargo-audit -ErrorAction SilentlyContinue
    if ($null -ne $CargoAudit) {
        Write-Log "cargo-audit detected; running advisory scan"
        Invoke-Logged cargo "audit"
    }
    else {
        Write-Log "INFO: cargo-audit is not installed; advisory scan skipped."
    }

    $Succeeded = $true
    Write-Log "PHASE 11 AUTOMATED VERIFICATION: PASS"
}
catch {
    Write-Log "PHASE 11 AUTOMATED VERIFICATION: FAIL"
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

if (-not $Succeeded) { exit 1 }
