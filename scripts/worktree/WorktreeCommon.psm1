<#
.SYNOPSIS
    Shared rules of the MuninnDB working copies (worktrees): derived names, which copy a session runs in,
    git queries, Engram projects, Claude Code memory and the user-facing messages.

.DESCRIPTION
    Imported by every script in scripts/worktree (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md). Works in Windows
    PowerShell 5.1.

    From one change name <n> (lowercase letters, digits and single hyphens) everything is derived:

      branch          feat/<n>
      folder          <Root>\muninndb-<n>        (Root defaults to C:\wt)
      Engram project  muninndb-<n>
      work folder     <copy>\work                     (ignored by git)
      discard tag     exp/<n>

    The main copy is the one whose .git is a folder; it is the parent of `git rev-parse --git-common-dir`. The
    base of each copy is recorded in the shared repository config as branch.feat/<n>.worktreebase, and every
    copy ever created on a base is appended to the multi-valued worktreebase.<base>.created, so discarding a
    rehearsal also finds the copies that were already removed. A rehearsal base carries
    worktreebase.<base>.rehearsal = true.

    User-facing messages are Spanish and verbatim from the PRD table "Validaciones y mensajes"; the few that the
    PRD does not define (missing tools, failed steps outside creation) follow its style.
#>

Set-StrictMode -Version 2.0

$script:FolderPrefix = 'muninndb-'
$script:EngramPrefix = 'muninndb-'
$script:PrivateFiles = @('.claude\settings.local.json', '.atl\skill-registry.md')
$script:NamePattern = '^[a-z0-9]+(-[a-z0-9]+)*$'
$script:MaxPath = 259
$script:BuildOutputReserve = 100

$script:Messages = @{
    # PRD, "Validaciones y mensajes" (verbatim).
    CopyContext     = "Esto se hace desde la copia principal, y esta sesión está en la copia '{0}'. No se ha cambiado nada."
    MergeFromMain   = 'La fusión se hace desde la copia del cambio, no desde la copia principal. No se ha cambiado nada.'
    InvalidName     = "Nombre '{0}' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha creado nada."
    NameTooLong     = "El nombre '{0}' tiene {1} caracteres; el máximo es {2} para que ninguna ruta pase de 259. No se ha creado nada."
    BaseMissing     = "La base '{0}' no existe. Para un ensayo, créala antes con la frase de crear base de ensayo. No se ha creado nada."
    AlreadyExists   = "Ya existe {0} '{1}'. No se ha creado nada."
    StepFailed      = "Falló el paso '{0}': {1}. Ya creado: {2}. Corrige la causa y limpia con la frase de eliminar el worktree '{3}'."
    CopyDirty       = 'La copia tiene cambios sin confirmar: {0}. Confírmalos o descártalos antes. No se ha cambiado nada.'
    BaseAhead       = "La base '{0}' tiene {1} commit(s) que tu rama no contiene. Incorpóralos, vuelve a verificar y repite la fusión."
    NothingToMerge  = "La rama 'feat/{0}' no tiene nada que la base '{1}' no tenga. No se ha cambiado nada."
    BaseOpenDirty   = "La base '{0}' está abierta en '{1}' con cambios sin confirmar. Confírmalos o guárdalos allí y repite la fusión."
    Merged          = "'{0}' avanza hasta 'feat/{1}' ({2}). La copia y la rama se conservan."
    Unmerged        = "No se elimina '{0}': la rama tiene {1} commit(s) que no están en '{2}'. Para descartarla y conservar su historia en exp/{0}, pide descartar el worktree. No se ha eliminado nada."
    WouldLose       = "No se elimina '{0}'. Se perdería: {1}. No se ha eliminado nada."
    SessionInside   = "Esta sesión está dentro de '{0}'. Elimínala desde la copia principal."
    InUse           = "No se pudo borrar '{0}': está en uso. Cierra el editor, la aplicación o las pruebas que lo usan y repite. No se ha cerrado ningún proceso."
    TagExists       = 'Ya existe la etiqueta exp/{0}. No se ha eliminado nada.'
    InvalidNameRm   = "Nombre '{0}' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha eliminado nada."
    NotRehearsal    ="'{0}' no es una base de ensayo. No se ha cambiado nada."
    BaseExists      = "La base '{0}' ya existe. No se ha creado nada."
    NoWorktrees     = 'No hay worktrees (solo la copia principal).'

    # Not in the PRD table: same style.
    ToolMissing     = "No se encuentra '{0}' en el PATH. No se ha creado nada."
    ToolMissingRm   = "No se encuentra '{0}' en el PATH. No se ha eliminado nada."
    EngramFailed    = 'No se pudo consultar Engram: {0}. No se ha creado nada.'
    EngramFailedRm  = 'No se pudo consultar Engram: {0}. No se ha eliminado nada.'
    NotOnFeature    = "La copia no está en una rama feat/<nombre> (está en '{0}'). No se ha cambiado nada."
    NoRecordedBase  = "La rama 'feat/{0}' no tiene base registrada. No se ha cambiado nada."
    MergeBaseGone   = "La base '{0}' no existe. No se ha cambiado nada."
    StepUnchanged   = "Falló el paso '{0}': {1}. No se ha cambiado nada."
    StepNotCreated  = "Falló el paso '{0}': {1}. No se ha creado nada."
    RemoveFailed    = "Falló el paso '{0}': {1}. Ya eliminado: {2}. Corrige la causa y repite."
    NothingToRemove = "No existe '{0}': ni copia, ni carpeta, ni rama. No se ha eliminado nada."
    ForeignFolder   = "La carpeta '{0}' no es una copia de este repositorio. No se ha eliminado nada."
    BaseCheckedOut  = "La base '{0}' está abierta en '{1}'. Cambia allí a otra rama y repite. No se ha cambiado nada."
    NothingLeft     = "No queda nada del ensayo '{0}'. No se ha cambiado nada."
}

# ---------------------------------------------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------------------------------------------

function Initialize-WorktreeConsole {
    <# Spanish messages must survive a redirected stdout: the agent reads it as UTF-8. #>
    try {
        if ([Console]::IsOutputRedirected) { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) }
    }
    catch { }
}

function Get-WorktreeMessage {
    <# One user-facing message, placeholders filled in order. #>
    param([Parameter(Mandatory = $true)][string]$Key, [object[]]$Values = @())
    return ($script:Messages[$Key] -f $Values)
}

function Exit-Worktree {
    <# Prints the message and ends the script with the exit code (0 done, 3 merge must go back, 1 anything else). #>
    param([Parameter(Mandatory = $true)][string]$Message, [int]$Code = 1)
    $color = if ($Code -eq 0) { 'Green' } else { 'Red' }
    Write-Host $Message -ForegroundColor $color
    exit $Code
}

# ---------------------------------------------------------------------------------------------------------------
# Native tools
# ---------------------------------------------------------------------------------------------------------------

function Invoke-NativeTool {
    <#
    Runs an executable and returns ExitCode, Out (stdout lines) and Err (stderr lines). Never throws on a non-zero
    exit or on stderr output, and never prompts: stdin is empty and stderr is not a terminal.
    #>
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory
    )

    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $pushed = $false
    $raw = @()
    $code = 0
    try {
        if ($WorkingDirectory) { Push-Location -LiteralPath $WorkingDirectory; $pushed = $true }
        $raw = @($null | & $FilePath @Arguments 2>&1)
        $code = $LASTEXITCODE
    }
    finally {
        if ($pushed) { Pop-Location }
        $ErrorActionPreference = $previous
    }

    $out = New-Object System.Collections.Generic.List[string]
    $err = New-Object System.Collections.Generic.List[string]
    foreach ($item in $raw) {
        if ($item -is [System.Management.Automation.ErrorRecord]) { $err.Add("$($item.Exception.Message)".TrimEnd()) }
        else { $out.Add("$item".TrimEnd()) }
    }
    return [pscustomobject]@{ ExitCode = $code; Out = $out.ToArray(); Err = $err.ToArray() }
}

function Get-NativeText {
    <# The first line a failed tool said (stderr first), at most 200 characters, for a message: a tool's
       usage or help text after it would bury the message. #>
    param([Parameter(Mandatory = $true)]$Result)
    $lines = @(@($Result.Err) + @($Result.Out) | Where-Object { $_ -and $_.Trim() -and $_ -notmatch '^(Update available|To update:|\s+go install|\s+or: https)' })
    $text = if ($lines.Count -gt 0) { "$($lines[0])".Trim() } else { '' }
    if (-not $text) { $text = "código de salida $($Result.ExitCode)" }
    if ($text.Length -gt 200) { $text = $text.Substring(0, 200) + '…' }
    return $text.TrimEnd('.')
}

function Invoke-Git {
    <# Runs git in -Path (core.quotepath off). Same result shape as Invoke-NativeTool. #>
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string[]]$Arguments)
    return Invoke-NativeTool -FilePath 'git' -Arguments (@('-C', $Path, '-c', 'core.quotepath=off') + $Arguments)
}

function Get-GitLine {
    <# First stdout line of a git command, or $null when it fails or prints nothing. #>
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string[]]$Arguments)
    $result = Invoke-Git -Path $Path -Arguments $Arguments
    if ($result.ExitCode -ne 0 -or @($result.Out).Count -eq 0) { return $null }
    return @($result.Out)[0]
}

function Test-GitRef {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Ref)
    return ((Invoke-Git -Path $Path -Arguments @('show-ref', '--verify', '--quiet', $Ref)).ExitCode -eq 0)
}

function Get-StatusPaths {
    <# Paths that `git status --porcelain` reports in a copy (modified, staged or untracked, not ignored). #>
    param([Parameter(Mandatory = $true)][string]$Path, [switch]$TrackedOnly)
    $arguments = @('status', '--porcelain')
    if ($TrackedOnly) { $arguments += '--untracked-files=no' }
    $result = Invoke-Git -Path $Path -Arguments $arguments
    if ($result.ExitCode -ne 0) { throw "git status: $(Get-NativeText $result)" }
    return @($result.Out | Where-Object { $_ } | ForEach-Object { if ($_.Length -gt 3) { $_.Substring(3) } else { $_ } })
}

function Get-CommandPath {
    <# Path of an executable on PATH (.exe or .cmd), or $null. #>
    param([Parameter(Mandatory = $true)][string]$Name)
    $command = Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $command) { return $null }
    return $command.Path
}

# ---------------------------------------------------------------------------------------------------------------
# Paths and copies
# ---------------------------------------------------------------------------------------------------------------

function ConvertTo-NormalPath {
    <# Absolute Windows path with backslashes and no trailing separator. #>
    param([Parameter(Mandatory = $true)][string]$Path)
    $full = [System.IO.Path]::GetFullPath($Path.Replace('/', '\'))
    if ($full.Length -gt 3) { $full = $full.TrimEnd('\') }
    return $full
}

function Test-SamePath {
    param([Parameter(Mandatory = $true)][string]$A, [Parameter(Mandatory = $true)][string]$B)
    return [string]::Equals((ConvertTo-NormalPath $A), (ConvertTo-NormalPath $B), [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-PathWithin {
    <# True when -Path is -Folder or lies inside it. #>
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Folder)
    $p = ConvertTo-NormalPath $Path
    $f = ConvertTo-NormalPath $Folder
    return ($p.Equals($f, [System.StringComparison]::OrdinalIgnoreCase) -or
            $p.StartsWith($f + '\', [System.StringComparison]::OrdinalIgnoreCase))
}

function Get-RepositoryContext {
    <#
    The main copy (parent of the git common dir), the copy that holds -Path and whether that copy is the main
    one (its .git is a folder; in a copy it is a file).
    #>
    param([Parameter(Mandatory = $true)][string]$Path)

    $top = Get-GitLine -Path $Path -Arguments @('rev-parse', '--show-toplevel')
    $common = Get-GitLine -Path $Path -Arguments @('rev-parse', '--path-format=absolute', '--git-common-dir')
    if (-not $top -or -not $common) { throw "'$Path' no está dentro de un repositorio git." }
    $copyRoot = ConvertTo-NormalPath $top
    $mainRoot = ConvertTo-NormalPath (Split-Path -Parent (ConvertTo-NormalPath $common))
    $isMain = Test-Path -LiteralPath (Join-Path $copyRoot '.git') -PathType Container
    return [pscustomobject]@{ MainRoot = $mainRoot; CopyRoot = $copyRoot; IsMain = $isMain; GitCommonDir = (ConvertTo-NormalPath $common) }
}

function Get-WorktreeList {
    <# `git worktree list --porcelain` as objects: Path, Head, Branch (refs/heads/...), IsMain (the first). #>
    param([Parameter(Mandatory = $true)][string]$MainRoot)

    $result = Invoke-Git -Path $MainRoot -Arguments @('worktree', 'list', '--porcelain')
    if ($result.ExitCode -ne 0) { throw "git worktree list: $(Get-NativeText $result)" }
    $entries = New-Object System.Collections.Generic.List[object]
    $current = $null
    foreach ($line in $result.Out) {
        if ($line -match '^worktree (.+)$') {
            $current = [pscustomobject]@{ Path = (ConvertTo-NormalPath $Matches[1]); Head = $null; Branch = $null; IsMain = ($entries.Count -eq 0) }
            $entries.Add($current)
        }
        elseif ($null -ne $current -and $line -match '^HEAD (\S+)$') { $current.Head = $Matches[1] }
        elseif ($null -ne $current -and $line -match '^branch (\S+)$') { $current.Branch = $Matches[1] }
    }
    return $entries.ToArray()
}

function Get-SessionCopy {
    <#
    The copy (not the main one) this session runs in: the copy that holds the script, else the registered copy
    that holds the current directory. $null when the session is in the main copy.
    #>
    param([Parameter(Mandatory = $true)]$Context, [object[]]$Worktrees = @())
    if (-not $Context.IsMain) { return $Context.CopyRoot }
    $here = (Get-Location).ProviderPath
    foreach ($worktree in $Worktrees) {
        if (-not $worktree.IsMain -and (Test-PathWithin -Path $here -Folder $worktree.Path)) { return $worktree.Path }
    }
    return $null
}

function Get-BranchHolder {
    <# The registered copy (main included) that has refs/heads/<Branch> checked out, or $null. #>
    param([Parameter(Mandatory = $true)][string]$Branch, [object[]]$Worktrees = @())
    foreach ($worktree in $Worktrees) {
        if ($worktree.Branch -eq "refs/heads/$Branch") { return $worktree.Path }
    }
    return $null
}

function Resolve-RootPath {
    <# -Root as an absolute path, resolved against the current PowerShell location. #>
    param([Parameter(Mandatory = $true)][string]$Root)
    $resolved = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Root)
    return ConvertTo-NormalPath $resolved
}

# ---------------------------------------------------------------------------------------------------------------
# Names
# ---------------------------------------------------------------------------------------------------------------

function Test-ChangeName {
    param([string]$Name)
    return ($Name -cmatch $script:NamePattern)
}

function Get-CopyFolder {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string]$Name)
    return Join-Path (ConvertTo-NormalPath $Root) ($script:FolderPrefix + $Name)
}

function Get-EngramProjectName {
    param([Parameter(Mandatory = $true)][string]$Name)
    return $script:EngramPrefix + $Name
}

function Get-MaxNameLength {
    <#
    Longest change name that keeps every path under 259 characters: 259 - length of '<Root>\muninndb-' - 1
    (the separator after the name) - max(longest path tracked in the base, 100 reserved for build output).
    #>
    param(
        [Parameter(Mandatory = $true)][string]$MainRoot,
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Base
    )
    $prefix = (ConvertTo-NormalPath $Root) + '\' + $script:FolderPrefix
    $result = Invoke-Git -Path $MainRoot -Arguments @('ls-tree', '-r', '--name-only', "refs/heads/$Base")
    if ($result.ExitCode -ne 0) { throw "git ls-tree: $(Get-NativeText $result)" }
    $longest = 0
    foreach ($path in $result.Out) { if ($path.Length -gt $longest) { $longest = $path.Length } }
    return $script:MaxPath - $prefix.Length - 1 - [Math]::Max($longest, $script:BuildOutputReserve)
}

# ---------------------------------------------------------------------------------------------------------------
# Recorded bases
# ---------------------------------------------------------------------------------------------------------------

function Get-RecordedBase {
    <# The base recorded for feat/<Name>, or $null. #>
    param([Parameter(Mandatory = $true)][string]$MainRoot, [Parameter(Mandatory = $true)][string]$Name)
    return Get-GitLine -Path $MainRoot -Arguments @('config', '--get', "branch.feat/$Name.worktreebase")
}

function Get-CreatedNames {
    <# Every change name ever created on -Base (worktreebase.<base>.created), without repetitions. #>
    param([Parameter(Mandatory = $true)][string]$MainRoot, [Parameter(Mandatory = $true)][string]$Base)
    $result = Invoke-Git -Path $MainRoot -Arguments @('config', '--get-all', "worktreebase.$Base.created")
    if ($result.ExitCode -ne 0) { return @() }
    return @($result.Out | Where-Object { $_ } | Select-Object -Unique)
}

function Get-NamesOnBase {
    <# Change names whose live branch feat/<n> records -Base as its base. #>
    param([Parameter(Mandatory = $true)][string]$MainRoot, [Parameter(Mandatory = $true)][string]$Base)
    $result = Invoke-Git -Path $MainRoot -Arguments @('config', '--get-regexp', '^branch\.feat/.*\.worktreebase$')
    if ($result.ExitCode -ne 0) { return @() }
    $names = foreach ($line in $result.Out) {
        if ($line -match '^branch\.feat/(.+)\.worktreebase (.+)$' -and $Matches[2] -ceq $Base) { $Matches[1] }
    }
    return @($names)
}

# ---------------------------------------------------------------------------------------------------------------
# Engram
# ---------------------------------------------------------------------------------------------------------------

function Get-EngramProjects {
    <# Names of the projects Engram knows (engram projects list). Throws when Engram cannot answer. #>
    param([Parameter(Mandatory = $true)][string]$Engram)
    $result = Invoke-NativeTool -FilePath $Engram -Arguments @('projects', 'list')
    if ($result.ExitCode -ne 0) { throw "engram projects list: $(Get-NativeText $result)" }
    $names = foreach ($line in $result.Out) {
        if ($line -match '^\s+(\S+)\s+\d+\s+obs\b') { $Matches[1] }
    }
    return @($names)
}

function Remove-EngramProject {
    <# Hard-deletes an Engram project when it exists. Throws when Engram fails. #>
    param([Parameter(Mandatory = $true)][string]$Engram, [Parameter(Mandatory = $true)][string]$Project)
    if (@(Get-EngramProjects -Engram $Engram) -notcontains $Project) { return $false }
    $result = Invoke-NativeTool -FilePath $Engram -Arguments @('delete', 'project', $Project, '--hard')
    if ($result.ExitCode -ne 0) { throw "engram delete project ${Project}: $(Get-NativeText $result)" }
    return $true
}

# ---------------------------------------------------------------------------------------------------------------
# Claude Code memory
# ---------------------------------------------------------------------------------------------------------------

function Get-DefaultClaudeProjectsDir {
    if ($env:CLAUDE_CONFIG_DIR) { return Join-Path $env:CLAUDE_CONFIG_DIR 'projects' }
    return Join-Path $env:USERPROFILE '.claude\projects'
}

function Get-ClaudeProjectKey {
    <# Claude Code's folder name for a session path: every character other than a letter or digit becomes '-'. #>
    param([Parameter(Mandatory = $true)][string]$Path)
    return ((ConvertTo-NormalPath $Path) -replace '[^A-Za-z0-9]', '-')
}

function Get-ClaudeMemoryFolders {
    <# The memory folders Claude Code keeps for -CopyFolder (case-insensitive); never the session history. #>
    param([Parameter(Mandatory = $true)][string]$ClaudeProjectsDir, [Parameter(Mandatory = $true)][string]$CopyFolder)
    if (-not (Test-Path -LiteralPath $ClaudeProjectsDir -PathType Container)) { return @() }
    $key = Get-ClaudeProjectKey -Path $CopyFolder
    $folders = Get-ChildItem -LiteralPath $ClaudeProjectsDir -Directory -Force |
        Where-Object { $_.Name -ieq $key } |
        ForEach-Object { Join-Path $_.FullName 'memory' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Container }
    return @($folders)
}

# ---------------------------------------------------------------------------------------------------------------
# Private files and deletion without forcing
# ---------------------------------------------------------------------------------------------------------------

function Get-PrivateFiles { return $script:PrivateFiles }

function Remove-FolderTree {
    <#
    Deletes -Path bottom-up without following links and without stopping any process. Returns $null when it is
    gone, or the first file or folder that could not be deleted (in use).
    #>
    param([Parameter(Mandatory = $true)][string]$Path)
    $directory = New-Object System.IO.DirectoryInfo($Path)
    if (-not $directory.Exists) { return $null }
    foreach ($entry in $directory.GetFileSystemInfos()) {
        $isLink = [bool]($entry.Attributes -band [System.IO.FileAttributes]::ReparsePoint)
        if ($entry -is [System.IO.DirectoryInfo] -and -not $isLink) {
            $blocked = Remove-FolderTree -Path $entry.FullName
            if ($blocked) { return $blocked }
            continue
        }
        try {
            if ($entry.Attributes -band [System.IO.FileAttributes]::ReadOnly) {
                $entry.Attributes = [System.IO.FileAttributes]([int]$entry.Attributes -band (-bnot [int][System.IO.FileAttributes]::ReadOnly))
            }
            $entry.Delete()
        }
        catch { return $entry.FullName }
    }
    try { $directory.Delete() }
    catch { return $directory.FullName }
    return $null
}

function Test-OwnLeftover {
    <#
    True when a folder that git no longer registers may be deleted as what is left of one of this repository's
    copies: its .git file points into this repository's worktrees folder, or it has no .git and the copy's base is
    still recorded (-RecordedBase). A removal deletes the branch, and with it the recorded base, only after the
    folder, so a recorded base proves an unfinished removal; a folder without .git and without it is someone
    else's and is never deleted.
    #>
    param(
        [Parameter(Mandatory = $true)][string]$Folder,
        [Parameter(Mandatory = $true)][string]$GitCommonDir,
        [string]$RecordedBase
    )
    $dotGit = Join-Path $Folder '.git'
    if (-not (Test-Path -LiteralPath $dotGit)) { return [bool]$RecordedBase }
    if (Test-Path -LiteralPath $dotGit -PathType Container) { return $false }
    $line = @(Get-Content -LiteralPath $dotGit -TotalCount 1)
    if ($line.Count -eq 0 -or $line[0] -notmatch '^gitdir:\s*(.+)$') { return $false }
    return (Test-PathWithin -Path $Matches[1].Trim() -Folder (Join-Path $GitCommonDir 'worktrees'))
}

function Remove-CopyFolder {
    <#
    Removes a copy's folder and registration, in that order, never with --force. Returns $null when done,
    or a failure: InUse (the path in use) or Step and Cause. Git unregisters a copy even when a file in use stops
    the delete; what is left is deleted here (the copy was clean), so running it again completes the removal.
    #>
    param(
        [Parameter(Mandatory = $true)][string]$MainRoot,
        [Parameter(Mandatory = $true)][string]$GitCommonDir,
        [Parameter(Mandatory = $true)][string]$Folder,
        [bool]$Registered,
        [string]$RecordedBase
    )

    if ($Registered -and (Test-Path -LiteralPath $Folder)) {
        $result = Invoke-Git -Path $MainRoot -Arguments @('worktree', 'remove', $Folder)
        if ($result.ExitCode -ne 0) {
            $still = @(Get-WorktreeList -MainRoot $MainRoot | Where-Object { -not $_.IsMain -and (Test-SamePath $_.Path $Folder) })
            if ($still.Count -gt 0) {
                return [pscustomobject]@{ InUse = $null; Step = 'eliminar la copia'; Cause = "git worktree remove: $(Get-NativeText $result)" }
            }
        }
    }

    if (Test-Path -LiteralPath $Folder) {
        # A copy git just unregistered here was clean, so what is left of it is this removal's own.
        if (-not $Registered -and -not (Test-OwnLeftover -Folder $Folder -GitCommonDir $GitCommonDir -RecordedBase $RecordedBase)) {
            return [pscustomobject]@{ InUse = $null; Step = 'eliminar la carpeta'; Cause = "'$Folder' no es una copia de este repositorio" }
        }
        $blocked = Remove-FolderTree -Path $Folder
        if ($blocked) { return [pscustomobject]@{ InUse = $blocked; Step = $null; Cause = $null } }
    }

    $null = Invoke-Git -Path $MainRoot -Arguments @('worktree', 'prune')
    return $null
}

Export-ModuleMember -Function Initialize-WorktreeConsole, Get-WorktreeMessage, Exit-Worktree, Invoke-NativeTool, Get-NativeText,
    Invoke-Git, Get-GitLine, Test-GitRef, Get-StatusPaths, Get-CommandPath, ConvertTo-NormalPath, Test-SamePath, Test-PathWithin,
    Get-RepositoryContext, Get-WorktreeList, Get-SessionCopy, Get-BranchHolder, Resolve-RootPath, Test-ChangeName, Get-CopyFolder,
    Get-EngramProjectName, Get-MaxNameLength, Get-RecordedBase, Get-CreatedNames, Get-NamesOnBase, Get-EngramProjects,
    Remove-EngramProject, Get-DefaultClaudeProjectsDir, Get-ClaudeProjectKey, Get-ClaudeMemoryFolders, Get-PrivateFiles,
    Remove-FolderTree, Test-OwnLeftover, Remove-CopyFolder
