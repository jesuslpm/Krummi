<#
.SYNOPSIS
    Restores the power plan that was active before Enter-BenchPowerPlan.ps1 and removes the "Krummi bench" plan.

.DESCRIPTION
    Activates the plan saved in %LOCALAPPDATA%\krummi-bench-power.txt, checks that it is active, deletes the bench plan
    and the saved file.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/power/Exit-BenchPowerPlan.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

# The plan GUID, the saved-plan file and the two functions below are the same in Enter-BenchPowerPlan.ps1: change
# both scripts together.
$bench = '6c0f3a52-1d7e-4b8a-9f21-4b7d2e9c5a10'
$state = Join-Path $env:LOCALAPPDATA 'krummi-bench-power.txt'
$guid = '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'

function Invoke-PowerCfg {
    $out = & powercfg.exe @args 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "powercfg $($args -join ' ') failed: $out" }
    $out
}

function Get-ActiveScheme {
    $out = Invoke-PowerCfg /getactivescheme
    if ($out -notmatch '[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}') { throw "Unexpected powercfg output: $out" }
    $Matches[0].ToLowerInvariant()
}

if (-not (Test-Path $state)) {
    throw "No saved plan in ${state}: run Enter-BenchPowerPlan.ps1 first, or activate your usual plan by hand."
}
$saved = (Get-Content $state -Raw).Trim().ToLowerInvariant()
if ($saved -notmatch $guid -or $saved -eq $bench) {
    throw "$state does not hold a usable plan ('$saved'): activate your usual plan by hand."
}
Invoke-PowerCfg /setactive $saved | Out-Null
if ((Get-ActiveScheme) -ne $saved) { throw "The saved plan $saved could not be activated." }
if ((Invoke-PowerCfg /list) -match $bench) { Invoke-PowerCfg /delete $bench | Out-Null }
Remove-Item $state
Write-Host "Power plan restored: $saved"
