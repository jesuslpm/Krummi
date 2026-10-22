<#
.SYNOPSIS
    Merges this copy's branch into its base: the base advances to the branch, without a merge commit and without
    leaving the copy. Nothing is ever pushed.

.DESCRIPTION
    Run from the copy of the change (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md, "Fusionar una copia en su base"),
    after bringing the base's new commits into the branch and running the full checks again. The base is the one
    recorded when the copy was created (branch.feat/<n>.worktreebase).

    Checked before anything changes: the session is in a copy (not the main one) on its feat/<n> branch; the copy
    has no uncommitted changes; the branch contains the whole base (exit 3 otherwise: bring the base in, verify
    again and repeat); the branch brings something new; and, when the base is checked out in a copy (usually the
    main one), that copy has no uncommitted changes in tracked files.

    Then the base advances: in the copy that has it checked out with `git merge --ff-only feat/<n>`, so its files
    follow; when no copy has it checked out, with `git update-ref` as a compare-and-swap. The copy and its branch
    are kept.

    Exit codes: 0 merged; 3 the base has commits the branch lacks (go back to step 1 once); 1 anything else.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Merge-Worktree.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WorktreeCommon.psm1') -Force
Initialize-WorktreeConsole

# ---------------------------------------------------------------------------------------------------------------
# A. Checks: nothing changes
# ---------------------------------------------------------------------------------------------------------------

$context = Get-RepositoryContext -Path $PSScriptRoot
if ($context.IsMain) { Exit-Worktree (Get-WorktreeMessage MergeFromMain) }
$copy = $context.CopyRoot
$mainRoot = $context.MainRoot

$branch = Get-GitLine -Path $copy -Arguments @('symbolic-ref', '--short', '-q', 'HEAD')
if (-not $branch -or -not $branch.StartsWith('feat/') -or $branch.Length -le 5) {
    $shown = if ($branch) { $branch } else { 'HEAD' }
    Exit-Worktree (Get-WorktreeMessage NotOnFeature $shown)
}
$name = $branch.Substring(5)

$base = Get-RecordedBase -MainRoot $mainRoot -Name $name
if (-not $base) { Exit-Worktree (Get-WorktreeMessage NoRecordedBase $name) }
if (-not (Test-GitRef -Path $mainRoot -Ref "refs/heads/$base")) { Exit-Worktree (Get-WorktreeMessage MergeBaseGone $base) }

$pending = @(Get-StatusPaths -Path $copy)
if ($pending.Count -gt 0) { Exit-Worktree (Get-WorktreeMessage CopyDirty ($pending -join ', ')) }

$headSha = Get-GitLine -Path $copy -Arguments @('rev-parse', 'HEAD')
$baseSha = Get-GitLine -Path $copy -Arguments @('rev-parse', "refs/heads/$base")

if ((Invoke-Git -Path $copy -Arguments @('merge-base', '--is-ancestor', $baseSha, $headSha)).ExitCode -ne 0) {
    $missing = Get-GitLine -Path $copy -Arguments @('rev-list', '--count', "$headSha..$baseSha")
    Exit-Worktree (Get-WorktreeMessage BaseAhead $base, $missing) -Code 3
}

$ahead = Get-GitLine -Path $copy -Arguments @('rev-list', '--count', "$baseSha..$headSha")
if ($ahead -eq '0') { Exit-Worktree (Get-WorktreeMessage NothingToMerge $name, $base) }

$worktrees = @(Get-WorktreeList -MainRoot $mainRoot)
$holder = Get-BranchHolder -Branch $base -Worktrees $worktrees
if ($holder) {
    $holderPending = @(Get-StatusPaths -Path $holder -TrackedOnly)
    if ($holderPending.Count -gt 0) { Exit-Worktree (Get-WorktreeMessage BaseOpenDirty $base, $holder) }
}

# ---------------------------------------------------------------------------------------------------------------
# B. The base advances (fast-forward only, never a push)
# ---------------------------------------------------------------------------------------------------------------

if ($holder) {
    $step = "avanzar '$base' en '$holder'"
    $result = Invoke-Git -Path $holder -Arguments @('merge', '--ff-only', '-q', $branch)
    if ($result.ExitCode -ne 0) {
        # The base may have moved in its holder since the checks above: then it is exit 3, as below.
        $now = Get-GitLine -Path $mainRoot -Arguments @('rev-parse', "refs/heads/$base")
        if ($now -and (Invoke-Git -Path $copy -Arguments @('merge-base', '--is-ancestor', $now, $headSha)).ExitCode -ne 0) {
            $missing = Get-GitLine -Path $copy -Arguments @('rev-list', '--count', "$headSha..$now")
            Exit-Worktree (Get-WorktreeMessage BaseAhead $base, $missing) -Code 3
        }
        Exit-Worktree (Get-WorktreeMessage StepUnchanged $step, "git merge --ff-only: $(Get-NativeText $result)")
    }
}
else {
    $step = "avanzar '$base'"
    $result = Invoke-Git -Path $mainRoot -Arguments @('update-ref', '-m', "worktree: merge $branch", "refs/heads/$base", $headSha, $baseSha)
    if ($result.ExitCode -ne 0) {
        # Compare-and-swap lost: the base moved meanwhile. If the branch no longer contains it, it is exit 3.
        $now = Get-GitLine -Path $mainRoot -Arguments @('rev-parse', "refs/heads/$base")
        if ($now -and (Invoke-Git -Path $copy -Arguments @('merge-base', '--is-ancestor', $now, $headSha)).ExitCode -ne 0) {
            $missing = Get-GitLine -Path $copy -Arguments @('rev-list', '--count', "$headSha..$now")
            Exit-Worktree (Get-WorktreeMessage BaseAhead $base, $missing) -Code 3
        }
        Exit-Worktree (Get-WorktreeMessage StepUnchanged $step, "git update-ref: $(Get-NativeText $result)")
    }
}

$final = Get-GitLine -Path $mainRoot -Arguments @('rev-parse', "refs/heads/$base")
if ($final -ne $headSha) {
    Exit-Worktree (Get-WorktreeMessage StepUnchanged $step, "'$base' quedó en $final y no en $headSha")
}

$short = Get-GitLine -Path $copy -Arguments @('rev-parse', '--short', 'HEAD')
Write-Host (Get-WorktreeMessage Merged $base, $name, $short) -ForegroundColor Green
if ($holder) { Write-Host "La copia '$holder' tiene '$base' abierta y se ha actualizado." }
exit 0
