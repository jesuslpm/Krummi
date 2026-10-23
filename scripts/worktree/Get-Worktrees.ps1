<#
.SYNOPSIS
    Lists the working copies (worktrees): name, branch, base, whether the branch is already merged into its base,
    folder and Engram project. Read only.

.DESCRIPTION
    Works from the main copy or from any copy (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md, "Listar copias").
    "Fusionada" is "sí" when the branch has no commit that its base lacks; the base is the one recorded when the
    copy was created (branch.feat/<n>.worktreebase). Without copies it prints
    "No hay worktrees (solo la copia principal).".

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Get-Worktrees.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WorktreeCommon.psm1') -Force
Initialize-WorktreeConsole

$context = Get-RepositoryContext -Path $PSScriptRoot
$mainRoot = $context.MainRoot
$copies = @(Get-WorktreeList -MainRoot $mainRoot | Where-Object { -not $_.IsMain })

if ($copies.Count -eq 0) {
    Write-Output (Get-WorktreeMessage NoWorktrees)
    exit 0
}

$rows = foreach ($copy in $copies) {
    $branch = if ($copy.Branch) { $copy.Branch -replace '^refs/heads/', '' } else { '(sin rama)' }
    $leaf = Split-Path -Leaf $copy.Path
    $name = if ($branch -match '^feat/(.+)$') { $Matches[1] } elseif ($leaf -like 'krummi-*') { $leaf.Substring('krummi-'.Length) } else { $leaf }

    $base = Get-RecordedBase -MainRoot $mainRoot -Name $name
    $merged = '?'
    if (-not $base) { $base = '(sin registrar)' }
    elseif ($copy.Head -and (Test-GitRef -Path $mainRoot -Ref "refs/heads/$base")) {
        $ahead = Get-GitLine -Path $mainRoot -Arguments @('rev-list', '--count', "refs/heads/$base..$($copy.Head)")
        $merged = if ($ahead -eq '0') { 'sí' } else { 'no' }
    }

    $project = Get-EngramProjectName -Name $name
    $config = Join-Path $copy.Path '.engram\config.json'
    if (Test-Path -LiteralPath $config -PathType Leaf) {
        try {
            $declared = (Get-Content -LiteralPath $config -Raw -Encoding UTF8 | ConvertFrom-Json).project_name
            if ($declared) { $project = $declared }
        }
        catch { }
    }

    [pscustomobject]@{
        'Nombre'          = $name
        'Rama'            = $branch
        'Base'            = $base
        'Fusionada'       = $merged
        'Carpeta'         = $copy.Path
        'Proyecto Engram' = $project
    }
}

Write-Output (@($rows) | Format-Table -AutoSize | Out-String -Width 4096).Trim()
exit 0
