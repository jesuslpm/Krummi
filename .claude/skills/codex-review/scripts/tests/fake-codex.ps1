# Stand-in for the Codex CLI in Invoke-CodexReview tests. FAKE_CODEX_MODE picks the behavior of `exec`.
$rest = $args
switch ($rest[0]) {
    '--version' { "codex-cli $(if ($env:FAKE_CODEX_VERSION) { $env:FAKE_CODEX_VERSION } else { '0.164.0' })"; exit 0 }
    'doctor' {
        $model = if ($env:FAKE_MODEL) { $env:FAKE_MODEL } else { 'm1' }
        @{ checks = @{ 'config.load' = @{ details = @{ model = $model } } } } | ConvertTo-Json -Depth 9 -Compress
        exit 0
    }
}

$repo = $rest[[array]::IndexOf($rest, '-C') + 1]
$lastMessage = $rest[[array]::IndexOf($rest, '-o') + 1]
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
$prompt = [Console]::In.ReadToEnd()
Set-Content -LiteralPath $env:FAKE_LOG -Value (($rest -join ' ') + "`n" + $prompt) -Encoding utf8NoBOM
$report = [regex]::Match($prompt, 'report at `([^`]+)`').Groups[1].Value
$mode = $env:FAKE_CODEX_MODE
$thread = '01aa0000-0000-7000-8000-000000000001'

function Save-Report { Set-Content -LiteralPath (Join-Path $repo $report) -Value '# Fake report' -Encoding utf8NoBOM }
function Write-Answer($approved, $findings) {
    @{ report_path = $report; approved = $approved; verdict = 'fake verdict'; open_findings = $findings; open_questions = @('Q1?') } |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $lastMessage -Encoding utf8NoBOM
}
function Write-Rollouts {
    $dir = Join-Path $env:CODEX_HOME 'sessions/2026/10/26'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    @(('{"type":"session_meta","payload":{"id":"' + $thread + '"}}'),
        '{"type":"turn_context","payload":{"model":"m1","effort":"high"}}') |
        Set-Content -LiteralPath (Join-Path $dir "rollout-2026-10-26T08-00-00-$thread.jsonl")
    @(('{"type":"session_meta","payload":{"id":"child","parent_thread_id":"' + $thread + '","agent_role":"krummi-review","agent_nickname":"Ada"}}'),
        '{"type":"turn_context","payload":{"model":"m2","effort":"xhigh"}}') |
        Set-Content -LiteralPath (Join-Path $dir 'rollout-2026-10-26T08-00-01-child.jsonl')
}

'{"type":"thread.started","thread_id":"' + $thread + '"}'
'{"type":"turn.started"}'
switch ($mode) {
    'fail' { '{"type":"turn.failed","error":{"message":"model_not_found"}}'; exit 1 }
    'hang' { Start-Sleep -Seconds 30; exit 0 }
}
Write-Rollouts
switch ($mode) {
    'approved' { Save-Report; Write-Answer $true @() }
    'findings' { Save-Report; Write-Answer $false @(@{ id = 'R1'; severity = 'major'; title = 'Broken' }) }
    'stringapproved' {
        Save-Report
        @{ report_path = $report; approved = 'false'; verdict = 'v'; open_findings = @(); open_questions = @() } |
            ConvertTo-Json | Set-Content -LiteralPath $lastMessage -Encoding utf8NoBOM
    }
    'nojson' { Save-Report; Set-Content -LiteralPath $lastMessage -Value 'not json' }
    'noreport' { Write-Answer $true @() }
    'violate' { Save-Report; Add-Content -LiteralPath (Join-Path $repo 'a.txt') -Value 'edited'; Write-Answer $true @() }
    'commit' { Save-Report; git -C $repo -c user.name=t -c user.email=t@t commit -q --allow-empty -m 'sneaky' | Out-Null; Write-Answer $true @() }
    'agentconfig' {
        Save-Report
        New-Item -ItemType Directory -Force -Path (Join-Path $repo '.claude') | Out-Null
        Set-Content -LiteralPath (Join-Path $repo '.claude/settings.local.json') -Value '{}'
        Add-Content -LiteralPath (Join-Path $repo '.atl/skill-registry.md') -Value 'edited'
        Write-Answer $true @()
    }
    'ignored' {
        Save-Report
        New-Item -ItemType Directory -Force -Path (Join-Path $repo 'target') | Out-Null
        Set-Content -LiteralPath (Join-Path $repo 'target/build.bin') -Value 'artifact'
        Write-Answer $true @()
    }
}
'{"type":"turn.completed","usage":{}}'
exit 0
