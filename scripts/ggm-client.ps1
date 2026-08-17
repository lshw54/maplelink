<#
.SYNOPSIS
    Print the client-integrity values TW launches send, read from an installed
    Gamania Games Manager.

.DESCRIPTION
    beanfun's TW credential endpoint asks the caller to state which build of the
    game manager is asking: its version, and the SHA-256 of GGMWebStart.dll.
    MapleLink ships a known-good pair and can also read them off an installed
    manager, but when beanfun starts requiring a newer pair somebody has to
    write the new one down.

    This is that step. Run it on a machine with a current game manager and it
    prints the pair, ready to paste into ggm-client.json.

.PARAMETER Dll
    Path to GGMWebStart.dll. Located automatically when omitted.

.PARAMETER Write
    Write ggm-client.json to the repository root instead of printing it.

.PARAMETER Pin
    Write the file to MapleLink's data folder with "override": true, pinning
    these values on this machine only.

.EXAMPLE
    .\ggm-client.ps1
    .\ggm-client.ps1 -Write
    .\ggm-client.ps1 -Dll "D:\GGM\GGMWebStart.dll" -Pin
#>
[CmdletBinding()]
param(
    [string]$Dll,
    [switch]$Write,
    [switch]$Pin
)

$ErrorActionPreference = 'Stop'

function Find-GgmDll {
    # The manager's own key first, the protocol handler second: either can be
    # missing on an install that otherwise works.
    foreach ($view in @('Registry64', 'Registry32')) {
        try {
            $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine', $view)
            $key = $base.OpenSubKey('SOFTWARE\gamaniaGamesManager')
            if ($key) {
                $install = $key.GetValue('InstallPath')
                if ($install) {
                    $candidate = Join-Path $install 'GGMWebStart.dll'
                    if (Test-Path $candidate) { return $candidate }
                }
            }
        } catch { }
    }

    try {
        $command = (Get-ItemProperty 'Registry::HKEY_CLASSES_ROOT\gamaniagames\shell\open\command' -ErrorAction Stop).'(default)'
        if ($command -match '"([^"]+GGMWebStart\.exe)"') {
            $candidate = Join-Path (Split-Path $matches[1]) 'GGMWebStart.dll'
            if (Test-Path $candidate) { return $candidate }
        }
    } catch { }

    return $null
}

function Write-JsonFile([string]$Path, [string]$Content) {
    # Not Set-Content -Encoding utf8: on Windows PowerShell that writes a BOM,
    # and a BOM makes the file unparseable as JSON. Published, it would take the
    # values away from every user; pinned, it would silently do nothing.
    [System.IO.File]::WriteAllText($Path, $Content, (New-Object System.Text.UTF8Encoding($false)))
}

if (-not $Dll) { $Dll = Find-GgmDll }

if (-not $Dll -or -not (Test-Path $Dll)) {
    Write-Host "GGMWebStart.dll not found." -ForegroundColor Yellow
    Write-Host "Install the Gamania Games Manager from https://tw.beanfun.com/ggm/index.aspx,"
    Write-Host "or pass the path:  .\ggm-client.ps1 -Dll <path to GGMWebStart.dll>"
    exit 1
}

$item = Get-Item $Dll
$cv = $item.VersionInfo.FileVersion
$hash = (Get-FileHash $Dll -Algorithm SHA256).Hash.ToLower()

# The build beanfun currently announces. Recorded so the watcher has something
# to compare against, and worth printing: if it disagrees with the DLL on this
# machine, the manager here is out of date and these values are the wrong ones
# to publish.
$installer = ''
$announced = ''
try {
    $check = Invoke-RestMethod -Uri 'https://tw.beanfun.com/generic_handlers/CheckVersion.ashx' -TimeoutSec 20
    $announced = $check.version
    if ($check.url) { $installer = Split-Path $check.url -Leaf }
} catch {
    Write-Host "Could not reach beanfun to read the announced version." -ForegroundColor Yellow
}

if ($announced -and $announced -ne $cv) {
    Write-Host "beanfun announces $announced but this machine has $cv." -ForegroundColor Yellow
    Write-Host "Update the Gamania Games Manager before publishing these values." -ForegroundColor Yellow
    Write-Host ""
}

$fields = @()
if ($installer) { $fields += "  `"installer`": `"$installer`"" }
if ($Pin)       { $fields += "  `"override`": true" }
$fields += "  `"cv`": `"$cv`""
$fields += "  `"hash`": `"$hash`""
$json = "{`n" + ($fields -join ",`n") + "`n}`n"

Write-Host "GGMWebStart.dll : $Dll"
Write-Host "version         : $cv"
Write-Host "sha256          : $hash"
Write-Host "installer       : $(if ($installer) { $installer } else { '(unknown)' })"
Write-Host "beanfun says    : $(if ($announced) { $announced } else { '(unknown)' })"
Write-Host ""

if ($Pin) {
    $dir = Join-Path $env:APPDATA 'com.maplelink.app'
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $out = Join-Path $dir 'ggm-client.json'
    Write-JsonFile $out $json
    Write-Host "Pinned on this machine: $out" -ForegroundColor Green
    Write-Host "Delete that file to follow the published values again."
} elseif ($Write) {
    $out = Join-Path (Split-Path $PSScriptRoot -Parent) 'ggm-client.json'
    Write-JsonFile $out $json
    Write-Host "Written to $out" -ForegroundColor Green
    Write-Host "Commit it to publish these values to every user."
} else {
    Write-Host "ggm-client.json:" -ForegroundColor Green
    Write-Host $json
    Write-Host "Re-run with -Write to publish, or -Pin to apply on this machine only."
}
