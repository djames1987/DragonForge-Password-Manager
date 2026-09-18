param(
    [string]$OutputDirectory = "dist"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

node --check apps/browser-extension/src/core.js
if ($LASTEXITCODE -ne 0) { throw "Extension core syntax check failed." }
node --check apps/browser-extension/src/background.js
if ($LASTEXITCODE -ne 0) { throw "Extension background syntax check failed." }
node --check apps/browser-extension/ui/popup.js
if ($LASTEXITCODE -ne 0) { throw "Extension popup syntax check failed." }
node --test apps/browser-extension/tests/*.test.mjs
if ($LASTEXITCODE -ne 0) { throw "Extension tests failed." }

$OutputRoot = Join-Path $RepoRoot $OutputDirectory
New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null
$ZipPath = Join-Path $OutputRoot "dragonforge-browser-extension.zip"
Remove-Item -Force -ErrorAction SilentlyContinue $ZipPath

$ExtensionRoot = Join-Path $RepoRoot "apps\browser-extension"
$Files = @(
    (Join-Path $ExtensionRoot "manifest.json"),
    (Join-Path $ExtensionRoot "src"),
    (Join-Path $ExtensionRoot "ui")
)
Compress-Archive -Path $Files -DestinationPath $ZipPath -CompressionLevel Optimal

Write-Host "Browser extension package created:"
Write-Host "  $ZipPath"
