#Requires -Version 7.3
<#
.SYNOPSIS
    Ejecuta una tarea de un commit en la máquina Linux y trae sus resultados (docs/guides/linux-runner.md).
.EXAMPLE
    pwsh scripts/runner/Invoke-Linux.ps1 check
.EXAMPLE
    pwsh scripts/runner/Invoke-Linux.ps1 run 'cargo run --release -p krummi --example tree_timing' -Commit main
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)] [ValidateSet('check', 'build', 'bench', 'run')] [string] $Task,
    [string] $Commit = 'HEAD',
    # Arguments of the task, in one string that the Linux shell splits and unquotes, as in `ssh host '...'`.
    [Parameter(Position = 1)] [string] $Arguments = '',
    [string] $Repository = (Split-Path (Split-Path $PSScriptRoot))
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandArgumentPassing = 'Standard'

$configPath = $env:KRUMMI_RUNNER_CONFIG ? $env:KRUMMI_RUNNER_CONFIG : (Join-Path $HOME '.krummi-runner.json')
$example = '{ "destination": "usuario@host", "identityFile": "~/.ssh/krummi_runner" }'
if (-not (Test-Path -LiteralPath $configPath)) { throw "falta $configPath; créalo así: $example" }
$config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
$destination = $config.destination
if (-not $destination) { throw "$configPath no tiene destination; ejemplo: $example" }

$sshOptions = @('-o', 'BatchMode=yes')
if ($config.identityFile) {
    $identity = ($config.identityFile -replace '^~', $HOME).Replace('\', '/')
    $sshOptions += @('-i', $identity)
    $env:GIT_SSH_COMMAND = "ssh -o BatchMode=yes -i `"$identity`""
}
else { $env:GIT_SSH_COMMAND = 'ssh -o BatchMode=yes' }
$env:GIT_SSH_VARIANT = 'ssh'

function Assert-Success([string] $Step) { if ($LASTEXITCODE -ne 0) { throw "$Step falló (código $LASTEXITCODE)" } }

$sha = git -C $Repository rev-parse --verify "$Commit^{commit}"
Assert-Success "git rev-parse $Commit"
if ($Commit -eq 'HEAD' -and (git -C $Repository status --porcelain)) {
    Write-Warning 'hay cambios sin confirmar: no se ejecutan, solo el commit.'
}

ssh @sshOptions $destination 'mkdir -p krummi-runner'
Assert-Success "ssh $destination"
scp -q @sshOptions (Join-Path $PSScriptRoot 'runner.sh') "${destination}:krummi-runner/runner.sh"
Assert-Success 'scp runner.sh'
ssh @sshOptions $destination 'bash krummi-runner/runner.sh init'
Assert-Success 'runner.sh init'
git -C $Repository push -q --force "${destination}:krummi-runner/repo.git" "${sha}:refs/runner/$sha"
Assert-Success 'git push'

$resultsDir = $null
ssh @sshOptions $destination "bash krummi-runner/runner.sh $Task $sha -- $Arguments" | ForEach-Object {
    if ($_ -match '^RESULTS_DIR=(.+)$') { $resultsDir = $Matches[1] } else { Write-Host $_ }
}
$status = $LASTEXITCODE

if ($resultsDir) {
    $local = Join-Path $Repository 'work' 'runner'
    New-Item -ItemType Directory -Force $local | Out-Null
    scp -q -r @sshOptions "${destination}:$resultsDir" $local
    Assert-Success 'scp de los resultados'
    Write-Host "Resultados: $(Join-Path $local (Split-Path $resultsDir -Leaf))"
}
exit $status
