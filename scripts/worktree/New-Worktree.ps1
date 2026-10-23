<#
.SYNOPSIS
    Creates a working copy (git worktree) for one change: its branch, its folder, the private files,
    its work folder and its own Engram project.

.DESCRIPTION
    Run from the MAIN copy (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md, "Crear una copia"). For the change <Name>:

      branch          feat/<Name>, from the base (default main); the base is recorded in the repository config
      folder          <Root>\krummi-<Name>
      private files   .claude\settings.local.json and .atl\skill-registry.md, when the main copy has them
      work folder     <folder>\work (ignored by git), for acceptance runs and generated datasets
      Engram project  krummi-<Name> (engram init in the copy)

    then checks that git sees the copy clean, opens it in a new editor window (code -n) and prints the folder,
    branch, base, Engram project and the next step.

    Phase A checks everything and creates nothing: the session is in the main copy, the name is valid and short
    enough for Windows paths, the base exists, and no branch, folder, registered copy or Engram project with that
    name exists. Phase B creates; if a step fails it says which one and what was already created, and never undoes
    its own steps: Remove-Worktree.ps1 -Name <Name> cleans up.

    Exit codes: 0 created; 1 refused or failed.

.PARAMETER Name
    Change name: lowercase letters and digits in groups separated by single hyphens (e.g. startswith-odd).

.PARAMETER Base
    Branch the copy starts from and later merges into. Default main; a rehearsal uses its rehearsal base (see
    New-RehearsalBase.ps1). It must exist: a base is never created implicitly.

.PARAMETER Root
    Folder that holds the copies. Default C:\wt. The maximum name length depends on it.

.PARAMETER NoOpen
    Do not open the editor (tests use it).

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/New-Worktree.ps1 -Name startswith-odd

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/New-Worktree.ps1 -Name startswith-odd -Base ensayo
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$Base = 'main',
    [string]$Root = 'C:\wt',
    [switch]$NoOpen
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WorktreeCommon.psm1') -Force
Initialize-WorktreeConsole

# ---------------------------------------------------------------------------------------------------------------
# A. Checks: nothing is created
# ---------------------------------------------------------------------------------------------------------------

$context = Get-RepositoryContext -Path $PSScriptRoot
$mainRoot = $context.MainRoot
$worktrees = @(Get-WorktreeList -MainRoot $mainRoot)

$session = Get-SessionCopy -Context $context -Worktrees $worktrees
if ($session) { Exit-Worktree (Get-WorktreeMessage CopyContext $session) }

if (-not (Test-ChangeName $Name)) { Exit-Worktree (Get-WorktreeMessage InvalidName $Name) }

if (-not (Test-GitRef -Path $mainRoot -Ref "refs/heads/$Base")) { Exit-Worktree (Get-WorktreeMessage BaseMissing $Base) }

$Root = Resolve-RootPath -Root $Root
$max = Get-MaxNameLength -MainRoot $mainRoot -Root $Root -Base $Base
if ($Name.Length -gt $max) { Exit-Worktree (Get-WorktreeMessage NameTooLong $Name, $Name.Length, $max) }

$branch = "feat/$Name"
$folder = Get-CopyFolder -Root $Root -Name $Name
$project = Get-EngramProjectName -Name $Name

if (Test-GitRef -Path $mainRoot -Ref "refs/heads/$branch") { Exit-Worktree (Get-WorktreeMessage AlreadyExists 'la rama', $branch) }
if (Test-Path -LiteralPath $folder) { Exit-Worktree (Get-WorktreeMessage AlreadyExists 'la carpeta', $folder) }
if (@($worktrees | Where-Object { Test-SamePath $_.Path $folder }).Count -gt 0) {
    Exit-Worktree (Get-WorktreeMessage AlreadyExists 'la copia registrada', $folder)
}

$engram = Get-CommandPath -Name 'engram'
if (-not $engram) { Exit-Worktree (Get-WorktreeMessage ToolMissing 'engram') }
try { $projects = @(Get-EngramProjects -Engram $engram) }
catch { Exit-Worktree (Get-WorktreeMessage EngramFailed $_.Exception.Message.TrimEnd('.')) }
if ($projects -contains $project) { Exit-Worktree (Get-WorktreeMessage AlreadyExists 'el proyecto Engram', $project) }

$code = $null
if (-not $NoOpen) {
    $code = Get-CommandPath -Name 'code'
    if (-not $code) { Exit-Worktree (Get-WorktreeMessage ToolMissing 'code') }
}

# ---------------------------------------------------------------------------------------------------------------
# B. Creation: a failing step says what exists; nothing is undone
# ---------------------------------------------------------------------------------------------------------------

function Get-CreatedList {
    <# What exists now of this copy, in creation order, as the PRD's "Ya creado: <lista>". #>
    $items = New-Object System.Collections.Generic.List[string]
    if (Test-GitRef -Path $mainRoot -Ref "refs/heads/$branch") { $items.Add("la rama '$branch'") }
    if (Test-Path -LiteralPath $folder) { $items.Add("la copia '$folder'") }
    if (Get-RecordedBase -MainRoot $mainRoot -Name $Name) { $items.Add('la base registrada') }
    if (@(Get-PrivateFiles | Where-Object { Test-Path -LiteralPath (Join-Path $folder $_) }).Count -gt 0) { $items.Add('los archivos privados') }
    if (Test-Path -LiteralPath (Join-Path $folder 'work')) { $items.Add('la carpeta de trabajo') }
    if (Test-Path -LiteralPath (Join-Path $folder '.engram\config.json')) { $items.Add('la configuración de Engram') }
    if ($items.Count -eq 0) { return 'nada' }
    return ($items -join ', ')
}

function Invoke-Step {
    param([Parameter(Mandatory = $true)][string]$Step, [Parameter(Mandatory = $true)][scriptblock]$Action)
    Write-Host "[worktree] $Step..." -ForegroundColor Cyan
    try { & $Action }
    catch {
        $cause = "$($_.Exception.Message)".Trim().TrimEnd('.')
        Exit-Worktree (Get-WorktreeMessage StepFailed $Step, $cause, (Get-CreatedList), $Name)
    }
}

Invoke-Step 'crear la rama y la copia' {
    if (-not (Test-Path -LiteralPath $Root)) { New-Item -ItemType Directory -Path $Root -Force | Out-Null }
    $result = Invoke-Git -Path $mainRoot -Arguments @('worktree', 'add', '--no-track', '-b', $branch, $folder, "refs/heads/$Base")
    if ($result.ExitCode -ne 0) { throw "git worktree add: $(Get-NativeText $result)" }
}

Invoke-Step 'registrar la base' {
    $result = Invoke-Git -Path $mainRoot -Arguments @('config', "branch.$branch.worktreebase", $Base)
    if ($result.ExitCode -ne 0) { throw "git config: $(Get-NativeText $result)" }
    $result = Invoke-Git -Path $mainRoot -Arguments @('config', '--add', "worktreebase.$Base.created", $Name)
    if ($result.ExitCode -ne 0) { throw "git config: $(Get-NativeText $result)" }
}

Invoke-Step 'copiar los archivos privados' {
    foreach ($relative in (Get-PrivateFiles)) {
        $source = Join-Path $mainRoot $relative
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { continue }
        $target = Join-Path $folder $relative
        $targetDir = Split-Path -Parent $target
        if (-not (Test-Path -LiteralPath $targetDir)) { New-Item -ItemType Directory -Path $targetDir -Force | Out-Null }
        Copy-Item -LiteralPath $source -Destination $target
    }
}

Invoke-Step 'crear la carpeta de trabajo' {
    New-Item -ItemType Directory -Path (Join-Path $folder 'work') -Force | Out-Null
}

Invoke-Step 'asignar el proyecto Engram' {
    $result = Invoke-NativeTool -FilePath $engram -Arguments @('init', $project) -WorkingDirectory $folder
    if ($result.ExitCode -ne 0) { throw "engram init: $(Get-NativeText $result)" }
}

Invoke-Step 'comprobar que la copia está limpia' {
    $pending = @(Get-StatusPaths -Path $folder)
    if ($pending.Count -gt 0) { throw "git ve cambios: $($pending -join ', ')" }
}

if (-not $NoOpen) {
    Invoke-Step 'abrir el editor' {
        $result = Invoke-NativeTool -FilePath $code -Arguments @('-n', $folder)
        if ($result.ExitCode -ne 0) { throw "code -n: $(Get-NativeText $result)" }
    }
}

Write-Host ''
Write-Host "Worktree '$Name' creado." -ForegroundColor Green
Write-Host "  Carpeta:         $folder"
Write-Host "  Rama:            $branch"
Write-Host "  Base:            $Base"
Write-Host "  Proyecto Engram: $project"
if ($NoOpen) {
    Write-Host "Siguiente paso: abre '$folder' en una ventana nueva del editor, inicia allí una sesión nueva del agente y pide el trabajo del cambio '$Name'."
}
else {
    Write-Host "Siguiente paso: abre una sesión nueva del agente en la ventana del editor de '$folder' y pide el trabajo del cambio '$Name'."
}
exit 0
