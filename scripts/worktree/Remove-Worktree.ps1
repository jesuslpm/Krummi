<#
.SYNOPSIS
    Removes a working copy: its folder, its registration and its branch. With -Discard it first tags the branch
    as exp/<n> and also deletes the copy's Engram project and Claude Code memory. Never forces anything and never
    stops a process.

.DESCRIPTION
    Run from the MAIN copy (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md, "Eliminar una copia").

    Without -Discard the branch must have no commit that its base lacks (the base recorded at creation); the
    copy's Engram project and Claude Code memory are kept. With -Discard an unmerged branch is accepted: the tag
    exp/<n> is created at its tip (refused when exp/<n> already exists elsewhere) and then the Engram project
    muninndb-<n> and the memory folder Claude Code keeps for the copy's path are deleted too (never the
    session history).

    Always refused, touching nothing: from a copy; from inside the copy being removed; with uncommitted changes
    (modified or untracked, not ignored), also when discarding.

    Order: `git worktree remove` (never --force), what git could not delete, `git worktree
    prune`, `git branch -D feat/<n>`, then, when discarding, the Engram project and the Claude Code memory.

    A file in use stops the removal with its name; close what uses it and run the same command again: it
    completes whatever is left.

    Exit codes: 0 removed, or nothing of <Name> was left; 1 refused or stopped.

.PARAMETER Name
    Change name of the copy: folder <Root>\muninndb-<Name>, branch feat/<Name>.

.PARAMETER Discard
    Remove an unmerged copy, keeping its history in the tag exp/<Name>, and delete its Engram project and Claude
    Code memory.

.PARAMETER Root
    Folder that holds the copies. Default C:\wt.

.PARAMETER ClaudeProjectsDir
    Claude Code's projects folder. Default %USERPROFILE%\.claude\projects (or %CLAUDE_CONFIG_DIR%\projects).
    Tests use another one.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Remove-Worktree.ps1 -Name startswith-odd

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Remove-Worktree.ps1 -Name startswith-vibe -Discard
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [switch]$Discard,
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

$validName = Test-ChangeName $Name
$branch = "feat/$Name"
$folder = $null
$registered = $null
if ($validName) {
    $folder = Get-CopyFolder -Root $Root -Name $Name
    $registered = @($worktrees | Where-Object { -not $_.IsMain -and ((Test-SamePath $_.Path $folder) -or $_.Branch -eq "refs/heads/$branch") }) |
        Select-Object -First 1
    if ($registered) { $folder = $registered.Path }
    $here = (Get-Location).ProviderPath
    if ((Test-PathWithin -Path $here -Folder $folder) -or (Test-PathWithin -Path $context.CopyRoot -Folder $folder)) {
        Exit-Worktree (Get-WorktreeMessage SessionInside $folder)
    }
}

$session = Get-SessionCopy -Context $context -Worktrees $worktrees
if ($session) { Exit-Worktree (Get-WorktreeMessage CopyContext $session) }
if (-not $validName) { Exit-Worktree (Get-WorktreeMessage InvalidNameRm $Name) }

$branchExists = Test-GitRef -Path $mainRoot -Ref "refs/heads/$branch"
$folderExists = Test-Path -LiteralPath $folder
$project = Get-EngramProjectName -Name $Name
$engram = $null
$projectExists = $false
$memory = @()
if ($Discard) {
    $engram = Get-CommandPath -Name 'engram'
    if (-not $engram) { Exit-Worktree (Get-WorktreeMessage ToolMissingRm 'engram') }
    try { $projectExists = @(Get-EngramProjects -Engram $engram) -contains $project }
    catch { Exit-Worktree (Get-WorktreeMessage EngramFailedRm $_.Exception.Message.TrimEnd('.')) }
    $memory = @(Get-ClaudeMemoryFolders -ClaudeProjectsDir $ClaudeProjectsDir -CopyFolder $folder)
}

if (-not $registered -and -not $folderExists -and -not $branchExists -and -not $projectExists -and $memory.Count -eq 0) {
    # Already removed (or never existed): repeating a removal is harmless.
    Exit-Worktree (Get-WorktreeMessage NothingToRemove $Name) -Code 0
}
$recordedBase = Get-RecordedBase -MainRoot $mainRoot -Name $Name
if (-not $registered -and $folderExists -and
    -not (Test-OwnLeftover -Folder $folder -GitCommonDir $context.GitCommonDir -RecordedBase $recordedBase)) {
    Exit-Worktree (Get-WorktreeMessage ForeignFolder $folder)
}

if ($branchExists -and -not $Discard) {
    $base = Get-RecordedBase -MainRoot $mainRoot -Name $Name
    if (-not $base -or -not (Test-GitRef -Path $mainRoot -Ref "refs/heads/$base")) { $base = 'main' }
    $count = Get-GitLine -Path $mainRoot -Arguments @('rev-list', '--count', "refs/heads/$base..refs/heads/$branch")
    if ($null -eq $count) { $count = Get-GitLine -Path $mainRoot -Arguments @('rev-list', '--count', "refs/heads/$branch") }
    if ([int]$count -gt 0) { Exit-Worktree (Get-WorktreeMessage Unmerged $Name, $count, $base) }
}

if ($registered -and $folderExists -and (Test-Path -LiteralPath (Join-Path $folder '.git'))) {
    try { $lost = @(Get-StatusPaths -Path $folder) }
    catch { $lost = @("lo que git no pudo comprobar ($($_.Exception.Message.TrimEnd('.')))") }
    if ($lost.Count -gt 0) { Exit-Worktree (Get-WorktreeMessage WouldLose $Name, ($lost -join ', ')) }
}

$tag = "exp/$Name"
$tagCommit = Get-GitLine -Path $mainRoot -Arguments @('rev-parse', '-q', '--verify', "refs/tags/$tag^{commit}")
$tipCommit = if ($branchExists) { Get-GitLine -Path $mainRoot -Arguments @('rev-parse', "refs/heads/$branch") } else { $null }
if ($Discard -and $tagCommit -and $branchExists -and $tagCommit -ne $tipCommit) {
    Exit-Worktree (Get-WorktreeMessage TagExists $Name)
}

# ---------------------------------------------------------------------------------------------------------------
# B. Removal: nothing is forced; a file in use stops it and a new run completes it
# ---------------------------------------------------------------------------------------------------------------

$done = New-Object System.Collections.Generic.List[string]

function Stop-Removal {
    param([string]$Step, [string]$Cause)
    $list = if ($done.Count -gt 0) { $done -join ', ' } else { 'nada' }
    Exit-Worktree (Get-WorktreeMessage RemoveFailed $Step, $Cause.TrimEnd('.'), $list)
}

if ($Discard -and $branchExists -and -not $tagCommit) {
    Write-Host "[worktree] crear la etiqueta $tag..." -ForegroundColor Cyan
    $result = Invoke-Git -Path $mainRoot -Arguments @('tag', $tag, "refs/heads/$branch")
    if ($result.ExitCode -ne 0) { Stop-Removal "crear la etiqueta $tag" "git tag: $(Get-NativeText $result)" }
}

if ($registered -or $folderExists) {
    Write-Host "[worktree] eliminar la copia '$folder'..." -ForegroundColor Cyan
    $failure = Remove-CopyFolder -MainRoot $mainRoot -GitCommonDir $context.GitCommonDir -Folder $folder -Registered ([bool]$registered) `
        -RecordedBase $recordedBase
    if ($failure) {
        if ($failure.InUse) { Exit-Worktree (Get-WorktreeMessage InUse $failure.InUse) }
        Stop-Removal $failure.Step $failure.Cause
    }
    $done.Add("la copia '$folder'")
}

if ($branchExists) {
    Write-Host "[worktree] eliminar la rama $branch..." -ForegroundColor Cyan
    $result = Invoke-Git -Path $mainRoot -Arguments @('branch', '-D', $branch)
    if ($result.ExitCode -ne 0) { Stop-Removal 'eliminar la rama' "git branch -D: $(Get-NativeText $result)" }
    $done.Add("la rama '$branch'")
}

if ($Discard) {
    if ($projectExists) {
        Write-Host "[worktree] eliminar el proyecto Engram $project..." -ForegroundColor Cyan
        try { $null = Remove-EngramProject -Engram $engram -Project $project }
        catch { Stop-Removal 'eliminar el proyecto Engram' $_.Exception.Message }
        $done.Add("el proyecto Engram '$project'")
    }
    foreach ($folderOfMemory in $memory) {
        Write-Host "[worktree] eliminar la memoria de Claude Code '$folderOfMemory'..." -ForegroundColor Cyan
        $blocked = Remove-FolderTree -Path $folderOfMemory
        if ($blocked) { Exit-Worktree (Get-WorktreeMessage InUse $blocked) }
    }
}

Write-Host ''
if ($Discard) {
    $kept = if (Test-GitRef -Path $mainRoot -Ref "refs/tags/$tag") { "su historia queda en la etiqueta $tag; " } else { '' }
    Write-Host "Descartado '$Name': ${kept}se han eliminado la copia, su rama, el proyecto Engram '$project' y la memoria de Claude Code de '$folder'." -ForegroundColor Green
}
else {
    Write-Host "Eliminado '$Name': se han eliminado la copia '$folder' y la rama '$branch'. Se conservan el proyecto Engram '$project' y la memoria de Claude Code." -ForegroundColor Green
}
exit 0
