param(
    [ValidateSet("Chrome", "Edge", "Both")]
    [string]$Browser = "Both"
)

$ErrorActionPreference = "Stop"

if ($Browser -eq "Chrome" -or $Browser -eq "Both") {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.dragonforge.passwordmanager"
}
if ($Browser -eq "Edge" -or $Browser -eq "Both") {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.dragonforge.passwordmanager"
}

Write-Host "DragonForge native messaging registration removed for $Browser."
