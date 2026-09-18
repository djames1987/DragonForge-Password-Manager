param(
    [ValidateSet("Edge", "Chrome")]
    [string]$Browser = "Edge"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$HostName = "com.dragonforge.passwordmanager"
$Failed = $false

function Pass([string]$Message) {
    Write-Host "[PASS] $Message" -ForegroundColor Green
}
function Fail([string]$Message) {
    Write-Host "[FAIL] $Message" -ForegroundColor Red
    $script:Failed = $true
}
function Info([string]$Message) {
    Write-Host "[INFO] $Message"
}

if ($Browser -eq "Edge") {
    $RegistryPath = "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$HostName"
}
else {
    $RegistryPath = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$HostName"
}

Write-Host "DragonForge browser integration diagnostic"
Write-Host "Browser: $Browser"
Write-Host ""

$ManifestPath = $null
if (Test-Path -LiteralPath $RegistryPath) {
    $ManifestPath = (Get-Item -LiteralPath $RegistryPath).GetValue("")
    if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
        Fail "$Browser registry key exists but the default manifest path is empty."
    }
    else {
        Pass "$Browser native-messaging registry key exists."
        Info "Manifest path: $ManifestPath"
    }
}
else {
    Fail "$Browser native-messaging registry key is missing: $RegistryPath"
}

$Manifest = $null
if (-not [string]::IsNullOrWhiteSpace($ManifestPath)) {
    if (Test-Path -LiteralPath $ManifestPath) {
        Pass "Native-host manifest exists."
        try {
            $Manifest = Get-Content -Raw -LiteralPath $ManifestPath | ConvertFrom-Json
            Pass "Native-host manifest is valid JSON."
        }
        catch {
            Fail "Native-host manifest is not valid JSON: $($_.Exception.Message)"
        }
    }
    else {
        Fail "Native-host manifest does not exist: $ManifestPath"
    }
}

if ($null -ne $Manifest) {
    if ($Manifest.name -eq $HostName) { Pass "Native-host manifest name matches." }
    else { Fail "Native-host manifest name is '$($Manifest.name)', expected '$HostName'." }

    if ($Manifest.type -eq "stdio") { Pass "Native-host manifest communication type is stdio." }
    else { Fail "Native-host manifest type must be stdio." }

    if (-not [string]::IsNullOrWhiteSpace($Manifest.path) -and (Test-Path -LiteralPath $Manifest.path)) {
        Pass "Native-host executable exists."
        Info "Host executable: $($Manifest.path)"
    }
    else {
        Fail "Native-host executable is missing: $($Manifest.path)"
    }

    $Origins = @($Manifest.allowed_origins)
    if ($Origins.Count -gt 0) {
        Pass "Native-host manifest contains allowed extension origins."
        foreach ($Origin in $Origins) { Info "Allowed origin: $Origin" }
    }
    else {
        Fail "Native-host manifest has no allowed_origins entries."
    }
}

$BridgePath = Join-Path $env:LOCALAPPDATA "DragonForge Password Manager\bridge.json"
$Endpoint = $null
if (Test-Path -LiteralPath $BridgePath) {
    Pass "Desktop bridge endpoint file exists."
    Info "Bridge file: $BridgePath"
    try {
        $Endpoint = Get-Content -Raw -LiteralPath $BridgePath | ConvertFrom-Json
        Pass "Desktop bridge endpoint is valid JSON."
    }
    catch {
        Fail "Desktop bridge endpoint is not valid JSON: $($_.Exception.Message)"
    }
}
else {
    Fail "Desktop bridge endpoint is missing. Start the current DragonForge desktop app and keep it running."
}

if ($null -ne $Endpoint) {
    if ($Endpoint.version -eq 1 -and $Endpoint.port -gt 0 -and $Endpoint.token -match '^[0-9a-fA-F]{64}$') {
        Pass "Desktop bridge endpoint metadata is structurally valid."
        Info "Desktop PID: $($Endpoint.pid)"
        Info "Bridge port: $($Endpoint.port)"
    }
    else {
        Fail "Desktop bridge endpoint metadata is malformed."
    }

    try {
        $Process = Get-Process -Id ([int]$Endpoint.pid) -ErrorAction Stop
        Pass "Desktop bridge process is running (PID $($Endpoint.pid), $($Process.ProcessName))."
    }
    catch {
        Fail "Desktop bridge process PID $($Endpoint.pid) is not running."
    }

    $Client = $null
    try {
        $Client = New-Object System.Net.Sockets.TcpClient
        $Async = $Client.BeginConnect("127.0.0.1", [int]$Endpoint.port, $null, $null)
        if (-not $Async.AsyncWaitHandle.WaitOne(2000)) {
            throw "Timed out connecting to 127.0.0.1:$($Endpoint.port)"
        }
        $Client.EndConnect($Async)
        Pass "Connected to the desktop loopback bridge."

        $Envelope = [ordered]@{
            token = [string]$Endpoint.token
            request = [ordered]@{
                version = 1
                action = "status"
            }
        }
        $Json = ($Envelope | ConvertTo-Json -Depth 6 -Compress) + [Environment]::NewLine
        $Utf8 = New-Object System.Text.UTF8Encoding($false)
        $Bytes = $Utf8.GetBytes($Json)

        $Stream = $Client.GetStream()
        $Stream.WriteTimeout = 3000
        $Stream.ReadTimeout = 3000
        $Stream.Write($Bytes, 0, $Bytes.Length)
        $Stream.Flush()

        $Reader = New-Object System.IO.StreamReader($Stream, $Utf8, $false, 1024, $true)
        $Line = $Reader.ReadLine()
        if ([string]::IsNullOrWhiteSpace($Line)) {
            throw "Desktop bridge returned an empty response."
        }

        $Response = $Line | ConvertFrom-Json
        if ($Response.ok -eq $true -and $null -ne $Response.status) {
            Pass "Authenticated desktop bridge status request succeeded."
            Info "Vault unlocked: $($Response.status.unlocked)"
            Info "Vault item count: $($Response.status.itemCount)"
        }
        else {
            Fail "Desktop bridge returned an error: $Line"
        }
    }
    catch {
        Fail "Desktop bridge connectivity/status test failed: $($_.Exception.Message)"
    }
    finally {
        if ($null -ne $Client) { $Client.Dispose() }
    }
}

Write-Host ""
if ($Failed) {
    Write-Host "DRAGONFORGE BROWSER INTEGRATION DIAGNOSTIC: FAIL" -ForegroundColor Red
    exit 1
}
Write-Host "DRAGONFORGE BROWSER INTEGRATION DIAGNOSTIC: PASS" -ForegroundColor Green
exit 0
