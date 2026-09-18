param(
    [string]$ChromeExtensionId,
    [string]$EdgeExtensionId,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Validate-ExtensionId {
    param([string]$Id, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Id)) { return }
    if ($Id -notmatch '^[a-p]{32}$') {
        throw "$Label must be the 32-character extension ID shown on the browser extensions page."
    }
}

Validate-ExtensionId $ChromeExtensionId "ChromeExtensionId"
Validate-ExtensionId $EdgeExtensionId "EdgeExtensionId"

if ([string]::IsNullOrWhiteSpace($ChromeExtensionId) -and [string]::IsNullOrWhiteSpace($EdgeExtensionId)) {
    throw "Provide -ChromeExtensionId, -EdgeExtensionId, or both."
}

if (-not $SkipBuild) {
    cargo build -p dragonforge-desktop --release --bin dragonforge-native-host
    if ($LASTEXITCODE -ne 0) { throw "Native messaging host build failed." }
}

$SourceExe = Join-Path $RepoRoot "target\release\dragonforge-native-host.exe"
if (-not (Test-Path $SourceExe)) {
    throw "Native host binary was not found at $SourceExe"
}

$InstallDir = Join-Path $env:LOCALAPPDATA "DragonForge Password Manager\NativeMessaging"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$HostExe = Join-Path $InstallDir "dragonforge-native-host.exe"
$ManifestPath = Join-Path $InstallDir "com.dragonforge.passwordmanager.json"
Copy-Item -Force $SourceExe $HostExe

$AllowedOrigins = @()
if (-not [string]::IsNullOrWhiteSpace($ChromeExtensionId)) {
    $AllowedOrigins += "chrome-extension://$ChromeExtensionId/"
}
if (-not [string]::IsNullOrWhiteSpace($EdgeExtensionId)) {
    $AllowedOrigins += "chrome-extension://$EdgeExtensionId/"
}
$AllowedOrigins = @($AllowedOrigins | Select-Object -Unique)

$Manifest = [ordered]@{
    name = "com.dragonforge.passwordmanager"
    description = "DragonForge Password Manager native browser bridge"
    path = $HostExe
    type = "stdio"
    allowed_origins = $AllowedOrigins
}
$ManifestJson = $Manifest | ConvertTo-Json -Depth 4
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($ManifestPath, $ManifestJson, $Utf8NoBom)

if (-not [string]::IsNullOrWhiteSpace($ChromeExtensionId)) {
    $ChromeKey = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.dragonforge.passwordmanager"
    New-Item -Force -Path $ChromeKey | Out-Null
    Set-Item -Path $ChromeKey -Value $ManifestPath
    Write-Host "Registered DragonForge native messaging for Chrome: $ChromeExtensionId"
}

if (-not [string]::IsNullOrWhiteSpace($EdgeExtensionId)) {
    $EdgeKey = "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.dragonforge.passwordmanager"
    New-Item -Force -Path $EdgeKey | Out-Null
    Set-Item -Path $EdgeKey -Value $ManifestPath
    Write-Host "Registered DragonForge native messaging for Edge: $EdgeExtensionId"
}

Write-Host ""
Write-Host "Native host installed:"
Write-Host "  $HostExe"
Write-Host "Manifest:"
Write-Host "  $ManifestPath"
Write-Host ""
Write-Host "Restart the browser if DragonForge was already open in it."
