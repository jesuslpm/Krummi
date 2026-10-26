# Pester 3.4 tests: pwsh -NoProfile -Command "Invoke-Pester .claude/skills/codex-review/scripts/tests"
$script:Wrapper = Join-Path $PSScriptRoot '..\Invoke-CodexReview.ps1'
$script:Fake = Join-Path $PSScriptRoot 'fake-codex.cmd'
$script:Date = Get-Date -Format 'yyyy-MM-dd'

function New-TestRepo {
    $root = Join-Path ([IO.Path]::GetTempPath()) ("codex-review-test-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $root, "$root/.codex", "$root/docs/prd", "$root/docs/reviews/feat" | Out-Null
    Set-Content "$root/.codex/config.toml" 'model = "m1"'
    Set-Content "$root/.gitignore" "/target/`n.claude/settings.local.json`n.atl/"
    New-Item -ItemType Directory -Path "$root/.atl" | Out-Null
    Set-Content "$root/.atl/skill-registry.md" 'registry'
    Set-Content "$root/a.txt" 'base'
    Set-Content "$root/docs/prd/x.prd.md" '# PRD'
    git -C $root init -q -b main
    git -C $root -c user.name=t -c user.email=t@t add -A
    git -C $root -c user.name=t -c user.email=t@t commit -q -m init
    $env:CODEX_HOME = "$root-codex-home"
    $env:FAKE_LOG = Join-Path ([IO.Path]::GetTempPath()) ("fake-codex-" + [guid]::NewGuid().ToString('N') + '.log')
    $root
}

function Invoke-Review([string]$Mode, [string[]]$Arguments) {
    $env:FAKE_CODEX_MODE = $Mode
    $out = & pwsh -NoProfile -File $script:Wrapper @Arguments -CodexPath $script:Fake 2>&1
    $exit = $LASTEXITCODE
    $json = ($out | Where-Object { $_ -isnot [Management.Automation.ErrorRecord] }) -join "`n"
    $result = $json | ConvertFrom-Json
    if ($result.logDir) { $script:LogDirs += $result.logDir }
    [pscustomobject]@{ Exit = $exit; Result = $result; Raw = $json }
}

function Get-FakeLog { if (Test-Path $env:FAKE_LOG) { Get-Content -Raw $env:FAKE_LOG } else { '' } }

Describe 'Invoke-CodexReview' {
    BeforeEach {
        $script:Repo = New-TestRepo
        $script:LogDirs = @()
        Remove-Item Env:FAKE_MODEL, Env:FAKE_CODEX_VERSION -ErrorAction SilentlyContinue
    }
    AfterEach {
        Remove-Item -Recurse -Force (@($script:Repo, $env:CODEX_HOME) + $script:LogDirs) -ErrorAction SilentlyContinue
        Remove-Item -Force $env:FAKE_LOG -ErrorAction SilentlyContinue
    }

    It 'runs a PRD review with the plain request, saves the report path and reads the real settings' {
        $r = Invoke-Review 'approved' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Exit | Should Be 0
        $r.Result.status | Should Be 'approved'
        $r.Result.report | Should Be "docs/reviews/feat/$script:Date-prd.md"
        Test-Path (Join-Path $script:Repo $r.Result.report) | Should Be $true
        $r.Result.openQuestions[0] | Should Be 'Q1?'
        $log = Get-FakeLog
        $log | Should Match '(?m)^Revisa el PRD docs/prd/x\.prd\.md\.$'
        $log | Should Match '-s workspace-write'
        $log | Should Match '--output-schema'
        $r.Result.telemetry.coordinator.model | Should Be 'm1'
        $r.Result.telemetry.coordinator.effort | Should Be 'high'
        $r.Result.telemetry.reviewers[0].role | Should Be 'krummi-review'
        $r.Result.telemetry.reviewers[0].model | Should Be 'm2'
        $r.Result.telemetry.reviewers[0].effort | Should Be 'xhigh'
    }

    It 'never overwrites an earlier report: the next one is -02' {
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/$script:Date-prd.md") 'original'
        $r = Invoke-Review 'approved' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.report | Should Be "docs/reviews/feat/$script:Date-prd-02.md"
        (Get-Content (Join-Path $script:Repo "docs/reviews/feat/$script:Date-prd.md")) | Should Be 'original'
    }

    It 'runs a code review of a range with both full commit IDs' {
        $head = (git -C $script:Repo rev-parse HEAD)
        $r = Invoke-Review 'approved' @('-Range', 'HEAD..HEAD', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'approved'
        $r.Result.report | Should Be "docs/reviews/feat/$script:Date-code.md"
        Get-FakeLog | Should Match ([regex]::Escape("Revisa el código de HEAD..HEAD ($head..$head)."))
    }

    It 'reports open findings' {
        $r = Invoke-Review 'findings' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Exit | Should Be 0
        $r.Result.status | Should Be 'findings'
        $r.Result.openFindings[0].id | Should Be 'R1'
    }

    It 'treats a failed Codex run as an error, never an approval' {
        $r = Invoke-Review 'fail' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Exit | Should Be 1
        $r.Result.status | Should Be 'error'
        $r.Result.message | Should Match 'model_not_found'
    }

    It 'treats an invalid final message as an error' {
        $r = Invoke-Review 'nojson' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'error'
    }

    It 'treats a missing report as an error' {
        $r = Invoke-Review 'noreport' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'error'
        $r.Result.message | Should Match 'did not save the report'
    }

    It 'treats a timeout as an error and returns without waiting for the hung process' {
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $r = Invoke-Review 'hang' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo, '-TimeoutMinutes', '0.05')
        $clock.Stop()
        $r.Result.status | Should Be 'error'
        $r.Result.message | Should Match 'timed out'
        $clock.Elapsed.TotalSeconds | Should BeLessThan 20
    }

    It 'treats a final message with a non-boolean approved as an error, never an approval' {
        $r = Invoke-Review 'stringapproved' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'error'
        $r.Result.message | Should Match 'no valid final result'
    }

    It 'reports a change to a tracked file as a violation and leaves it in place' {
        $r = Invoke-Review 'violate' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Exit | Should Be 2
        $r.Result.status | Should Be 'violation'
        $r.Result.unexpectedChanges | Should Be 'a.txt'
        (Get-Content (Join-Path $script:Repo 'a.txt'))[-1] | Should Be 'edited'
    }

    It 'reports a new commit as a violation' {
        $r = Invoke-Review 'commit' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'violation'
        ($r.Result.unexpectedChanges -join ',') | Should Match 'HEAD'
    }

    It 'reports ignored files that configure Claude as a violation' {
        $r = Invoke-Review 'agentconfig' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'violation'
        ($r.Result.unexpectedChanges -join ',') | Should Be '.atl/skill-registry.md,.claude/settings.local.json'
    }

    It 'allows ignored build artifacts and keeps changes that were there before' {
        Set-Content (Join-Path $script:Repo 'a.txt') 'dirty before the review'
        Set-Content (Join-Path $script:Repo 'notes.txt') 'untracked before the review'
        $r = Invoke-Review 'ignored' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'approved'
    }

    It 'refuses to run when Codex does not load the project config' {
        $env:FAKE_MODEL = 'gpt-5.5'
        $r = Invoke-Review 'approved' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'error'
        $r.Result.message | Should Match 'does not load the project config'
        Get-FakeLog | Should Be ''
    }

    It 'refuses an old Codex CLI' {
        $env:FAKE_CODEX_VERSION = '0.138.0'
        $r = Invoke-Review 'approved' @('-Prd', 'docs/prd/x.prd.md', '-Feature', 'feat', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'error'
        $r.Result.message | Should Match '0\.163\.0 or later'
    }

    It 'limits a code follow-up to pending findings and the fix commits' {
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-code.md") 'report'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-code-resolutions.md") 'resolutions'
        $r = Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-code.md', '-FixRange', 'HEAD..HEAD', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'approved'
        $r.Result.feature | Should Be 'feat'
        $r.Result.resolutions | Should Be 'docs/reviews/feat/2026-10-01-code-resolutions.md'
        $log = Get-FakeLog
        $log | Should Match 'Claude lo ha corregido: docs/reviews/feat/2026-10-01-code-resolutions\.md'
        $log | Should Match 'verifica solo los hallazgos que siguen pendientes'
        $log | Should Match 'Correcciones en HEAD\.\.HEAD'
    }

    It 'accepts a numbered original report that has its own resolutions' {
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd.md") 'first chain'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd-02.md") 'second chain'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd-02-resolutions.md") 'resolutions'
        $r = Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-prd-02.md', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'approved'
        $r.Result.kind | Should Be 'prd'
        $r.Result.resolutions | Should Be 'docs/reviews/feat/2026-10-01-prd-02-resolutions.md'
        Get-FakeLog | Should Match 'Revisión de seguimiento de docs/reviews/feat/2026-10-01-prd-02\.md'
    }

    It 'accepts every numbered original the allocator produces, including three digits' {
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd-100.md") 'chain 100'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd-100-resolutions.md") 'resolutions'
        $r = Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-prd-100.md', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'approved'
        $r.Result.resolutions | Should Be 'docs/reviews/feat/2026-10-01-prd-100-resolutions.md'
    }

    It 'passes -FixRange on a PRD follow-up to Codex and rejects an invalid one' {
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd.md") 'report'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-prd-resolutions.md") 'resolutions'
        $r = Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-prd.md', '-FixRange', 'HEAD..HEAD', '-Repo', $script:Repo)
        $r.Result.status | Should Be 'approved'
        Get-FakeLog | Should Match 'Correcciones en HEAD\.\.HEAD'
        Remove-Item -Force $env:FAKE_LOG
        $bad = Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-prd.md', '-FixRange', 'nope..HEAD', '-Repo', $script:Repo)
        $bad.Result.status | Should Be 'error'
        Get-FakeLog | Should Be ''
    }

    It 'refuses a follow-up without resolutions, without fix range, or of a follow-up report' {
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-code.md") 'report'
        (Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-code.md', '-FixRange', 'HEAD..HEAD', '-Repo', $script:Repo)).Result.message |
            Should Match 'No resolutions to verify'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-code-resolutions.md") 'resolutions'
        (Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-code.md', '-Repo', $script:Repo)).Result.message |
            Should Match 'needs -FixRange'
        Set-Content (Join-Path $script:Repo "docs/reviews/feat/2026-10-01-code-02.md") 'follow-up'
        (Invoke-Review 'approved' @('-FollowUp', 'docs/reviews/feat/2026-10-01-code-02.md', '-Repo', $script:Repo)).Result.message |
            Should Match 'No resolutions to verify'
        Get-FakeLog | Should Be ''
    }
}
