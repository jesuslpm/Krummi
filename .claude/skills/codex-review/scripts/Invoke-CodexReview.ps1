#Requires -Version 7
<#
.SYNOPSIS
    Runs one Codex review of a PRD, a commit range or a follow-up, and prints the outcome as JSON. Used by the
    codex-review skill; Codex stays the reviewer and this script never edits the repository.

.DESCRIPTION
    Before the run it checks the Codex version, that Codex loads this repository's .codex/config.toml (it only does
    in trusted paths), and records a fingerprint of the repository (HEAD, refs, index and every non-ignored changed
    or untracked file). Codex then runs `codex exec` in the workspace-write sandbox, so it can run cargo checks and
    save its report at the path this script fixes. Afterwards the only change allowed is that new report; anything
    else is reported as a violation and left in place.

    Status: approved (no finding open in scope), findings, error (Codex failed, timed out, gave no valid result or
    saved no report; never an approval) or violation (repository changed beyond the report). Exit code: 0 for
    approved and findings, 1 for error, 2 for violation. Real model and effort per thread come from Codex's session
    files (telemetry, best effort).

.EXAMPLE
    pwsh -NoProfile -File .claude/skills/codex-review/scripts/Invoke-CodexReview.ps1 -Prd docs/prd/krummi/trees-scan.prd.md -Feature trees-scan

.EXAMPLE
    pwsh -NoProfile -File .claude/skills/codex-review/scripts/Invoke-CodexReview.ps1 -Range main..feat/x -Feature trees-keys

.EXAMPLE
    pwsh -NoProfile -File .claude/skills/codex-review/scripts/Invoke-CodexReview.ps1 -FollowUp docs/reviews/trees-keys/2026-10-26-code.md -FixRange 1a2b3c4..HEAD
#>
[CmdletBinding(DefaultParameterSetName = 'Prd')]
param(
    [Parameter(Mandatory, ParameterSetName = 'Prd')][string]$Prd,
    [Parameter(Mandatory, ParameterSetName = 'Code')][string]$Range,
    [Parameter(Mandatory, ParameterSetName = 'FollowUp')][string]$FollowUp,
    [Parameter(ParameterSetName = 'FollowUp')][string]$FixRange,
    [Parameter(Mandatory, ParameterSetName = 'Prd')]
    [Parameter(Mandatory, ParameterSetName = 'Code')][string]$Feature,
    [string]$Repo = (Get-Location).Path,
    [string]$CodexPath = 'codex',
    [double]$TimeoutMinutes = 90
)

Set-StrictMode -Version 3.0
$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8

$MinCodexVersion = [version]'0.163.0'
$Sandbox = 'workspace-write'
$SchemaPath = (Resolve-Path (Join-Path $PSScriptRoot '..\assets\review-result.schema.json')).Path

$result = [ordered]@{
    status = 'error'; message = ''; kind = $null; feature = $null; sandbox = $Sandbox
    report = $null; resolutions = $null; verdict = $null; openFindings = @(); openQuestions = @()
    unexpectedChanges = @(); threadId = $null; telemetry = $null; codexExitCode = $null; logDir = $null
}

function Complete([string]$Status, [string]$Message) {
    $result.status = $Status
    $result.message = $Message
    $result | ConvertTo-Json -Depth 8
    exit @{ approved = 0; findings = 0; error = 1; violation = 2 }[$Status]
}

function Invoke-Git {
    $output = & git -C $Repo @args 2>&1
    $errors = @($output | Where-Object { $_ -is [Management.Automation.ErrorRecord] })
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed: $($errors -join ' ')" }
    $output | Where-Object { $_ -isnot [Management.Automation.ErrorRecord] }
}

function Get-TextHash([string]$Text) {
    $bytes = [Security.Cryptography.SHA256]::HashData($utf8.GetBytes($Text))
    [Convert]::ToHexString($bytes)
}

function Get-Prop($Object, [string]$Name) {
    if ($null -ne $Object -and $Object.PSObject.Properties[$Name]) { $Object.$Name } else { $null }
}

# Fingerprint of everything a reviewer must not change. Ignored files (target/, test temporaries) are not part of it.
function Get-RepoState {
    $state = [ordered]@{}
    $state['HEAD'] = (Invoke-Git rev-parse HEAD) -join ''
    $state['refs'] = Get-TextHash ((Invoke-Git for-each-ref '--format=%(refname) %(objectname)') -join "`n")
    $state['index'] = Get-TextHash ((Invoke-Git diff --cached --no-ext-diff --binary) -join "`n")
    $tokens = @(((Invoke-Git status --porcelain=v1 -z --untracked-files=all) -join "`n") -split "`0" |
            Where-Object { $_.Trim() })
    for ($i = 0; $i -lt $tokens.Count; $i++) {
        $token = $tokens[$i].TrimStart("`n")
        $code = $token.Substring(0, 2)
        $path = $token.Substring(3)
        if ($code[0] -in 'R', 'C') { $i++ }
        $full = Join-Path $Repo $path
        $hash = if (Test-Path -LiteralPath $full -PathType Leaf) { (Get-FileHash -LiteralPath $full).Hash } else { 'missing' }
        $state["path:$path"] = "$code $hash"
    }
    $state
}

function Compare-RepoState($Before, $After, [string]$AllowedPath) {
    $keys = @($Before.Keys) + @($After.Keys) | Sort-Object -Unique
    foreach ($key in $keys) {
        if ($key -eq "path:$AllowedPath") { continue }
        if ($Before[$key] -ne $After[$key]) { $key -replace '^path:', '' }
    }
}

function Resolve-Range([string]$Text) {
    $parts = $Text -split '\.\.', 2
    if ($parts.Count -ne 2 -or -not $parts[0] -or -not $parts[1]) { throw "Range '$Text' must be <base>..<head>." }
    $parts | ForEach-Object { (Invoke-Git rev-parse --verify --quiet "$_^{commit}") -join '' }
}

function Get-RepoRelative([string]$Path) {
    $full = [IO.Path]::GetFullPath($Path, $Repo)
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "File not found: $Path" }
    [IO.Path]::GetRelativePath($Repo, $full) -replace '\\', '/'
}

function Get-NextReportPath([string]$FeatureName, [string]$Kind) {
    $date = Get-Date -Format 'yyyy-MM-dd'
    for ($n = 1; ; $n++) {
        $name = if ($n -eq 1) { "$date-$Kind.md" } else { "{0}-{1}-{2:D2}.md" -f $date, $Kind, $n }
        $rel = "docs/reviews/$FeatureName/$name"
        if (-not (Test-Path -LiteralPath (Join-Path $Repo $rel))) { return $rel }
    }
}

function Invoke-Codex([string[]]$CodexArgs, [string]$Stdin, [double]$Minutes) {
    $psi = [Diagnostics.ProcessStartInfo]::new($script:CodexExe)
    foreach ($arg in $CodexArgs) { $psi.ArgumentList.Add($arg) }
    $psi.WorkingDirectory = $Repo
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardInputEncoding = $utf8
    $psi.StandardOutputEncoding = $utf8
    $psi.StandardErrorEncoding = $utf8
    $process = [Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if ($Stdin) { $process.StandardInput.Write($Stdin) }
    $process.StandardInput.Close()
    $timedOut = -not $process.WaitForExit([int]($Minutes * 60000))
    $stopped = $true
    if ($timedOut) {
        try { $process.Kill($true) } catch { }
        $stopped = $process.WaitForExit(10000)
    }
    # Only bounded waits: a process that cannot be stopped must not hold back the error result.
    $drained = $stopped -and [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout, $stderr), 30000)
    [pscustomobject]@{
        ExitCode = if ($stopped) { $process.ExitCode } else { $null }
        Stdout = if ($drained) { $stdout.Result } else { '' }
        Stderr = if ($drained) { $stderr.Result } else { '' }
        TimedOut = $timedOut; Stopped = $stopped; ProcessId = $process.Id
    }
}

function Read-ThreadSettings([IO.FileInfo]$File) {
    $context = Select-String -LiteralPath $File.FullName -Pattern '"type":"turn_context"' -List -SimpleMatch
    if (-not $context) { return $null }
    $payload = ($context.Line | ConvertFrom-Json).payload
    [pscustomobject]@{ model = Get-Prop $payload 'model'; effort = Get-Prop $payload 'effort' }
}

# Real settings of the coordinator thread and of the reviewers it spawned, from Codex's session files.
function Get-Telemetry([string]$ThreadId, [datetime]$Since) {
    $codexHome = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME '.codex' }
    $sessions = Join-Path $codexHome 'sessions'
    if (-not $ThreadId -or -not (Test-Path $sessions)) { return $null }
    $files = Get-ChildItem $sessions -Recurse -Filter 'rollout-*.jsonl' | Where-Object { $_.LastWriteTime -ge $Since.AddMinutes(-1) }
    $telemetry = [ordered]@{ coordinator = $null; reviewers = @() }
    foreach ($file in $files) {
        if ($file.Name.EndsWith("$ThreadId.jsonl")) { $telemetry.coordinator = Read-ThreadSettings $file; continue }
        $meta = (Get-Content -LiteralPath $file.FullName -TotalCount 1 | ConvertFrom-Json).payload
        if ((Get-Prop $meta 'parent_thread_id') -ne $ThreadId) { continue }
        $settings = Read-ThreadSettings $file
        $telemetry.reviewers += [ordered]@{
            role = Get-Prop $meta 'agent_role'; nickname = Get-Prop $meta 'agent_nickname'
            model = Get-Prop $settings 'model'; effort = Get-Prop $settings 'effort'
        }
    }
    $telemetry
}

try {
    $Repo = (Invoke-Git rev-parse --show-toplevel) -join ''
    $Repo = [IO.Path]::GetFullPath($Repo)

    # The request: the same short sentence a person types, so the first review gets no framing from Claude.
    switch ($PSCmdlet.ParameterSetName) {
        'Prd' {
            $kind = 'prd'
            $prdRel = Get-RepoRelative $Prd
            $request = "Revisa el PRD $prdRel."
        }
        'Code' {
            $kind = 'code'
            $base, $head = Resolve-Range $Range
            $request = "Revisa el código de $Range ($base..$head)."
        }
        'FollowUp' {
            $originalRel = Get-RepoRelative $FollowUp
            $leaf = Split-Path $originalRel -Leaf
            if ($leaf -notmatch '^\d{4}-\d{2}-\d{2}-(prd|code)(-\d{2})?\.md$') {
                throw "FollowUp must name a review report (<date>-<prd|code>[-NN].md), not '$leaf'."
            }
            $kind = $Matches[1]
            $Feature = Split-Path (Split-Path $originalRel -Parent) -Leaf
            $resolutionsRel = $originalRel -replace '\.md$', '-resolutions.md'
            if (-not (Test-Path -LiteralPath (Join-Path $Repo $resolutionsRel))) {
                throw "No resolutions to verify: $resolutionsRel does not exist. Pass the chain's original report; follow-up reports share its resolutions."
            }
            $result.resolutions = $resolutionsRel
            $request = "Claude lo ha corregido: $resolutionsRel. Revisión de seguimiento de ${originalRel}: verifica solo " +
                "los hallazgos que siguen pendientes y las regresiones causadas por sus correcciones. Las resoluciones " +
                "'dismissed with evidence' también se verifican: confirma o rebate su evidencia."
            if ($kind -eq 'code') {
                if (-not $FixRange) { throw 'A code follow-up needs -FixRange <base>..<head> with the fix commits.' }
                $fixBase, $fixHead = Resolve-Range $FixRange
                $request += " Correcciones en $FixRange ($fixBase..$fixHead)."
            }
        }
    }
    if ($Feature -notmatch '^[a-z0-9][a-z0-9-]*$') { throw "Feature '$Feature' must be a lowercase slug." }
    $result.kind = $kind
    $result.feature = $Feature
    $reportRel = Get-NextReportPath $Feature $kind
    $result.report = $reportRel

    $command = Get-Command $CodexPath -CommandType Application -ErrorAction SilentlyContinue |
        Where-Object { $_.Extension -in '.exe', '.cmd', '.bat' } | Select-Object -First 1
    if (-not $command) { throw "Codex CLI not found: $CodexPath" }
    $script:CodexExe = $command.Source

    $versionRun = Invoke-Codex @('--version') $null 1
    if ($versionRun.Stdout -notmatch '(\d+\.\d+\.\d+)' -or [version]$Matches[1] -lt $MinCodexVersion) {
        throw "Codex CLI $MinCodexVersion or later is required; found '$($versionRun.Stdout.Trim())'. Run: npm install -g @openai/codex"
    }

    # Codex loads .codex/config.toml (model and review agents) only in trusted paths; elsewhere it silently uses the
    # user's model. Refuse to run then, instead of reviewing with the wrong settings.
    $configPath = Join-Path $Repo '.codex/config.toml'
    $projectModel = if (Test-Path $configPath) {
        Select-String -LiteralPath $configPath -Pattern '^\s*model\s*=\s*"([^"]+)"' | Select-Object -First 1 |
            ForEach-Object { $_.Matches[0].Groups[1].Value }
    }
    if (-not $projectModel) { throw "No model in $configPath." }
    $doctor = Invoke-Codex @('doctor', '--json') $null 3
    $loadedModel = $null
    try { $loadedModel = ($doctor.Stdout | ConvertFrom-Json).checks.'config.load'.details.model } catch { }
    if ($loadedModel -ne $projectModel) {
        throw "Codex does not load the project config here (model '$loadedModel', expected '$projectModel'). Trust this folder in Codex ($Repo) and retry."
    }

    $logDir = Join-Path ([IO.Path]::GetTempPath()) ("codex-review/{0}-{1}-{2}" -f (Get-Date -Format 'yyyyMMdd-HHmmss'), $Feature, $kind)
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    $result.logDir = $logDir
    $lastMessagePath = Join-Path $logDir 'last-message.json'

    $prompt = @"
$request

---
Automated invocation through .claude/skills/codex-review/scripts/Invoke-CodexReview.ps1.
- Feature: $Feature. Sandbox: $Sandbox, for checks and for the report only.
- Save the consolidated report at ``$reportRel`` (this request fixes the name). It is the only file you may create
  or change; build artifacts and test temporaries excepted.
- End with a final message that matches the provided JSON schema. ``open_findings`` lists every finding still open
  in the requested scope with its report ID; ``approved`` is true only when that list is empty.
"@
    Set-Content -LiteralPath (Join-Path $logDir 'prompt.txt') -Value $prompt -Encoding utf8NoBOM

    $before = Get-RepoState
    $started = Get-Date
    $codexArgs = @('exec', '--json', '--color', 'never', '-s', $Sandbox, '-C', $Repo,
        '--output-schema', $SchemaPath, '-o', $lastMessagePath, '-')
    $run = Invoke-Codex $codexArgs $prompt $TimeoutMinutes
    $after = Get-RepoState
    Set-Content -LiteralPath (Join-Path $logDir 'events.jsonl') -Value $run.Stdout -Encoding utf8NoBOM
    Set-Content -LiteralPath (Join-Path $logDir 'stderr.log') -Value $run.Stderr -Encoding utf8NoBOM
    $result.codexExitCode = $run.ExitCode

    $completed = $false
    $failures = @()
    foreach ($line in $run.Stdout -split "`n") {
        if (-not $line.Trim()) { continue }
        try { $item = $line | ConvertFrom-Json } catch { continue }
        switch (Get-Prop $item 'type') {
            'thread.started' { $result.threadId = Get-Prop $item 'thread_id' }
            'turn.completed' { $completed = $true }
            'turn.failed' { $failures += (Get-Prop (Get-Prop $item 'error') 'message') }
            'error' { $failures += (Get-Prop $item 'message') }
        }
    }
    try { $result.telemetry = Get-Telemetry $result.threadId $started } catch { $result.telemetry = "unavailable: $($_.Exception.Message)" }

    $problem = $null
    if ($run.TimedOut) {
        $problem = "Codex timed out after $TimeoutMinutes min."
        if (-not $run.Stopped) { $problem += " Process $($run.ProcessId) could not be stopped and may still change files; end it by hand." }
    }
    elseif ($run.ExitCode -ne 0 -or $failures -or -not $completed) {
        $problem = "Codex failed (exit $($run.ExitCode)): $(($failures | Where-Object { $_ }) -join ' | ')"
    }
    $answer = $null
    if (-not $problem) {
        try { $answer = Get-Content -LiteralPath $lastMessagePath -Raw | ConvertFrom-Json } catch { }
        $fields = 'report_path', 'approved', 'verdict', 'open_findings', 'open_questions'
        if (-not $answer -or @($fields | Where-Object { -not $answer.PSObject.Properties[$_] }) -or
            $answer.approved -isnot [bool]) {
            $problem = 'Codex gave no valid final result (see last-message.json).'
        }
        elseif (($answer.report_path -replace '\\', '/') -ne $reportRel) {
            $problem = "Codex named report '$($answer.report_path)' instead of '$reportRel'."
        }
        elseif (-not (Test-Path -LiteralPath (Join-Path $Repo $reportRel) -PathType Leaf)) {
            $problem = "Codex did not save the report at $reportRel."
        }
    }

    $changes = @(Compare-RepoState $before $after $reportRel)
    if ($changes) {
        $result.unexpectedChanges = $changes
        Complete 'violation' ("Repository changed beyond the report; nothing was reverted. $problem").Trim()
    }
    if ($problem) { Complete 'error' $problem }

    $result.verdict = $answer.verdict
    $result.openFindings = @($answer.open_findings)
    $result.openQuestions = @($answer.open_questions)
    if ($answer.approved -and $result.openFindings.Count -eq 0) { Complete 'approved' 'No finding open in scope.' }
    Complete 'findings' ("{0} finding(s) open." -f $result.openFindings.Count)
}
catch {
    Complete 'error' $_.Exception.Message
}
