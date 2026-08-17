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
    Write ggm-client.json beside this script instead of printing it.

.EXAMPLE
    .\ggm-client.ps1
    .\ggm-client.ps1 -Dll "D:\GGM\GGMWebStart.dll" -Write
#>
[CmdletBinding()]
param(
    [string]$Dll,
    [switch]$Write
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

$json = @"
{
  "cv": "$cv",
  "hash": "$hash"
}
"@

Write-Host "GGMWebStart.dll : $Dll"
Write-Host "version         : $cv"
Write-Host "sha256          : $hash"
Write-Host ""

if ($Write) {
    $out = Join-Path $PSScriptRoot 'ggm-client.json'
    Set-Content -Path $out -Value $json -Encoding utf8
    Write-Host "Written to $out" -ForegroundColor Green
} else {
    Write-Host "ggm-client.json:" -ForegroundColor Green
    Write-Host $json
    Write-Host ""
    Write-Host "Publish it by committing this as ggm-client.json at the repo root."
    Write-Host "To pin these values on this machine only, add `"override`": true and save it to:"
    Write-Host "  $env:APPDATA\com.maplelink.app\ggm-client.json"
}
