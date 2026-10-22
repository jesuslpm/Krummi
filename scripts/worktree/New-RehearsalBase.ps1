<#
.SYNOPSIS
    Creates a rehearsal base: a throwaway branch made from main that plays the part of main during a rehearsal.

.DESCRIPTION
    Run from the MAIN copy (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md, "Ensayar y descartar el ensayo"). It creates
    the branch <Base> at the tip of main, without checking it out, and marks it as a rehearsal base
    (worktreebase.<Base>.rehearsal = true), which is what Remove-Rehearsal.ps1 requires. The copies of the
    rehearsal are then created with New-Worktree.ps1 -Base <Base>, and everything is discarded with
    Remove-Rehearsal.ps1 -Base <Base>.

    Refused, creating nothing: from a copy; a name that is not lowercase letters, digits and single hyphens; a
    base that already exists (main included).

    Exit codes: 0 created; 1 refused or failed.

.PARAMETER Base
    Name of the rehearsal base, e.g. ensayo.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/New-RehearsalBase.ps1 -Base ensayo
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Base
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WorktreeCommon.psm1') -Force
Initialize-WorktreeConsole

$context = Get-RepositoryContext -Path $PSScriptRoot
$mainRoot = $context.MainRoot
$worktrees = @(Get-WorktreeList -MainRoot $mainRoot)

$session = Get-SessionCopy -Context $context -Worktrees $worktrees
if ($session) { Exit-Worktree (Get-WorktreeMessage CopyContext $session) }
if (-not (Test-ChangeName $Base)) { Exit-Worktree (Get-WorktreeMessage InvalidName $Base) }
if (Test-GitRef -Path $mainRoot -Ref "refs/heads/$Base") { Exit-Worktree (Get-WorktreeMessage BaseExists $Base) }
if (-not (Test-GitRef -Path $mainRoot -Ref 'refs/heads/main')) { Exit-Worktree (Get-WorktreeMessage BaseMissing 'main') }

# The mark first: alone it is harmless, and a base never exists unmarked.
$result = Invoke-Git -Path $mainRoot -Arguments @('config', "worktreebase.$Base.rehearsal", 'true')
if ($result.ExitCode -ne 0) { Exit-Worktree (Get-WorktreeMessage StepNotCreated 'marcar la base de ensayo', "git config: $(Get-NativeText $result)") }
$result = Invoke-Git -Path $mainRoot -Arguments @('branch', '--no-track', $Base, 'refs/heads/main')
if ($result.ExitCode -ne 0) { Exit-Worktree (Get-WorktreeMessage StepNotCreated 'crear la base', "git branch: $(Get-NativeText $result)") }

$short = Get-GitLine -Path $mainRoot -Arguments @('rev-parse', '--short', "refs/heads/$Base")
Write-Host "Base de ensayo '$Base' creada desde 'main' ($short)." -ForegroundColor Green
Write-Host "Siguiente paso: crea cada copia del ensayo con la frase `"Crea un worktree para el cambio <n> sobre $Base`"."
exit 0
