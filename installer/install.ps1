# Installs the Stellwerk agent {{VERSION}} for the current user with the Windows installer, pairs
# it with a Zentrale and starts it at every login. Attached to every release; the pairing dialog
# shows the command for PowerShell:
#
#   & ([scriptblock]::Create((irm https://github.com/firsttris/stellwerk/releases/download/v{{VERSION}}/install.ps1))) https://zentrale.example CODE [-CaSha256 FINGERPRINT]
#
# The script comes from GitHub, not from the Zentrale, so it also works where the Zentrale's
# certificate comes from a private CA the device does not trust yet: -CaSha256 lets the agent
# fetch that CA and trust it once its fingerprint matches. Without a code the device waits for
# approval under Devices (pairing in the LAN). Running it again on a paired device updates the
# agent and keeps the pairing. STELLWERK_RELEASES replaces the download location (tests).
param(
    [Parameter(Mandatory = $true)][string]$Server,
    [string]$Code = "",
    [string]$CaSha256 = ""
)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
# Windows PowerShell 5.1 may not offer TLS 1.2 by default; GitHub requires it.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$version = "{{VERSION}}"
$releases = if ($env:STELLWERK_RELEASES) { $env:STELLWERK_RELEASES } else { "https://github.com/firsttris/stellwerk/releases/download/v$version" }
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
if ($CaSha256) { $arguments += "/CASHA256=$CaSha256" }
$process = Start-Process -FilePath $setup -ArgumentList $arguments -Wait -PassThru
Remove-Item $setup -ErrorAction SilentlyContinue
if ($process.ExitCode -ne 0) { throw "the installer failed with exit code $($process.ExitCode)" }
Write-Host "Stellwerk agent $version installed and paired."
