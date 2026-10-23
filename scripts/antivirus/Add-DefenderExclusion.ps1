<#
.SYNOPSIS
    Excludes one folder from Microsoft Defender's real-time scan, so that the AC10 sweeps can be measured without
    the scan of every modified file. Run it as administrator.

.DESCRIPTION
    Adds only the given folder (by default %TEMP%\krummi-av-bench, created if it does not exist) and checks that the
    exclusion is in place. Point the temporary folder of the tests at it (for example $env:TMP) to run the sweeps
    there. Remove the exclusion afterwards with Remove-DefenderExclusion.ps1.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/antivirus/Add-DefenderExclusion.ps1
#>
[CmdletBinding()]
param(
    [string]$Path = (Join-Path $env:TEMP 'krummi-av-bench')
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error 'Run this script as administrator.'
}
New-Item -ItemType Directory -Force -Path $Path | Out-Null
$folder = (Resolve-Path $Path).Path
Add-MpPreference -ExclusionPath $folder
if ((Get-MpPreference).ExclusionPath -contains $folder) {
    Write-Host "Exclusion added: $folder"
} else {
    Write-Error "The exclusion was not added (a policy may block it): $folder"
}
