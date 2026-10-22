<#
.SYNOPSIS
    Discards a rehearsal in one step: every copy whose base is the rehearsal base, their branches, their exp/
    tags, their Engram projects, the Claude Code memory of their folders and the base itself. main does not change.

.DESCRIPTION
    Run from the MAIN copy (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md, "Ensayar y descartar el ensayo").

    The copies are every name in worktreebase.<Base>.created (each copy ever created on the base, so copies
    already removed or discarded are cleaned too) plus any live branch whose recorded base is <Base>. A name whose
    live branch now records another base is skipped and reported. For each copy, as in Remove-Worktree.ps1: the
    folder and registration (never --force), the branch, the tag exp/<n>, the Engram project muninndb-<n> and the memory folder Claude Code
    keeps for the copy's path (never the session history). Then the base branch and its worktreebase.<Base>
    config section.

    Refused, removing nothing: from a copy; for main or any branch not created with New-RehearsalBase.ps1; while
    the base is checked out in a copy (the main one included); while any of its copies has uncommitted changes
    (each one is listed). A file in use stops it with its name; running it again completes whatever is left, and
    once nothing is left it says so and changes nothing.

    Exit codes: 0 discarded (or nothing left); 1 refused or stopped.

.PARAMETER Base
    The rehearsal base to discard, e.g. ensayo.

.PARAMETER Root
    Folder that holds the copies. Default C:\wt.

.PARAMETER ClaudeProjectsDir
    Claude Code's projects folder. Default %USERPROFILE%\.claude\projects (or %CLAUDE_CONFIG_DIR%\projects).
    Tests use another one.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Remove-Rehearsal.ps1 -Base ensayo
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Base,
    [string]$Root = 'C:\wt',
    [string]$ClaudeProjectsDir
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WorktreeCommon.psm1') -Force
Initialize-WorktreeConsole

# ---------------------------------------------------------------------------------------------------------------
# A. Checks: nothing is removed
# ---------------------------------------------------------------------------------------------------------------

$context = Get-RepositoryContext -Path $PSScriptRoot
$mainRoot = $context.MainRoot
$worktrees = @(Get-WorktreeList -MainRoot $mainRoot)
$Root = Resolve-RootPath -Root $Root
if (-not $ClaudeProjectsDir) { $ClaudeProjectsDir = Get-DefaultClaudeProjectsDir }

$session = Get-SessionCopy -Context $context -Worktrees $worktrees
if ($session) { Exit-Worktree (Get-WorktreeMessage CopyContext $session) }
if ($Base -ceq 'main') { Exit-Worktree (Get-WorktreeMessage NotRehearsal 'main') }

$marked = (Get-GitLine -Path $mainRoot -Arguments @('config', '--get', "worktreebase.$Base.rehearsal")) -eq 'true'
$baseExists = Test-GitRef -Path $mainRoot -Ref "refs/heads/$Base"
$names = @(@(Get-CreatedNames -MainRoot $mainRoot -Base $Base) + @(Get-NamesOnBase -MainRoot $mainRoot -Base $Base) |
    Where-Object { $_ } | Select-Object -Unique)

if (-not $marked) {
    if ($baseExists -or $names.Count -gt 0) { Exit-Worktree (Get-WorktreeMessage NotRehearsal $Base) }
    Exit-Worktree (Get-WorktreeMessage NothingLeft $Base) -Code 0
}

$holder = Get-BranchHolder -Branch $Base -Worktrees $worktrees
if ($holder) { Exit-Worktree (Get-WorktreeMessage BaseCheckedOut $Base, $holder) }

$engram = Get-CommandPath -Name 'engram'
if (-not $engram) { Exit-Worktree (Get-WorktreeMessage ToolMissingRm 'engram') }
try { $projects = @(Get-EngramProjects -Engram $engram) }
catch { Exit-Worktree (Get-WorktreeMessage EngramFailedRm $_.Exception.Message.TrimEnd('.')) }

$plan = New-Object System.Collections.Generic.List[object]
$refusals = New-Object System.Collections.Generic.List[string]
foreach ($name in $names) {
    if (-not (Test-ChangeName $name)) { continue }
    $branch = "feat/$name"
    $branchExists = Test-GitRef -Path $mainRoot -Ref "refs/heads/$branch"
    $recorded = Get-RecordedBase -MainRoot $mainRoot -Name $name
    if ($branchExists -and $recorded -and $recorded -cne $Base) {
        Write-Host "Se omite '$name': su rama $branch tiene ahora la base '$recorded'." -ForegroundColor Yellow
        continue
    }
    $folder = Get-CopyFolder -Root $Root -Name $name
    $registered = @($worktrees | Where-Object { -not $_.IsMain -and ((Test-SamePath $_.Path $folder) -or $_.Branch -eq "refs/heads/$branch") }) |
        Select-Object -First 1
    if ($registered) { $folder = $registered.Path }
    $folderExists = Test-Path -LiteralPath $folder
    if (-not $registered -and $folderExists -and
        -not (Test-OwnLeftover -Folder $folder -GitCommonDir $context.GitCommonDir -RecordedBase $recorded)) {
        Exit-Worktree (Get-WorktreeMessage ForeignFolder $folder)
    }
    if ($registered -and $folderExists -and (Test-Path -LiteralPath (Join-Path $folder '.git'))) {
        try { $lost = @(Get-StatusPaths -Path $folder) }
        catch { $lost = @("lo que git no pudo comprobar ($($_.Exception.Message.TrimEnd('.')))") }
        if ($lost.Count -gt 0) { $refusals.Add((Get-WorktreeMessage WouldLose $name, ($lost -join ', '))) }
    }
    $plan.Add([pscustomobject]@{
        Name         = $name
        Branch       = $branch
        BranchExists = $branchExists
        Folder       = $folder
        Registered   = [bool]$registered
        RecordedBase = $recorded
        FolderExists = $folderExists
        Project      = Get-EngramProjectName -Name $name
        Memory       = @(Get-ClaudeMemoryFolders -ClaudeProjectsDir $ClaudeProjectsDir -CopyFolder $folder)
    })
}

if ($refusals.Count -gt 0) {
    for ($i = 0; $i -lt $refusals.Count - 1; $i++) { Write-Host $refusals[$i] -ForegroundColor Red }
    Exit-Worktree $refusals[$refusals.Count - 1]
}

# ---------------------------------------------------------------------------------------------------------------
# B. Discard: nothing is forced; a file in use stops it and a new run completes it
# ---------------------------------------------------------------------------------------------------------------

$done = New-Object System.Collections.Generic.List[string]

function Stop-Discard {
    param([string]$Step, [string]$Cause)
    $list = if ($done.Count -gt 0) { $done -join ', ' } else { 'nada' }
    Exit-Worktree (Get-WorktreeMessage RemoveFailed $Step, $Cause.TrimEnd('.'), $list)
}

foreach ($item in $plan) {
    if ($item.Registered -or $item.FolderExists) {
        Write-Host "[worktree] eliminar la copia '$($item.Folder)'..." -ForegroundColor Cyan
        $failure = Remove-CopyFolder -MainRoot $mainRoot -GitCommonDir $context.GitCommonDir -Folder $item.Folder -Registered $item.Registered `
            -RecordedBase $item.RecordedBase
        if ($failure) {
            if ($failure.InUse) { Exit-Worktree (Get-WorktreeMessage InUse $failure.InUse) }
            Stop-Discard $failure.Step $failure.Cause
        }
        $done.Add("la copia '$($item.Folder)'")
    }
    if ($item.BranchExists) {
        $result = Invoke-Git -Path $mainRoot -Arguments @('branch', '-D', $item.Branch)
        if ($result.ExitCode -ne 0) { Stop-Discard 'eliminar la rama' "git branch -D $($item.Branch): $(Get-NativeText $result)" }
        $done.Add("la rama '$($item.Branch)'")
    }
    $tag = "exp/$($item.Name)"
    if (Test-GitRef -Path $mainRoot -Ref "refs/tags/$tag") {
        $result = Invoke-Git -Path $mainRoot -Arguments @('tag', '-d', $tag)
        if ($result.ExitCode -ne 0) { Stop-Discard 'eliminar la etiqueta' "git tag -d ${tag}: $(Get-NativeText $result)" }
        $done.Add("la etiqueta '$tag'")
    }
    if ($projects -contains $item.Project) {
        try { $null = Remove-EngramProject -Engram $engram -Project $item.Project }
        catch { Stop-Discard 'eliminar el proyecto Engram' $_.Exception.Message }
        $done.Add("el proyecto Engram '$($item.Project)'")
    }
    foreach ($folderOfMemory in $item.Memory) {
        $blocked = Remove-FolderTree -Path $folderOfMemory
        if ($blocked) { Exit-Worktree (Get-WorktreeMessage InUse $blocked) }
        $done.Add("la memoria de Claude Code '$folderOfMemory'")
    }
}

if ($baseExists) {
    $result = Invoke-Git -Path $mainRoot -Arguments @('branch', '-D', $Base)
    if ($result.ExitCode -ne 0) { Stop-Discard 'eliminar la base' "git branch -D ${Base}: $(Get-NativeText $result)" }
}
$null = Invoke-Git -Path $mainRoot -Arguments @('config', '--remove-section', "worktreebase.$Base")
# The section may already be gone (exit code 128), so success is judged by what is left, not by the exit code.
$left = Invoke-Git -Path $mainRoot -Arguments @('config', '--get-regexp', "^worktreebase\.$([regex]::Escape($Base))\.")
if ($left.ExitCode -eq 0 -and @($left.Out | Where-Object { $_ }).Count -gt 0) {
    Stop-Discard 'limpiar la configuración del ensayo' "git config --remove-section worktreebase.${Base}: no se pudo quitar"
}

Write-Host ''
Write-Host ("Ensayo '$Base' descartado: $($plan.Count) copia(s), con sus ramas, etiquetas exp/, proyectos Engram y memoria de " +
            "Claude Code, y la base '$Base'. 'main' no ha cambiado.") -ForegroundColor Green
exit 0
