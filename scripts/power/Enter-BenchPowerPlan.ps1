<#
.SYNOPSIS
    Switches Windows to a temporary power plan for benchmark runs: steady processor frequency, no parked cores and no
    sleep. Restore the previous plan afterwards with Exit-BenchPowerPlan.ps1.

.DESCRIPTION
    Copies the Balanced plan into a plan named "Krummi bench" and changes only its plugged-in (AC) values:
    - processor boost (turbo) disabled, unless -Boost is given, so the frequency does not follow the temperature;
    - minimum and maximum processor state at 100 % and no parked cores, for both core classes (P and E cores);
    - never sleep, hibernate or turn the disk off.
    The active plan is saved in %LOCALAPPDATA%\krummi-bench-power.txt before the switch. Running the script again while
    the bench plan is active updates its values and keeps the saved plan. Battery (DC) values are not changed, so run
    the benchmark plugged in.

.PARAMETER Boost
    Keep processor boost enabled, in its aggressive mode: faster runs, but the frequency then depends on the
    temperature of the machine.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/power/Enter-BenchPowerPlan.ps1
#>
[CmdletBinding()]
param(
    [switch]$Boost
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$bench = '6c0f3a52-1d7e-4b8a-9f21-4b7d2e9c5a10'
$state = Join-Path $env:LOCALAPPDATA 'krummi-bench-power.txt'

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

$active = Get-ActiveScheme
if ($active -eq $bench) {
    if (-not (Test-Path $state)) {
        throw "The bench plan is active but $state is missing: activate your usual plan by hand."
    }
    Write-Host 'The bench plan is already active; the saved plan is kept.'
} else {
    Set-Content -Path $state -Value $active -Encoding ascii
    if ((Invoke-PowerCfg /list) -match $bench) { Invoke-PowerCfg /delete $bench | Out-Null }
    Invoke-PowerCfg /duplicatescheme SCHEME_BALANCED $bench | Out-Null
    Invoke-PowerCfg /changename $bench 'Krummi bench' 'Temporary plan for benchmark runs' | Out-Null
}

$cpu = '54533251-82be-4824-96c1-47b60b740d00'
$values = @(
    @($cpu, 'be337238-0d82-4146-a960-4f3749d470c7', $(if ($Boost) { 2 } else { 0 })), # boost mode
    @($cpu, '893dee8e-2bef-41e0-89c6-b55d0929964c', 100), # minimum processor state
    @($cpu, '893dee8e-2bef-41e0-89c6-b55d0929964d', 100), # minimum processor state, class 1
    @($cpu, 'bc5038f7-23e0-4960-96da-33abaf5935ec', 100), # maximum processor state
    @($cpu, 'bc5038f7-23e0-4960-96da-33abaf5935ed', 100), # maximum processor state, class 1
    @($cpu, '0cc5b647-c1df-4637-891a-dec35c318583', 100), # core parking minimum cores
    @($cpu, '0cc5b647-c1df-4637-891a-dec35c318584', 100), # core parking minimum cores, class 1
    @('SUB_SLEEP', 'STANDBYIDLE', 0),
    @('SUB_SLEEP', 'HIBERNATEIDLE', 0),
    @('SUB_DISK', 'DISKIDLE', 0)
)
foreach ($v in $values) { Invoke-PowerCfg /setacvalueindex $bench $v[0] $v[1] $v[2] | Out-Null }
Invoke-PowerCfg /setactive $bench | Out-Null
if ((Get-ActiveScheme) -ne $bench) { throw 'The bench plan could not be activated.' }

Add-Type -AssemblyName System.Windows.Forms
if ([System.Windows.Forms.SystemInformation]::PowerStatus.PowerLineStatus -ne 'Online') {
    Write-Warning 'The machine runs on battery: plug it in, the bench plan only changes the plugged-in values.'
}
$mode = if ($Boost) { 'boost on' } else { 'boost off' }
Write-Host "Bench power plan active ($mode). Saved plan: $(Get-Content $state). Restore it with Exit-BenchPowerPlan.ps1."
