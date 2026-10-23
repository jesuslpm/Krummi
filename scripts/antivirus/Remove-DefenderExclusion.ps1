<#
.SYNOPSIS
    Removes the Microsoft Defender exclusion added by Add-DefenderExclusion.ps1. Run it as administrator.

.DESCRIPTION
    Removes only the given folder (by default %TEMP%\krummi-av-bench) from the exclusions and checks that it is gone.
    The folder itself is left in place.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/antivirus/Remove-DefenderExclusion.ps1
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
$folder = if (Test-Path $Path) { (Resolve-Path $Path).Path } else { $Path }
Remove-MpPreference -ExclusionPath $folder
if ((Get-MpPreference).ExclusionPath -contains $folder) {
    Write-Error "The exclusion is still there: $folder"
} else {
    Write-Host "Exclusion removed: $folder"
}
