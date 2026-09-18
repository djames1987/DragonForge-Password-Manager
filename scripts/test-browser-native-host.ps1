param(
    [ValidateSet("Edge", "Chrome")]
    [string]$Browser = "Edge",
    [string]$ExtensionId
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$HostName = "com.dragonforge.passwordmanager"
$Failed = $false

if (-not [string]::IsNullOrWhiteSpace($ExtensionId) -and $ExtensionId -notmatch '^[a-p]{32}$') {
    throw "ExtensionId must be the 32-character extension ID shown on the browser extensions page."
}

$ExpectedOrigin = $null
if (-not [string]::IsNullOrWhiteSpace($ExtensionId)) {
    $ExpectedOrigin = "chrome-extension://$ExtensionId/"
}

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
if ($null -ne $ExpectedOrigin) {
    Write-Host "Extension origin: $ExpectedOrigin"
}
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
$Origins = @()
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
    if ($Manifest.name -eq $HostName) {
        Pass "Native-host manifest name matches."
    }
    else {
        Fail "Native-host manifest name is '$($Manifest.name)', expected '$HostName'."
    }

    if ($Manifest.type -eq "stdio") {
        Pass "Native-host manifest communication type is stdio."
    }
    else {
        Fail "Native-host manifest type must be stdio."
    }

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
        foreach ($AllowedOrigin in $Origins) {
            Info "Allowed origin: $AllowedOrigin"
        }

        if ($null -ne $ExpectedOrigin) {
            if ($Origins -contains $ExpectedOrigin) {
                Pass "$Browser extension ID is explicitly allowed by the native-host manifest."
            }
            else {
                Fail "$Browser extension ID is not present in allowed_origins: $ExpectedOrigin"
            }
        }
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
        if ($null -ne $Client) {
            $Client.Dispose()
        }
    }
}

if ($null -ne $Manifest -and
    $null -ne $Endpoint -and
    -not [string]::IsNullOrWhiteSpace($Manifest.path)) {
    if ($null -ne $ExpectedOrigin) {
        $Origin = $ExpectedOrigin
    }
    else {
        $Origin = $Origins | Select-Object -First 1
        Info "No ExtensionId was supplied; using the first allowed origin for the direct native-host test."
    }

    if (-not [string]::IsNullOrWhiteSpace($Origin)) {
        $NativeProcess = $null
        try {
            $StartInfo = New-Object System.Diagnostics.ProcessStartInfo
            $StartInfo.FileName = [string]$Manifest.path
            $StartInfo.Arguments = '"' + [string]$Origin + '" --parent-window=0'
            $StartInfo.UseShellExecute = $false
            $StartInfo.RedirectStandardInput = $true
            $StartInfo.RedirectStandardOutput = $true
            $StartInfo.RedirectStandardError = $true
            $StartInfo.CreateNoWindow = $true

            $NativeProcess = New-Object System.Diagnostics.Process
            $NativeProcess.StartInfo = $StartInfo
            if (-not $NativeProcess.Start()) {
                throw "Failed to start native-host process."
            }
            Pass "Native-host process launched directly with browser-style arguments for $Origin"

            $RequestJson = '{"version":1,"action":"status"}'
            $Utf8 = New-Object System.Text.UTF8Encoding($false)
            $RequestBytes = $Utf8.GetBytes($RequestJson)
            $LengthBytes = [BitConverter]::GetBytes([uint32]$RequestBytes.Length)

            $InputStream = $NativeProcess.StandardInput.BaseStream
            $InputStream.Write($LengthBytes, 0, 4)
            $InputStream.Write($RequestBytes, 0, $RequestBytes.Length)
            $InputStream.Flush()

            $OutputStream = $NativeProcess.StandardOutput.BaseStream
            $Header = New-Object byte[] 4
            $Read = 0
            while ($Read -lt 4) {
                $Count = $OutputStream.Read($Header, $Read, 4 - $Read)
                if ($Count -le 0) {
                    throw "Native host closed stdout before returning a message header."
                }
                $Read += $Count
            }

            $ResponseLength = [BitConverter]::ToUInt32($Header, 0)
            if ($ResponseLength -eq 0 -or $ResponseLength -gt 1048576) {
                throw "Native host returned an invalid message length: $ResponseLength"
            }

            $ResponseBytes = New-Object byte[] $ResponseLength
            $Read = 0
            while ($Read -lt $ResponseLength) {
                $Count = $OutputStream.Read($ResponseBytes, $Read, $ResponseLength - $Read)
                if ($Count -le 0) {
                    throw "Native host closed stdout before returning the full message."
                }
                $Read += $Count
            }

            $NativeResponseJson = $Utf8.GetString($ResponseBytes)
            $NativeResponse = $NativeResponseJson | ConvertFrom-Json
            if ($NativeResponse.ok -eq $true -and $null -ne $NativeResponse.status) {
                Pass "Native-host framed status request succeeded end-to-end."
                Info "Native host reports vault unlocked: $($NativeResponse.status.unlocked)"
                Info "Native host reports vault item count: $($NativeResponse.status.itemCount)"
            }
            else {
                Fail "Native host returned an error response: $NativeResponseJson"
            }

            $NativeProcess.StandardInput.Close()
            if (-not $NativeProcess.WaitForExit(2000)) {
                $NativeProcess.Kill()
            }

            $Stderr = $NativeProcess.StandardError.ReadToEnd()
            if (-not [string]::IsNullOrWhiteSpace($Stderr)) {
                Info "Native host stderr: $Stderr"
            }
        }
        catch {
            $Stderr = ""
            if ($null -ne $NativeProcess) {
                try {
                    $Stderr = $NativeProcess.StandardError.ReadToEnd()
                }
                catch {}
                try {
                    if (-not $NativeProcess.HasExited) {
                        $NativeProcess.Kill()
                    }
                }
                catch {}
            }

            Fail "Native-host end-to-end framing test failed: $($_.Exception.Message)"
            if (-not [string]::IsNullOrWhiteSpace($Stderr)) {
                Info "Native host stderr: $Stderr"
            }
        }
        finally {
            if ($null -ne $NativeProcess) {
                $NativeProcess.Dispose()
            }
        }
    }
}

Write-Host ""
if ($Failed) {
    Write-Host "DRAGONFORGE BROWSER INTEGRATION DIAGNOSTIC: FAIL" -ForegroundColor Red
    exit 1
}

Write-Host "DRAGONFORGE BROWSER INTEGRATION DIAGNOSTIC: PASS" -ForegroundColor Green
exit 0
