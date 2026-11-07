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

$bench = '6c0f3a52-1d7e-4b8a-9f21-4b7d2e9c5a10'
$state = Join-Path $env:LOCALAPPDATA 'krummi-bench-power.txt'

function Invoke-PowerCfg {
    $out = & powercfg.exe @args 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "powercfg $($args -join ' ') failed: $out" }
    $out
}

if (-not (Test-Path $state)) {
    throw "No saved plan in ${state}: run Enter-BenchPowerPlan.ps1 first, or activate your usual plan by hand."
}
$saved = (Get-Content $state -Raw).Trim()
Invoke-PowerCfg /setactive $saved | Out-Null
if ((Invoke-PowerCfg /getactivescheme) -notmatch $saved) { throw "The saved plan $saved could not be activated." }
if ((Invoke-PowerCfg /list) -match $bench) { Invoke-PowerCfg /delete $bench | Out-Null }
Remove-Item $state
Write-Host "Power plan restored: $saved"
