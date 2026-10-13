# Installs the Stellwerk agent {{VERSION}} for the current user with the Windows installer, pairs
# it with a Zentrale and starts it at every login. The Zentrale serves this script at
# /install.ps1; in PowerShell:
#
#   & ([scriptblock]::Create((irm https://zentrale.example/install.ps1))) https://zentrale.example CODE
#
# Without a code the device waits for approval under Devices (pairing in the LAN). Running it
# again on a paired device updates the agent and keeps the pairing.
param(
    [Parameter(Mandatory = $true)][string]$Server,
    [string]$Code = ""
)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
# Windows PowerShell 5.1 may not offer TLS 1.2 by default; GitHub requires it.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$version = "{{VERSION}}"
$releases = "https://github.com/firsttris/stellwerk/releases/download/v$version"
$asset = "stellwerk-agent-setup-windows-x86_64.exe"
$setup = Join-Path ([IO.Path]::GetTempPath()) $asset

Write-Host "Downloading $asset $version ..."
Invoke-WebRequest -Uri "$releases/$asset" -OutFile $setup -UseBasicParsing
$sums = (Invoke-WebRequest -Uri "$releases/SHA256SUMS" -UseBasicParsing).Content
if ($sums -is [byte[]]) { $sums = [Text.Encoding]::ASCII.GetString($sums) }
$line = $sums -split "`n" | Where-Object { $_.Trim() -match "\s\*?$([regex]::Escape($asset))$" } | Select-Object -First 1
if (-not $line) { throw "$asset is not in the checksums of v$version" }
$expected = ($line.Trim() -split "\s+")[0].ToLower()
$actual = (Get-FileHash -Path $setup -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $expected) { throw "the download does not match its checksum" }

$arguments = @("/S", "/SERVER=$Server")
if ($Code) { $arguments += "/CODE=$Code" }
$process = Start-Process -FilePath $setup -ArgumentList $arguments -Wait -PassThru
Remove-Item $setup -ErrorAction SilentlyContinue
if ($process.ExitCode -ne 0) { throw "the installer failed with exit code $($process.ExitCode)" }
Write-Host "Stellwerk agent $version installed and paired."
