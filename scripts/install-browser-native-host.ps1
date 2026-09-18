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
if (-not (Test-Path -LiteralPath $SourceExe)) {
    throw "Native host binary was not found at $SourceExe"
}

$InstallDir = Join-Path $env:LOCALAPPDATA "DragonForge Password Manager\NativeMessaging"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$HostExe = Join-Path $InstallDir "dragonforge-native-host.exe"
$ManifestPath = Join-Path $InstallDir "com.dragonforge.passwordmanager.json"
Copy-Item -Force -LiteralPath $SourceExe -Destination $HostExe

$AllowedOrigins = @()
if (Test-Path -LiteralPath $ManifestPath) {
    try {
        $ExistingManifest = Get-Content -Raw -LiteralPath $ManifestPath | ConvertFrom-Json
        if ($null -ne $ExistingManifest.allowed_origins) {
            $AllowedOrigins += @($ExistingManifest.allowed_origins)
        }
    }
    catch {
        Write-Warning "Existing DragonForge native host manifest could not be read; it will be replaced."
    }
}

if (-not [string]::IsNullOrWhiteSpace($ChromeExtensionId)) {
    $AllowedOrigins += "chrome-extension://$ChromeExtensionId/"
}
if (-not [string]::IsNullOrWhiteSpace($EdgeExtensionId)) {
    $AllowedOrigins += "chrome-extension://$EdgeExtensionId/"
}

$AllowedOrigins = @(
    $AllowedOrigins |
        Where-Object { $_ -match '^chrome-extension://[a-p]{32}/$' } |
        Select-Object -Unique
)

if ($AllowedOrigins.Count -eq 0) {
    throw "No valid browser extension origins were available for the native host manifest."
}

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

$WrittenManifest = Get-Content -Raw -LiteralPath $ManifestPath | ConvertFrom-Json
if ($WrittenManifest.name -ne "com.dragonforge.passwordmanager") {
    throw "Native host manifest validation failed."
}
if ($WrittenManifest.type -ne "stdio") {
    throw "Native host manifest has an invalid communication type."
}
if (-not (Test-Path -LiteralPath $WrittenManifest.path)) {
    throw "Native host manifest points to a missing executable: $($WrittenManifest.path)"
}

if (-not [string]::IsNullOrWhiteSpace($ChromeExtensionId)) {
    $ChromeKey = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.dragonforge.passwordmanager"
    New-Item -Force -Path $ChromeKey | Out-Null
    Set-Item -Path $ChromeKey -Value $ManifestPath
    $Registered = (Get-Item -LiteralPath $ChromeKey).GetValue("")
    if ($Registered -ne $ManifestPath) { throw "Chrome native messaging registry verification failed." }
    Write-Host "Registered DragonForge native messaging for Chrome: $ChromeExtensionId"
}

if (-not [string]::IsNullOrWhiteSpace($EdgeExtensionId)) {
    $EdgeKey = "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.dragonforge.passwordmanager"
    New-Item -Force -Path $EdgeKey | Out-Null
    Set-Item -Path $EdgeKey -Value $ManifestPath
    $Registered = (Get-Item -LiteralPath $EdgeKey).GetValue("")
    if ($Registered -ne $ManifestPath) { throw "Edge native messaging registry verification failed." }
    Write-Host "Registered DragonForge native messaging for Edge: $EdgeExtensionId"
}

Write-Host ""
Write-Host "Native host installed:"
Write-Host "  $HostExe"
Write-Host "Manifest:"
Write-Host "  $ManifestPath"
Write-Host "Allowed origins:"
foreach ($Origin in $AllowedOrigins) { Write-Host "  $Origin" }
Write-Host ""
if (-not [string]::IsNullOrWhiteSpace($ChromeExtensionId)) {
    Write-Host "Verify Chrome with:"
    Write-Host "  .\scripts\test-browser-native-host.ps1 -Browser Chrome -ExtensionId $ChromeExtensionId"
}
if (-not [string]::IsNullOrWhiteSpace($EdgeExtensionId)) {
    Write-Host "Verify Edge with:"
    Write-Host "  .\scripts\test-browser-native-host.ps1 -Browser Edge -ExtensionId $EdgeExtensionId"
}
Write-Host "Restart each configured browser if DragonForge was already open in it."
