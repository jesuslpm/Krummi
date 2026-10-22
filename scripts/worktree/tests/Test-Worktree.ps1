<#
.SYNOPSIS
    Tests of the worktree scripts (chicago-crime PRD docs/prd/worktrees/worktrees.prd.md) on throwaway repositories.

.DESCRIPTION
    Every scenario runs in a fresh folder under %LOCALAPPDATA%\Temp\wtt-<random>: a main copy made with
    `git init` that holds a copy of scripts/worktree, the repository's .gitignore,
    .claude/settings.local.json and .atl/skill-registry.md. The scripts run as child
    processes through their public interface with -Root <scenario>\wt, -ClaudeProjectsDir
    <scenario>\claude-projects and -NoOpen, and Engram runs against a throwaway database (ENGRAM_DATA_DIR). The
    tests never touch the real repository's branches, C:\wt, the real Claude Code memory, the real Engram
    database.

    Each test prints PASS or FAIL with its name. The script exits with 1 when any test fails.

.PARAMETER Keep
    Keep the temporary folder for inspection instead of deleting it.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/tests/Test-Worktree.ps1
#>
[CmdletBinding()]
param([switch]$Keep)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
if ([Console]::IsOutputRedirected) { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false) }

$SourceDir = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $SourceDir '..\..')).Path
$P = 'wtt-' + [guid]::NewGuid().ToString('N').Substring(0, 6)
$TestRoot = Join-Path (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Temp') $P
$OriginalEngramDataDir = $env:ENGRAM_DATA_DIR

$script:Passed = 0
$script:Failures = New-Object System.Collections.Generic.List[string]

# -----------------------------------------------------------------------------------------------------------------
# Harness
# -----------------------------------------------------------------------------------------------------------------

function Test-Case {
    param([Parameter(Mandatory = $true)][string]$Name, [Parameter(Mandatory = $true)][scriptblock]$Body)
    try {
        & $Body
        $script:Passed++
        Write-Host "PASS  $Name"
    }
    catch {
        $script:Failures.Add($Name)
        Write-Host "FAIL  $Name -- $($_.Exception.Message)"
    }
}

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function Assert-Equal {
    param($Expected, $Actual, [string]$What)
    if ("$Expected" -cne "$Actual") { throw "${What}: expected [$Expected], got [$Actual]" }
}

function Assert-Exit {
    param($Result, [int]$Code)
    if ($Result.ExitCode -ne $Code) {
        throw "exit code: expected $Code, got $($Result.ExitCode). Output: $($Result.Lines -join ' | ') $($Result.Err.Trim())"
    }
}

function Assert-Line {
    <# The output has exactly this line. #>
    param($Result, [string]$Expected)
    if (@($Result.Lines) -cnotcontains $Expected) {
        throw "missing line [$Expected]. Output: $($Result.Lines -join ' | ') $($Result.Err.Trim())"
    }
}

function Assert-LineMatch {
    param($Result, [string]$Pattern)
    if (-not (@($Result.Lines) | Where-Object { $_ -cmatch $Pattern })) {
        throw "no line matches [$Pattern]. Output: $($Result.Lines -join ' | ')"
    }
}

function Assert-Unchanged {
    param([string]$Before, $Fixture)
    $after = Get-State -Fixture $Fixture
    if ($after -cne $Before) {
        $old = @($Before -split "`n")
        $new = @($after -split "`n")
        $diff = @(Compare-Object -ReferenceObject $old -DifferenceObject $new -CaseSensitive | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" })
        if ($diff.Count -eq 0) {
            for ($i = 0; $i -lt [Math]::Max($old.Count, $new.Count); $i++) {
                if ($i -ge $old.Count -or $i -ge $new.Count -or $old[$i] -cne $new[$i]) {
                    $diff = @("line $i was [$(if ($i -lt $old.Count) { $old[$i] })], now [$(if ($i -lt $new.Count) { $new[$i] })]")
                    break
                }
            }
        }
        throw "the state changed: $($diff -join ' ; ')"
    }
}

function Assert-Refused {
    <# Exit code, exact message and nothing changed. #>
    param($Result, [string]$Message, [string]$Before, $Fixture, [int]$Code = 1)
    Assert-Exit -Result $Result -Code $Code
    Assert-Line -Result $Result -Expected $Message
    Assert-Unchanged -Before $Before -Fixture $Fixture
}

function Invoke-GitIn {
    <# git -C <Path> <Arguments>; returns stdout lines and throws on failure unless -AllowFailure. #>
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string[]]$Arguments, [switch]$AllowFailure)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $output = @(& git -C $Path @Arguments 2>$null) }
    finally { $ErrorActionPreference = $previous }
    $code = $LASTEXITCODE
    if ($code -ne 0 -and -not $AllowFailure) { throw "git $($Arguments -join ' ') in '$Path' failed with exit $code" }
    return @($output | ForEach-Object { "$_".TrimEnd() } | Where-Object { $_ -ne '' })
}

function Test-Ref {
    param([string]$Path, [string]$Ref)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & git -C $Path show-ref --verify --quiet $Ref 2>$null } finally { $ErrorActionPreference = $previous }
    return ($LASTEXITCODE -eq 0)
}

function Get-Sha {
    param([string]$Path, [string]$Ref)
    return @(Invoke-GitIn -Path $Path -Arguments @('rev-parse', $Ref))[0]
}

function Invoke-Engram {
    param([string[]]$Arguments)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $output = @(& engram @Arguments 2>$null) } finally { $ErrorActionPreference = $previous }
    return @($output | ForEach-Object { "$_" })
}

function Get-EngramProjectList {
    $names = foreach ($line in (Invoke-Engram -Arguments @('projects', 'list'))) {
        if ($line -match '^\s+(\S+)\s+\d+\s+obs\b') { $Matches[1] }
    }
    return @($names)
}

function New-EngramProject {
    <# A project exists in Engram once something is saved in it. #>
    param([string]$Project)
    $null = Invoke-Engram -Arguments @('save', 'worktree test', "memory of $Project", '--project', $Project)
    Assert-True (@(Get-EngramProjectList) -contains $Project) "could not create the Engram project $Project"
}

function Invoke-Tool {
    <# Runs a script as a child process: powershell -NoProfile -ExecutionPolicy Bypass -File <script> <args>. #>
    param(
        [Parameter(Mandatory = $true)][string]$Script,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$Cwd,
        [hashtable]$Environment = @{}
    )
    $all = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $Script) + $Arguments
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = 'powershell.exe'
    $psi.Arguments = ($all | ForEach-Object { if ($_ -match '[\s"]') { '"' + $_.Replace('"', '\"') + '"' } else { $_ } }) -join ' '
    $psi.WorkingDirectory = $Cwd
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    foreach ($key in $Environment.Keys) { $psi.EnvironmentVariables[$key] = $Environment[$key] }
    $process = [System.Diagnostics.Process]::Start($psi)
    $process.StandardInput.Close()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $out = $stdout.Result
    $lines = @($out -split "`r?`n" | ForEach-Object { $_.TrimEnd() } | Where-Object { $_ -ne '' })
    return [pscustomobject]@{ ExitCode = $process.ExitCode; Out = $out; Err = $stderr.Result; Lines = $lines }
}

function New-Fixture {
    <# A throwaway main copy with the worktree scripts committed and the private files. #>
    param([Parameter(Mandatory = $true)][string]$Label)
    $base = Join-Path $TestRoot $Label
    $main = Join-Path $base 'main'
    New-Item -ItemType Directory -Path $base -Force | Out-Null
    $null = Invoke-GitIn -Path $base -Arguments @('init', '-q', '-b', 'main', 'main')
    $null = Invoke-GitIn -Path $main -Arguments @('config', 'user.name', 'Worktree Tests')
    $null = Invoke-GitIn -Path $main -Arguments @('config', 'user.email', 'worktree-tests@example.invalid')
    $null = Invoke-GitIn -Path $main -Arguments @('config', 'core.autocrlf', 'false')
    $scripts = Join-Path $main 'scripts\worktree'
    New-Item -ItemType Directory -Path $scripts -Force | Out-Null
    Get-ChildItem -LiteralPath $SourceDir -File | Copy-Item -Destination $scripts
    Copy-Item -LiteralPath (Join-Path $RepoRoot '.gitignore') -Destination $main
    [System.IO.File]::WriteAllText((Join-Path $main 'README.md'), "fixture`n")
    $null = Invoke-GitIn -Path $main -Arguments @('add', '-A')
    $null = Invoke-GitIn -Path $main -Arguments @('commit', '-q', '-m', 'init')

    New-Item -ItemType Directory -Path (Join-Path $main '.claude') -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $main '.claude\settings.local.json'), "{ `"permissions`": { `"allow`": [] } }`n")
    New-Item -ItemType Directory -Path (Join-Path $main '.atl') -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $main '.atl\skill-registry.md'), "# Skill registry`n")

    return [pscustomobject]@{
        Base      = $base
        Main      = $main
        Root      = Join-Path $base 'wt'
        Claude    = Join-Path $base 'claude-projects'
        Scripts   = $scripts
    }
}

function Get-Folder {
    param($Fixture, [string]$Name)
    return Join-Path $Fixture.Root "muninndb-$Name"
}

function Get-State {
    <# Everything a refusal must leave as it was. #>
    param([Parameter(Mandatory = $true)]$Fixture)
    $lines = New-Object System.Collections.Generic.List[string]
    foreach ($line in (Invoke-GitIn -Path $Fixture.Main -Arguments @('for-each-ref', '--format=%(refname) %(objectname)'))) { $lines.Add("ref $line") }
    foreach ($line in (Invoke-GitIn -Path $Fixture.Main -Arguments @('worktree', 'list', '--porcelain'))) { $lines.Add("worktree $line") }
    foreach ($line in (Invoke-GitIn -Path $Fixture.Main -Arguments @('config', '--get-regexp', 'worktreebase') -AllowFailure)) { $lines.Add("config $line") }
    foreach ($line in (Invoke-GitIn -Path $Fixture.Main -Arguments @('status', '--porcelain'))) { $lines.Add("main-status $line") }
    $lines.Add("main-head $(Get-Sha -Path $Fixture.Main -Ref 'HEAD')")
    foreach ($folder in @($Fixture.Root, $Fixture.Claude)) {
        if (Test-Path -LiteralPath $folder) {
            Get-ChildItem -LiteralPath $folder -Recurse -Force |
                Where-Object { $_.FullName -notmatch '\\\.git(\\|$)' } |
                ForEach-Object { $lines.Add("fs $($_.FullName) $(if ($_.PSIsContainer) { 'dir' } else { $_.Length })") }
        }
    }
    # Engram lists projects in no stable order: compare them as a set.
    foreach ($project in (Get-EngramProjectList | Sort-Object)) { $lines.Add("engram $project") }
    return ($lines -join "`n")
}

function Invoke-Script {
    <# Runs one worktree script of -Scripts (default: the main copy's) from -Cwd (default: the main copy). #>
    param($Fixture, [string]$Script, [string[]]$Arguments = @(), [string]$Cwd, [string]$Scripts, [hashtable]$Environment = @{})
    if (-not $Cwd) { $Cwd = $Fixture.Main }
    if (-not $Scripts) { $Scripts = $Fixture.Scripts }
    return Invoke-Tool -Script (Join-Path $Scripts $Script) -Arguments $Arguments -Cwd $Cwd -Environment $Environment
}

function New-Copy {
    param($Fixture, [string]$Name, [string]$Base, [string]$Cwd, [string]$Scripts, [hashtable]$Environment = @{})
    # -Name:<value> so that a value starting with '-' is not read as a parameter.
    $arguments = @("-Name:$Name", '-Root', $Fixture.Root, '-NoOpen')
    if ($Base) { $arguments += @('-Base', $Base) }
    return Invoke-Script -Fixture $Fixture -Script 'New-Worktree.ps1' -Arguments $arguments -Cwd $Cwd -Scripts $Scripts -Environment $Environment
}

function Remove-Copy {
    param($Fixture, [string]$Name, [switch]$Discard, [string]$Cwd, [string]$Scripts)
    $arguments = @("-Name:$Name", '-Root', $Fixture.Root, '-ClaudeProjectsDir', $Fixture.Claude)
    if ($Discard) { $arguments += '-Discard' }
    return Invoke-Script -Fixture $Fixture -Script 'Remove-Worktree.ps1' -Arguments $arguments -Cwd $Cwd -Scripts $Scripts
}

function Merge-Copy {
    <# Runs Merge-Worktree.ps1 from the copy, as its agent would. #>
    param($Fixture, [string]$Name)
    $folder = Get-Folder -Fixture $Fixture -Name $Name
    return Invoke-Script -Fixture $Fixture -Script 'Merge-Worktree.ps1' -Cwd $folder -Scripts (Join-Path $folder 'scripts\worktree')
}

function Remove-RehearsalOf {
    param($Fixture, [string]$Base, [string]$Cwd, [string]$Scripts)
    return Invoke-Script -Fixture $Fixture -Script 'Remove-Rehearsal.ps1' -Arguments @('-Base', $Base, '-Root', $Fixture.Root, '-ClaudeProjectsDir', $Fixture.Claude) -Cwd $Cwd -Scripts $Scripts
}

function Add-Commit {
    param([string]$Path, [string]$File, [string]$Text = 'content')
    [System.IO.File]::WriteAllText((Join-Path $Path $File), "$Text`n")
    $null = Invoke-GitIn -Path $Path -Arguments @('add', $File)
    $null = Invoke-GitIn -Path $Path -Arguments @('commit', '-q', '-m', "add $File")
}

function New-ClaudeMemory {
    <# Claude Code's folder for a session path (drive letter lowercase, as when the path is typed so) with a
       memory folder and a session history file. #>
    param($Fixture, [string]$Folder)
    $key = ($Folder -replace '[^A-Za-z0-9]', '-')
    $key = $key.Substring(0, 1).ToLowerInvariant() + $key.Substring(1)
    $project = Join-Path $Fixture.Claude $key
    New-Item -ItemType Directory -Path (Join-Path $project 'memory') -Force | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $project 'memory\MEMORY.md'), "- a memory`n")
    [System.IO.File]::WriteAllText((Join-Path $project 'session.jsonl'), "{}`n")
    return $project
}

function Get-ListRow {
    <# The row of Get-Worktrees for a change: Name, Branch, Base, Merged, Folder, Engram. #>
    param($Result, [string]$Name)
    foreach ($line in $Result.Lines) {
        $cells = @($line.Trim() -split '\s+')
        if ($cells.Count -eq 6 -and $cells[1] -ceq "feat/$Name") {
            return [pscustomobject]@{ Name = $cells[0]; Branch = $cells[1]; Base = $cells[2]; Merged = $cells[3]; Folder = $cells[4]; Engram = $cells[5] }
        }
    }
    throw "no row for '$Name' in: $($Result.Lines -join ' | ')"
}

# -----------------------------------------------------------------------------------------------------------------
# Messages (verbatim from the PRD table "Validaciones y mensajes")
# -----------------------------------------------------------------------------------------------------------------

function Get-MsgCopyContext([string]$Folder) { "Esto se hace desde la copia principal, y esta sesión está en la copia '$Folder'. No se ha cambiado nada." }
$MsgMergeFromMain = 'La fusión se hace desde la copia del cambio, no desde la copia principal. No se ha cambiado nada.'
function Get-MsgInvalidName([string]$Name) { "Nombre '$Name' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha creado nada." }
function Get-MsgTooLong([string]$Name, [int]$Length, [int]$Max) { "El nombre '$Name' tiene $Length caracteres; el máximo es $Max para que ninguna ruta pase de 259. No se ha creado nada." }
function Get-MsgBaseMissing([string]$Base) { "La base '$Base' no existe. Para un ensayo, créala antes con la frase de crear base de ensayo. No se ha creado nada." }
function Get-MsgExists([string]$What, [string]$Value) { "Ya existe $What '$Value'. No se ha creado nada." }
function Get-MsgStepFailed([string]$Step, [string]$Cause, [string]$Created, [string]$Name) { "Falló el paso '$Step': $Cause. Ya creado: $Created. Corrige la causa y limpia con la frase de eliminar el worktree '$Name'." }
function Get-MsgDirty([string]$List) { "La copia tiene cambios sin confirmar: $List. Confírmalos o descártalos antes. No se ha cambiado nada." }
function Get-MsgBaseAhead([string]$Base, [int]$Count) { "La base '$Base' tiene $Count commit(s) que tu rama no contiene. Incorpóralos, vuelve a verificar y repite la fusión." }
function Get-MsgNothingNew([string]$Name, [string]$Base) { "La rama 'feat/$Name' no tiene nada que la base '$Base' no tenga. No se ha cambiado nada." }
function Get-MsgBaseOpenDirty([string]$Base, [string]$Folder) { "La base '$Base' está abierta en '$Folder' con cambios sin confirmar. Confírmalos o guárdalos allí y repite la fusión." }
function Get-MsgMerged([string]$Base, [string]$Name, [string]$Commit) { "'$Base' avanza hasta 'feat/$Name' ($Commit). La copia y la rama se conservan." }
function Get-MsgUnmerged([string]$Name, [int]$Count, [string]$Base) { "No se elimina '$Name': la rama tiene $Count commit(s) que no están en '$Base'. Para descartarla y conservar su historia en exp/$Name, pide descartar el worktree. No se ha eliminado nada." }
function Get-MsgWouldLose([string]$Name, [string]$List) { "No se elimina '$Name'. Se perdería: $List. No se ha eliminado nada." }
function Get-MsgSessionInside([string]$Folder) { "Esta sesión está dentro de '$Folder'. Elimínala desde la copia principal." }
function Get-MsgInUse([string]$File) { "No se pudo borrar '$File': está en uso. Cierra el editor, la aplicación o las pruebas que lo usan y repite. No se ha cerrado ningún proceso." }
function Get-MsgTagExists([string]$Name) { "Ya existe la etiqueta exp/$Name. No se ha eliminado nada." }
function Get-MsgNotRehearsal([string]$Base) { "'$Base' no es una base de ensayo. No se ha cambiado nada." }
function Get-MsgBaseExists([string]$Base) { "La base '$Base' ya existe. No se ha creado nada." }
$MsgNoWorktrees = 'No hay worktrees (solo la copia principal).'

# -----------------------------------------------------------------------------------------------------------------
# Scenarios
# -----------------------------------------------------------------------------------------------------------------

New-Item -ItemType Directory -Path $TestRoot -Force | Out-Null
$env:ENGRAM_DATA_DIR = Join-Path $TestRoot 'engram-data'
Write-Host "Test folder: $TestRoot"

try {
    # =========================== Create, list ===========================
    $F1 = New-Fixture -Label 'create'
    $n1 = "$P-alpha"
    $n2 = "$P-beta"
    $copy1 = Get-Folder -Fixture $F1 -Name $n1
    $copy2 = Get-Folder -Fixture $F1 -Name $n2

    Test-Case 'list: no copies' {
        $r = Invoke-Script -Fixture $F1 -Script 'Get-Worktrees.ps1'
        Assert-Exit $r 0
        Assert-Line $r $MsgNoWorktrees
    }

    Test-Case 'create: valid name on main (AC-W1)' {
        $mainTip = Get-Sha -Path $F1.Main -Ref 'refs/heads/main'
        $r = New-Copy -Fixture $F1 -Name $n1
        Assert-Exit $r 0
        Assert-Equal $mainTip (Get-Sha -Path $F1.Main -Ref "refs/heads/feat/$n1") 'branch tip'
        Assert-True (Test-Path -LiteralPath $copy1 -PathType Container) 'copy folder missing'
        $registered = @(Invoke-GitIn -Path $F1.Main -Arguments @('worktree', 'list', '--porcelain')) -join "`n"
        Assert-True ($registered -match [regex]::Escape("branch refs/heads/feat/$n1")) 'copy not registered'
        Assert-Equal (Get-Content -LiteralPath (Join-Path $F1.Main '.claude\settings.local.json') -Raw) (Get-Content -LiteralPath (Join-Path $copy1 '.claude\settings.local.json') -Raw) 'agent permissions'
        Assert-Equal (Get-Content -LiteralPath (Join-Path $F1.Main '.atl\skill-registry.md') -Raw) (Get-Content -LiteralPath (Join-Path $copy1 '.atl\skill-registry.md') -Raw) 'skill registry'
        Assert-True (Test-Path -LiteralPath (Join-Path $copy1 'work') -PathType Container) 'work folder missing'
        $engram = Get-Content -LiteralPath (Join-Path $copy1 '.engram\config.json') -Raw | ConvertFrom-Json
        Assert-Equal "muninndb-$n1" $engram.project_name 'Engram project of the copy'
        Assert-Equal 'main' (@(Invoke-GitIn -Path $F1.Main -Arguments @('config', '--get', "branch.feat/$n1.worktreebase"))[0]) 'recorded base'
        Assert-True (@(Invoke-GitIn -Path $F1.Main -Arguments @('config', '--get-all', 'worktreebase.main.created')) -contains $n1) 'copy not in worktreebase.main.created'
        Assert-Equal 0 @(Invoke-GitIn -Path $copy1 -Arguments @('status', '--porcelain')).Count 'git status of the copy'
        Assert-LineMatch $r ('^\s*Carpeta:\s+' + [regex]::Escape($copy1) + '$')
        Assert-LineMatch $r ('^\s*Rama:\s+' + [regex]::Escape("feat/$n1") + '$')
        Assert-LineMatch $r '^\s*Base:\s+main$'
        Assert-LineMatch $r ('^\s*Proyecto Engram:\s+' + [regex]::Escape("muninndb-$n1") + '$')
        Assert-Line $r "Siguiente paso: abre '$copy1' en una ventana nueva del editor, inicia allí una sesión nueva del agente y pide el trabajo del cambio '$n1'."
    }

    Test-Case 'rehearsal base: created from main' {
        $mainTip = Get-Sha -Path $F1.Main -Ref 'refs/heads/main'
        $r = Invoke-Script -Fixture $F1 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'ensayo')
        Assert-Exit $r 0
        Assert-Equal $mainTip (Get-Sha -Path $F1.Main -Ref 'refs/heads/ensayo') 'rehearsal base tip'
        Assert-Equal 'main' (@(Invoke-GitIn -Path $F1.Main -Arguments @('symbolic-ref', '--short', 'HEAD'))[0]) 'main copy branch'
    }

    Test-Case 'rehearsal base: refuses an existing base, main included' {
        $before = Get-State -Fixture $F1
        $r = Invoke-Script -Fixture $F1 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'ensayo')
        Assert-Refused $r (Get-MsgBaseExists 'ensayo') $before $F1
        $r = Invoke-Script -Fixture $F1 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'main')
        Assert-Refused $r (Get-MsgBaseExists 'main') $before $F1
    }

    Test-Case 'create: on a rehearsal base (AC-W2)' {
        Add-Commit -Path $F1.Main -File 'after-ensayo.txt'   # main moves on; the rehearsal base does not
        $ensayoTip = Get-Sha -Path $F1.Main -Ref 'refs/heads/ensayo'
        $r = New-Copy -Fixture $F1 -Name $n2 -Base 'ensayo'
        Assert-Exit $r 0
        Assert-Equal $ensayoTip (Get-Sha -Path $F1.Main -Ref "refs/heads/feat/$n2") 'branch tip'
        Assert-Equal 'ensayo' (@(Invoke-GitIn -Path $F1.Main -Arguments @('config', '--get', "branch.feat/$n2.worktreebase"))[0]) 'recorded base'
        Assert-True (@(Invoke-GitIn -Path $F1.Main -Arguments @('config', '--get-all', 'worktreebase.ensayo.created')) -contains $n2) 'copy not in worktreebase.ensayo.created'
        Assert-LineMatch $r '^\s*Base:\s+ensayo$'
        $list = Invoke-Script -Fixture $F1 -Script 'Get-Worktrees.ps1'
        Assert-Equal 'ensayo' (Get-ListRow -Result $list -Name $n2).Base 'listed base'
    }

    Test-Case 'list: every copy with branch, base, merged flag, folder and Engram project, from any copy' {
        Add-Commit -Path $copy2 -File 'beta.txt'
        foreach ($where in @(@{ Cwd = $F1.Main; Scripts = $F1.Scripts }, @{ Cwd = $copy1; Scripts = (Join-Path $copy1 'scripts\worktree') })) {
            $r = Invoke-Script -Fixture $F1 -Script 'Get-Worktrees.ps1' -Cwd $where.Cwd -Scripts $where.Scripts
            Assert-Exit $r 0
            $row1 = Get-ListRow -Result $r -Name $n1
            Assert-Equal "$n1|feat/$n1|main|sí|$copy1|muninndb-$n1" "$($row1.Name)|$($row1.Branch)|$($row1.Base)|$($row1.Merged)|$($row1.Folder)|$($row1.Engram)" 'row 1'
            $row2 = Get-ListRow -Result $r -Name $n2
            Assert-Equal "$n2|feat/$n2|ensayo|no|$copy2|muninndb-$n2" "$($row2.Name)|$($row2.Branch)|$($row2.Base)|$($row2.Merged)|$($row2.Folder)|$($row2.Engram)" 'row 2'
        }
    }

    Test-Case 'isolation: each copy has its own ignored work folder and its own Engram project (AC-W5, AC-W6)' {
        [System.IO.File]::WriteAllText((Join-Path $copy1 'work\run.txt'), "copy 1`n")
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $copy2 'work\run.txt'))) 'the work folders are shared'
        foreach ($copy in @($copy1, $copy2)) {
            Assert-Equal 0 @(Invoke-GitIn -Path $copy -Arguments @('status', '--porcelain')).Count "git status of $copy"
        }
        $token = "isolation$($P.Replace('-', ''))"
        Push-Location -LiteralPath $copy1
        try { $null = Invoke-Engram -Arguments @('save', 'isolation', "$token memory") } finally { Pop-Location }
        $found = @{}
        foreach ($where in @($copy1, $copy2, $F1.Main)) {
            Push-Location -LiteralPath $where
            try { $found[$where] = (@(Invoke-Engram -Arguments @('search', $token)) -join ' ') -match [regex]::Escape("project: muninndb-$n1") }
            finally { Pop-Location }
        }
        Assert-True $found[$copy1] 'the memory is not found from its own copy'
        Assert-True (-not $found[$copy2]) 'the memory is found from another copy'
        Assert-True (-not $found[$F1.Main]) 'the memory is found from the main copy'
    }

    Test-Case 'create: invalid names are refused and nothing is created (AC-W3)' {
        $before = Get-State -Fixture $F1
        foreach ($bad in @('Bad', 'a--b', '-a', 'a-', 'a_b', 'a.b', 'feat/x')) {
            $r = New-Copy -Fixture $F1 -Name $bad
            Assert-Refused $r (Get-MsgInvalidName $bad) $before $F1
        }
    }

    Test-Case 'remove: an invalid name is refused with the removal wording' {
        $before = Get-State -Fixture $F1
        $r = Remove-Copy -Fixture $F1 -Name 'Bad_X'
        Assert-Refused $r "Nombre 'Bad_X' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha eliminado nada." $before $F1
    }

    Test-Case 'create: a name too long for Windows paths is refused (AC-W3)' {
        $longest = 0
        foreach ($path in (Invoke-GitIn -Path $F1.Main -Arguments @('ls-tree', '-r', '--name-only', 'main'))) { if ($path.Length -gt $longest) { $longest = $path.Length } }
        $max = 259 - ("$($F1.Root)\muninndb-").Length - 1 - [Math]::Max($longest, 100)
        $name = 'a' * ($max + 1)
        $before = Get-State -Fixture $F1
        $r = New-Copy -Fixture $F1 -Name $name
        Assert-Refused $r (Get-MsgTooLong $name ($max + 1) $max) $before $F1
    }

    Test-Case 'create: a missing base is refused and never created (AC-W3)' {
        $before = Get-State -Fixture $F1
        $r = New-Copy -Fixture $F1 -Name "$P-gamma" -Base 'nope'
        Assert-Refused $r (Get-MsgBaseMissing 'nope') $before $F1
    }

    Test-Case 'create: an existing branch is refused (AC-W3)' {
        $name = "$P-delta"
        $null = Invoke-GitIn -Path $F1.Main -Arguments @('branch', "feat/$name", 'main')
        $before = Get-State -Fixture $F1
        $r = New-Copy -Fixture $F1 -Name $name
        Assert-Refused $r (Get-MsgExists 'la rama' "feat/$name") $before $F1
        $null = Invoke-GitIn -Path $F1.Main -Arguments @('branch', '-D', "feat/$name")
    }

    Test-Case 'create: an existing folder is refused (AC-W3)' {
        $name = "$P-epsilon"
        $folder = Get-Folder -Fixture $F1 -Name $name
        New-Item -ItemType Directory -Path $folder -Force | Out-Null
        $before = Get-State -Fixture $F1
        $r = New-Copy -Fixture $F1 -Name $name
        Assert-Refused $r (Get-MsgExists 'la carpeta' $folder) $before $F1
        Remove-Item -LiteralPath $folder -Force
    }

    Test-Case 'create: an existing registered copy is refused (AC-W3)' {
        $name = "$P-zeta"
        $folder = Get-Folder -Fixture $F1 -Name $name
        $null = Invoke-GitIn -Path $F1.Main -Arguments @('worktree', 'add', '-q', '--detach', $folder, 'main')
        Remove-Item -LiteralPath $folder -Recurse -Force
        $before = Get-State -Fixture $F1
        $r = New-Copy -Fixture $F1 -Name $name
        Assert-Refused $r (Get-MsgExists 'la copia registrada' $folder) $before $F1
        $null = Invoke-GitIn -Path $F1.Main -Arguments @('worktree', 'prune')
    }

    Test-Case 'create: an existing Engram project is refused (AC-W3)' {
        $name = "$P-eta"
        New-EngramProject -Project "muninndb-$name"
        $before = Get-State -Fixture $F1
        $r = New-Copy -Fixture $F1 -Name $name
        Assert-Refused $r (Get-MsgExists 'el proyecto Engram' "muninndb-$name") $before $F1
    }

    Test-Case 'create, remove and rehearsal operations are refused from a copy (AC-W14)' {
        $before = Get-State -Fixture $F1
        $copyScripts = Join-Path $copy1 'scripts\worktree'
        $expected = Get-MsgCopyContext $copy1
        # The copy's own scripts, and the main copy's scripts run from inside the copy.
        foreach ($scripts in @($copyScripts, $F1.Scripts)) {
            $r = New-Copy -Fixture $F1 -Name "$P-theta" -Cwd $copy1 -Scripts $scripts
            Assert-Refused $r $expected $before $F1
            $r = Remove-Copy -Fixture $F1 -Name $n2 -Cwd $copy1 -Scripts $scripts
            Assert-Refused $r $expected $before $F1
            $r = Invoke-Script -Fixture $F1 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'otro') -Cwd $copy1 -Scripts $scripts
            Assert-Refused $r $expected $before $F1
            $r = Remove-RehearsalOf -Fixture $F1 -Base 'ensayo' -Cwd $copy1 -Scripts $scripts
            Assert-Refused $r $expected $before $F1
        }
    }

    Test-Case 'create: a failing step says what was created and is not undone; removal cleans it' {
        $name = "$P-iota"
        $folder = Get-Folder -Fixture $F1 -Name $name
        $fakeBin = Join-Path $TestRoot 'fake-bin'
        New-Item -ItemType Directory -Path $fakeBin -Force | Out-Null
        $fake = "@echo off`r`nif `"%1`"==`"projects`" (`r`n  echo No projects found.`r`n  exit /b 0`r`n)`r`necho engram init is broken 1>&2`r`necho usage: engram ^<command^> [arguments] 1>&2`r`nexit /b 1`r`n"
        [System.IO.File]::WriteAllText((Join-Path $fakeBin 'engram.cmd'), $fake)
        $r = New-Copy -Fixture $F1 -Name $name -Environment @{ PATH = "$fakeBin;$env:PATH" }
        Assert-Exit $r 1
        $created = "la rama 'feat/$name', la copia '$folder', la base registrada, los archivos privados, la carpeta de trabajo"
        Assert-Line $r (Get-MsgStepFailed 'asignar el proyecto Engram' 'engram init: engram init is broken' $created $name)
        Assert-True (Test-Path -LiteralPath (Join-Path $folder 'work')) 'creation undid its own steps'
        Assert-True (Test-Ref -Path $F1.Main -Ref "refs/heads/feat/$name") 'creation deleted its branch'
        $r = Remove-Copy -Fixture $F1 -Name $name
        Assert-Exit $r 0
        Assert-True (-not (Test-Path -LiteralPath $folder)) 'folder left after removal'
        Assert-True (-not (Test-Ref -Path $F1.Main -Ref "refs/heads/feat/$name")) 'branch left after removal'
    }

    # =========================== Merge ===========================
    $F2 = New-Fixture -Label 'merge'
    $m1 = "$P-m1"
    $m2 = "$P-m2"
    $mcopy1 = Get-Folder -Fixture $F2 -Name $m1
    $mcopy2 = Get-Folder -Fixture $F2 -Name $m2
    $null = New-Copy -Fixture $F2 -Name $m1

    Test-Case 'merge: refused from the main copy (AC-W14)' {
        $before = Get-State -Fixture $F2
        $r = Invoke-Script -Fixture $F2 -Script 'Merge-Worktree.ps1'
        Assert-Refused $r $MsgMergeFromMain $before $F2
    }

    Test-Case 'merge: refused when the branch brings nothing new' {
        $before = Get-State -Fixture $F2
        $r = Merge-Copy -Fixture $F2 -Name $m1
        Assert-Refused $r (Get-MsgNothingNew $m1 'main') $before $F2
    }

    Test-Case 'merge: refused with uncommitted changes (AC-W8)' {
        Add-Commit -Path $mcopy1 -File 'm1.txt'
        [System.IO.File]::WriteAllText((Join-Path $mcopy1 'README.md'), "changed`n")
        [System.IO.File]::WriteAllText((Join-Path $mcopy1 'dirty.txt'), "dirty`n")
        $before = Get-State -Fixture $F2
        $r = Merge-Copy -Fixture $F2 -Name $m1
        Assert-Refused $r (Get-MsgDirty 'README.md, dirty.txt') $before $F2
        $null = Invoke-GitIn -Path $mcopy1 -Arguments @('restore', 'README.md')
        Remove-Item -LiteralPath (Join-Path $mcopy1 'dirty.txt')
    }

    Test-Case 'merge: refused with exit 3 when the base has commits the branch lacks (AC-W8)' {
        Add-Commit -Path $F2.Main -File 'base1.txt'
        $before = Get-State -Fixture $F2
        $r = Merge-Copy -Fixture $F2 -Name $m1
        Assert-Refused $r (Get-MsgBaseAhead 'main' 1) $before $F2 -Code 3
        $null = Invoke-GitIn -Path $mcopy1 -Arguments @('rebase', '-q', 'main')
    }

    Test-Case 'merge: refused when the base is open in a copy with uncommitted changes (AC-W8)' {
        [System.IO.File]::WriteAllText((Join-Path $F2.Main 'README.md'), "changed in main`n")
        $before = Get-State -Fixture $F2
        $r = Merge-Copy -Fixture $F2 -Name $m1
        Assert-Refused $r (Get-MsgBaseOpenDirty 'main' $F2.Main) $before $F2
        $null = Invoke-GitIn -Path $F2.Main -Arguments @('restore', 'README.md')
    }

    Test-Case 'merge: the base checked out clean in the main copy advances and that copy is updated (AC-W7)' {
        $head = Get-Sha -Path $mcopy1 -Ref 'HEAD'
        $short = @(Invoke-GitIn -Path $mcopy1 -Arguments @('rev-parse', '--short', 'HEAD'))[0]
        $r = Merge-Copy -Fixture $F2 -Name $m1
        Assert-Exit $r 0
        Assert-Line $r (Get-MsgMerged 'main' $m1 $short)
        Assert-Equal $head (Get-Sha -Path $F2.Main -Ref 'refs/heads/main') 'main tip'
        Assert-Equal $head (Get-Sha -Path $F2.Main -Ref 'HEAD') 'main copy HEAD'
        Assert-True (Test-Path -LiteralPath (Join-Path $F2.Main 'm1.txt')) 'the main copy files were not updated'
        Assert-Equal 0 @(Invoke-GitIn -Path $F2.Main -Arguments @('status', '--porcelain')).Count 'main copy status'
        Assert-True (Test-Path -LiteralPath $mcopy1) 'the copy was not kept'
        Assert-Equal $head (Get-Sha -Path $F2.Main -Ref "refs/heads/feat/$m1") 'the branch was not kept'
        Assert-Equal '0' @(Invoke-GitIn -Path $F2.Main -Arguments @('rev-list', '--count', '--merges', 'main'))[0] 'merge commits on main'
    }

    Test-Case 'merge: a base not checked out anywhere moves alone (W9)' {
        $null = Invoke-Script -Fixture $F2 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'ensayo')
        $mainTip = Get-Sha -Path $F2.Main -Ref 'refs/heads/main'
        $null = New-Copy -Fixture $F2 -Name $m2 -Base 'ensayo'
        Add-Commit -Path $mcopy2 -File 'm2.txt'
        $head = Get-Sha -Path $mcopy2 -Ref 'HEAD'
        $short = @(Invoke-GitIn -Path $mcopy2 -Arguments @('rev-parse', '--short', 'HEAD'))[0]
        $r = Merge-Copy -Fixture $F2 -Name $m2
        Assert-Exit $r 0
        Assert-Line $r (Get-MsgMerged 'ensayo' $m2 $short)
        Assert-Equal $head (Get-Sha -Path $F2.Main -Ref 'refs/heads/ensayo') 'ensayo tip'
        Assert-Equal $mainTip (Get-Sha -Path $F2.Main -Ref 'refs/heads/main') 'main tip'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $F2.Main 'm2.txt'))) 'the main copy changed'
    }

    Test-Case 'remove: a merged and clean copy goes; Engram project and Claude memory stay (AC-W10)' {
        New-EngramProject -Project "muninndb-$m1"
        $claude = New-ClaudeMemory -Fixture $F2 -Folder $mcopy1
        $mainTip = Get-Sha -Path $F2.Main -Ref 'refs/heads/main'
        $r = Remove-Copy -Fixture $F2 -Name $m1
        Assert-Exit $r 0
        Assert-True (-not (Test-Path -LiteralPath $mcopy1)) 'folder left'
        Assert-True (-not (Test-Ref -Path $F2.Main -Ref "refs/heads/feat/$m1")) 'branch left'
        $registered = @(Invoke-GitIn -Path $F2.Main -Arguments @('worktree', 'list', '--porcelain')) -join "`n"
        Assert-True ($registered -notmatch [regex]::Escape("feat/$m1")) 'still registered'
        Assert-Equal 0 @(Invoke-GitIn -Path $F2.Main -Arguments @('config', '--get', "branch.feat/$m1.worktreebase") -AllowFailure).Count 'recorded base left'
        Assert-True (@(Get-EngramProjectList) -contains "muninndb-$m1") 'the Engram project was deleted'
        Assert-True (Test-Path -LiteralPath (Join-Path $claude 'memory\MEMORY.md')) 'the Claude memory was deleted'
        Assert-Equal $mainTip (Get-Sha -Path $F2.Main -Ref 'refs/heads/main') 'main tip'
    }

    # =========================== Remove and discard ===========================
    $F3 = New-Fixture -Label 'remove'
    $r1 = "$P-r1"
    $r2 = "$P-r2"
    $rcopy1 = Get-Folder -Fixture $F3 -Name $r1
    $rcopy2 = Get-Folder -Fixture $F3 -Name $r2
    $null = New-Copy -Fixture $F3 -Name $r1
    $null = New-Copy -Fixture $F3 -Name $r2
    Add-Commit -Path $rcopy1 -File 'r1.txt'

    Test-Case 'remove: an unmerged branch is refused without discarding (AC-W11)' {
        $before = Get-State -Fixture $F3
        $r = Remove-Copy -Fixture $F3 -Name $r1
        Assert-Refused $r (Get-MsgUnmerged $r1 1 'main') $before $F3
    }

    Test-Case 'remove: uncommitted changes are refused, also when discarding' {
        [System.IO.File]::WriteAllText((Join-Path $rcopy1 'lost.txt'), "work`n")
        [System.IO.File]::WriteAllText((Join-Path $rcopy2 'lost2.txt'), "work`n")
        $before = Get-State -Fixture $F3
        $r = Remove-Copy -Fixture $F3 -Name $r1 -Discard
        Assert-Refused $r (Get-MsgWouldLose $r1 'lost.txt') $before $F3
        $r = Remove-Copy -Fixture $F3 -Name $r2
        Assert-Refused $r (Get-MsgWouldLose $r2 'lost2.txt') $before $F3
        Remove-Item -LiteralPath (Join-Path $rcopy1 'lost.txt'), (Join-Path $rcopy2 'lost2.txt')
    }

    Test-Case 'discard: refused when the tag exp/<n> already exists' {
        $null = Invoke-GitIn -Path $F3.Main -Arguments @('tag', "exp/$r1", 'main')
        $before = Get-State -Fixture $F3
        $r = Remove-Copy -Fixture $F3 -Name $r1 -Discard
        Assert-Refused $r (Get-MsgTagExists $r1) $before $F3
        $null = Invoke-GitIn -Path $F3.Main -Arguments @('tag', '-d', "exp/$r1")
    }

    Test-Case 'discard: tags the branch tip, then the copy, branch, Engram project and Claude memory go (AC-W11)' {
        New-EngramProject -Project "muninndb-$r1"
        $claude = New-ClaudeMemory -Fixture $F3 -Folder $rcopy1
        $tip = Get-Sha -Path $F3.Main -Ref "refs/heads/feat/$r1"
        $r = Remove-Copy -Fixture $F3 -Name $r1 -Discard
        Assert-Exit $r 0
        Assert-Equal $tip (Get-Sha -Path $F3.Main -Ref "refs/tags/exp/$r1") 'exp tag'
        Assert-True (-not (Test-Path -LiteralPath $rcopy1)) 'folder left'
        Assert-True (-not (Test-Ref -Path $F3.Main -Ref "refs/heads/feat/$r1")) 'branch left'
        Assert-True (@(Get-EngramProjectList) -notcontains "muninndb-$r1") 'Engram project left'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $claude 'memory'))) 'Claude memory left'
        Assert-True (Test-Path -LiteralPath (Join-Path $claude 'session.jsonl')) 'the session history was deleted'
    }

    Test-Case 'remove: refused from inside the copy being removed' {
        $before = Get-State -Fixture $F3
        $r = Remove-Copy -Fixture $F3 -Name $r2 -Cwd $rcopy2
        Assert-Refused $r (Get-MsgSessionInside $rcopy2) $before $F3
        $r = Remove-Copy -Fixture $F3 -Name $r2 -Cwd $rcopy2 -Scripts (Join-Path $rcopy2 'scripts\worktree')
        Assert-Refused $r (Get-MsgSessionInside $rcopy2) $before $F3
    }

    Test-Case 'remove: a file in use stops it without closing anything; running it again completes it' {
        $locked = Join-Path $rcopy2 'README.md'
        $handle = [System.IO.File]::Open($locked, 'Open', 'Read', 'Read')
        try {
            $r = Remove-Copy -Fixture $F3 -Name $r2
            Assert-Exit $r 1
            Assert-Line $r (Get-MsgInUse $locked)
            Assert-True (Test-Ref -Path $F3.Main -Ref "refs/heads/feat/$r2") 'the branch went before the folder'
        }
        finally { $handle.Close() }
        $r = Remove-Copy -Fixture $F3 -Name $r2
        Assert-Exit $r 0
        Assert-True (-not (Test-Path -LiteralPath $rcopy2)) 'folder left'
        Assert-True (-not (Test-Ref -Path $F3.Main -Ref "refs/heads/feat/$r2")) 'branch left'
        $registered = @(Invoke-GitIn -Path $F3.Main -Arguments @('worktree', 'list', '--porcelain')) -join "`n"
        Assert-True ($registered -notmatch [regex]::Escape("muninndb-$r2")) 'still registered'
    }

    Test-Case 'remove: repeating it when nothing is left changes nothing' {
        $before = Get-State -Fixture $F3
        $r = Remove-Copy -Fixture $F3 -Name $r2
        Assert-Refused $r "No existe '$r2': ni copia, ni carpeta, ni rama. No se ha eliminado nada." $before $F3 -Code 0
    }

    Test-Case 'remove: a folder without .git that is not what a removal left is never deleted' {
        $name = "$P-foreign"
        $folder = Get-Folder -Fixture $F3 -Name $name
        New-Item -ItemType Directory -Path $folder -Force | Out-Null
        [System.IO.File]::WriteAllText((Join-Path $folder 'notes.txt'), "mine`n")
        try {
            $before = Get-State -Fixture $F3
            $r = Remove-Copy -Fixture $F3 -Name $name
            Assert-Refused $r "La carpeta '$folder' no es una copia de este repositorio. No se ha eliminado nada." $before $F3
            Assert-True (Test-Path -LiteralPath (Join-Path $folder 'notes.txt')) 'a foreign file was deleted'
        }
        finally { Remove-Item -LiteralPath $folder -Recurse -Force -ErrorAction SilentlyContinue }
    }

    # =========================== Rehearsal ===========================
    $F4 = New-Fixture -Label 'rehearsal'
    $prd = "$P-prd"
    $vibe = "$P-vibe"
    $odd = "$P-odd"
    $mainStart = Get-Sha -Path $F4.Main -Ref 'refs/heads/main'

    Test-Case 'rehearsal discard: refuses main and a branch that is not a rehearsal base' {
        $null = Invoke-GitIn -Path $F4.Main -Arguments @('branch', 'other', 'main')
        try {
            $before = Get-State -Fixture $F4
            $r = Remove-RehearsalOf -Fixture $F4 -Base 'main'
            Assert-Refused $r (Get-MsgNotRehearsal 'main') $before $F4
            $r = Remove-RehearsalOf -Fixture $F4 -Base 'other'
            Assert-Refused $r (Get-MsgNotRehearsal 'other') $before $F4
        }
        finally { $null = Invoke-GitIn -Path $F4.Main -Arguments @('branch', '-D', 'other') }
    }

    Test-Case 'rehearsal: the full flow runs with the same scripts on the rehearsal base' {
        $r = Invoke-Script -Fixture $F4 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'ensayo')
        Assert-Exit $r 0
        Assert-Exit (New-Copy -Fixture $F4 -Name $prd -Base 'ensayo') 0
        Add-Commit -Path (Get-Folder -Fixture $F4 -Name $prd) -File 'prd.md'
        Assert-Exit (Merge-Copy -Fixture $F4 -Name $prd) 0
        New-EngramProject -Project "muninndb-$prd"
        $null = New-ClaudeMemory -Fixture $F4 -Folder (Get-Folder -Fixture $F4 -Name $prd)
        Assert-Exit (Remove-Copy -Fixture $F4 -Name $prd) 0
        foreach ($name in @($vibe, $odd)) {
            Assert-Exit (New-Copy -Fixture $F4 -Name $name -Base 'ensayo') 0
            Add-Commit -Path (Get-Folder -Fixture $F4 -Name $name) -File "$name.txt"
            New-EngramProject -Project "muninndb-$name"
            $null = New-ClaudeMemory -Fixture $F4 -Folder (Get-Folder -Fixture $F4 -Name $name)
        }
        Assert-Exit (Merge-Copy -Fixture $F4 -Name $odd) 0
        Assert-Exit (Remove-Copy -Fixture $F4 -Name $vibe -Discard) 0
        Assert-True (Test-Ref -Path $F4.Main -Ref "refs/tags/exp/$vibe") 'exp tag of the discarded copy'
        Assert-Equal $mainStart (Get-Sha -Path $F4.Main -Ref 'refs/heads/main') 'main tip'
    }

    Test-Case 'rehearsal discard: refused while a copy has uncommitted changes, listing it' {
        $oddFolder = Get-Folder -Fixture $F4 -Name $odd
        [System.IO.File]::WriteAllText((Join-Path $oddFolder 'wip.txt'), "wip`n")
        $before = Get-State -Fixture $F4
        $r = Remove-RehearsalOf -Fixture $F4 -Base 'ensayo'
        Assert-Refused $r (Get-MsgWouldLose $odd 'wip.txt') $before $F4
        Remove-Item -LiteralPath (Join-Path $oddFolder 'wip.txt')
    }

    Test-Case 'rehearsal discard: refused while the base is checked out in the main copy' {
        $null = Invoke-GitIn -Path $F4.Main -Arguments @('switch', '-q', 'ensayo')
        try {
            $before = Get-State -Fixture $F4
            $r = Remove-RehearsalOf -Fixture $F4 -Base 'ensayo'
            Assert-Refused $r "La base 'ensayo' está abierta en '$($F4.Main)'. Cambia allí a otra rama y repite. No se ha cambiado nada." $before $F4
        }
        finally { $null = Invoke-GitIn -Path $F4.Main -Arguments @('switch', '-q', 'main') }
    }

    Test-Case 'rehearsal discard: no copy, branch, exp/ tag, Engram project, Claude memory or base is left; main is unchanged (AC-W12)' {
        $r = Remove-RehearsalOf -Fixture $F4 -Base 'ensayo'
        Assert-Exit $r 0
        $worktrees = @(Invoke-GitIn -Path $F4.Main -Arguments @('worktree', 'list', '--porcelain') | Where-Object { $_ -like 'worktree *' })
        Assert-Equal 1 $worktrees.Count 'registered copies (only the main one)'
        Assert-Equal 'refs/heads/main' ((Invoke-GitIn -Path $F4.Main -Arguments @('for-each-ref', '--format=%(refname)', 'refs/heads')) -join ',') 'branches'
        Assert-Equal 0 @(Invoke-GitIn -Path $F4.Main -Arguments @('for-each-ref', '--format=%(refname)', 'refs/tags')).Count 'tags'
        $projects = @(Get-EngramProjectList)
        foreach ($name in @($prd, $vibe, $odd)) {
            Assert-True ($projects -notcontains "muninndb-$name") "Engram project of $name left"
            $key = ((Get-Folder -Fixture $F4 -Name $name) -replace '[^A-Za-z0-9]', '-')
            Assert-True (-not (Test-Path -LiteralPath (Join-Path (Join-Path $F4.Claude $key) 'memory'))) "Claude memory of $name left"
            Assert-True (-not (Test-Path -LiteralPath (Get-Folder -Fixture $F4 -Name $name))) "folder of $name left"
        }
        Assert-Equal 0 @(Invoke-GitIn -Path $F4.Main -Arguments @('config', '--get-regexp', '^worktreebase\.ensayo\.') -AllowFailure).Count 'rehearsal config left'
        Assert-Equal $mainStart (Get-Sha -Path $F4.Main -Ref 'refs/heads/main') 'main tip'
        Assert-Equal 0 @(Invoke-GitIn -Path $F4.Main -Arguments @('status', '--porcelain')).Count 'main copy status'
    }

    Test-Case 'rehearsal discard: repeating it changes nothing' {
        $before = Get-State -Fixture $F4
        $r = Remove-RehearsalOf -Fixture $F4 -Base 'ensayo'
        Assert-Exit $r 0
        Assert-Line $r "No queda nada del ensayo 'ensayo'. No se ha cambiado nada."
        Assert-Unchanged -Before $before -Fixture $F4
    }

    Test-Case 'rehearsal discard: a name reused on another base is skipped and kept' {
        $reuse = "$P-reuse"
        Assert-Exit (Invoke-Script -Fixture $F4 -Script 'New-RehearsalBase.ps1' -Arguments @('-Base', 'ensayo2')) 0
        Assert-Exit (New-Copy -Fixture $F4 -Name $reuse -Base 'ensayo2') 0
        Assert-Exit (Remove-Copy -Fixture $F4 -Name $reuse) 0
        Assert-Exit (New-Copy -Fixture $F4 -Name $reuse) 0
        $r = Remove-RehearsalOf -Fixture $F4 -Base 'ensayo2'
        Assert-Exit $r 0
        Assert-Line $r "Se omite '$reuse': su rama feat/$reuse tiene ahora la base 'main'."
        Assert-True (Test-Ref -Path $F4.Main -Ref "refs/heads/feat/$reuse") 'the reused branch was deleted'
        Assert-True (Test-Path -LiteralPath (Get-Folder -Fixture $F4 -Name $reuse)) 'the reused copy was deleted'
        Assert-True (-not (Test-Ref -Path $F4.Main -Ref 'refs/heads/ensayo2')) 'the rehearsal base is left'
        Assert-Exit (Remove-Copy -Fixture $F4 -Name $reuse) 0
    }
}
catch {
    # A scenario's setup failed: the remaining tests of the run did not execute.
    $script:Failures.Add('setup')
    Write-Host "FAIL  setup -- $($_.Exception.Message)"
}
finally {
    # Engram projects of this run, in the throwaway database, then the whole folder.
    foreach ($project in (Get-EngramProjectList)) {
        if ($project -like "*$P*") { $null = Invoke-Engram -Arguments @('delete', 'project', $project, '--hard') }
    }
    if ($OriginalEngramDataDir) { $env:ENGRAM_DATA_DIR = $OriginalEngramDataDir } else { Remove-Item Env:\ENGRAM_DATA_DIR -ErrorAction SilentlyContinue }
    if (-not $Keep) {
        Get-ChildItem -LiteralPath $TestRoot -Recurse -Force -File -ErrorAction SilentlyContinue |
            Where-Object { $_.IsReadOnly } | ForEach-Object { $_.IsReadOnly = $false }
        Remove-Item -LiteralPath $TestRoot -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $TestRoot) { & cmd /c rd /s /q "$TestRoot" 2>$null }
    }
}

Test-Case 'cleanup: no test folder and no Engram project of this run in the real database' {
    if (-not $Keep) { Assert-True (-not (Test-Path -LiteralPath $TestRoot)) "test folder left: $TestRoot" }
    $leftovers = @(Get-EngramProjectList | Where-Object { $_ -like "*$P*" })
    Assert-Equal 0 $leftovers.Count "Engram projects left: $($leftovers -join ', ')"
}

Write-Host ''
Write-Host "$($script:Passed) passed, $($script:Failures.Count) failed."
if ($script:Failures.Count -gt 0) { exit 1 }
exit 0
