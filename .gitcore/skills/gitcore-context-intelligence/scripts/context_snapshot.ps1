param(
    [string]$RepoPath = ".",
    [string]$SnapshotOut = ".gitcore/reports/context_snapshot.md",
    [string]$StatusOut = ".gitcore/reports/project_status.md"
)

$ErrorActionPreference = "Stop"

function Try-Run {
    param(
        [string]$Cmd,
        [switch]$AllowFail
    )
    try {
        return (Invoke-Expression $Cmd) | Out-String
    } catch {
        if ($AllowFail) {
            return "[unavailable] $Cmd :: $($_.Exception.Message)"
        }
        throw
    }
}

function Get-ChangedFiles {
    $lines = (git status --porcelain=v1) 2>$null
    $files = @()
    foreach ($line in $lines) {
        if ($line.Length -lt 4) { continue }
        $path = $line.Substring(3).Trim()
        if ($path -match " -> ") {
            $path = ($path -split " -> ")[-1]
        }
        if ($path) { $files += $path }
    }
    return $files
}

function Get-KeywordsFromPaths {
    param([string[]]$Paths)
    $stop = @("src","main","mod","lib","test","tests","docs","file","core","timeline","gestalt")
    $words = @()
    foreach ($p in $Paths) {
        $parts = ($p -split "[/\\._-]") | Where-Object { $_ -and $_.Length -ge 4 }
        foreach ($w in $parts) {
            $lw = $w.ToLowerInvariant()
            if ($stop -notcontains $lw) { $words += $lw }
        }
    }
    return $words | Group-Object | Sort-Object Count -Descending | Select-Object -ExpandProperty Name
}

function Parse-IssueFile {
    param([string]$Path)
    $raw = Get-Content $Path -Raw
    $titleMatch = [regex]::Match($raw, '(?m)^title:\s*"?(.*?)"?\s*$')
    $title = if ($titleMatch.Success) { $titleMatch.Groups[1].Value } else { [IO.Path]::GetFileNameWithoutExtension($Path) }
    $open = ([regex]::Matches($raw, '(?m)^- \[ \]')).Count
    $done = ([regex]::Matches($raw, '(?m)^- \[[xX]\]')).Count
    [pscustomobject]@{
        File = [IO.Path]::GetFileName($Path)
        Title = $title
        OpenTasks = $open
        DoneTasks = $done
        Content = $raw.ToLowerInvariant()
    }
}

function Score-Issue {
    param(
        [string]$Content,
        [string[]]$Keywords,
        [string[]]$ChangedFiles,
        [string]$Branch
    )
    $score = 0
    foreach ($k in $Keywords) {
        if ($k.Length -lt 4) { continue }
        if ($Content.Contains($k)) { $score += 1 }
    }
    if ($Content.Contains("agentic")) { $score += 3 }
    if ($Content.Contains("synapse")) { $score += 3 }
    if ($Content.Contains("repl")) { $score += 2 }
    if ($Content.Contains("config")) { $score += 2 }

    $changed = ($ChangedFiles -join " ").ToLowerInvariant()
    $branchLower = $Branch.ToLowerInvariant()

    $isAgenticIssue = $Content.Contains("agentic") -or $Content.Contains("synapse") -or $Content.Contains("autonomous") -or $Content.Contains("tool system")
    if ($isAgenticIssue) {
        if ($changed.Contains("application/agent") -or $changed.Contains("src/application/agent")) { $score += 8 }
        if ($changed.Contains("ports/outbound/llm_provider.rs") -or $changed.Contains("adapters/llm/")) { $score += 5 }
        if ($changed.Contains("services/dispatcher.rs") -or $changed.Contains("services/memory.rs") -or $changed.Contains("services/task_queue.rs")) { $score += 4 }
        if ($branchLower.Contains("agentic") -or $branchLower.Contains("completion")) { $score += 2 }
    }

    $isReplIssue = $Content.Contains("repl") -or $Content.Contains("stream")
    if ($isReplIssue) {
        if ($changed.Contains("/repl.rs") -or $changed.Contains("src/cli/repl.rs")) { $score += 6 }
    }

    $isConfigIssue = $Content.Contains("config")
    if ($isConfigIssue) {
        if ($changed.Contains("config/default.toml")) { $score += 4 }
    }

    return $score
}

Set-Location $RepoPath

$branch = Try-Run "git rev-parse --abbrev-ref HEAD"
$status = Try-Run "git status --short --branch"
$log = Try-Run "git log --oneline -5"
$diffStat = Try-Run "git diff --stat"
$changedFiles = Get-ChangedFiles
$keywords = Get-KeywordsFromPaths -Paths $changedFiles

$issueFiles = Get-ChildItem ".github/issues" -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne "_TEMPLATE.md" }
$localIssues = @()
foreach ($f in $issueFiles) {
    $localIssues += Parse-IssueFile -Path $f.FullName
}

$scored = @()
$branchValue = $branch.Trim()
foreach ($i in $localIssues) {
    $s = Score-Issue -Content $i.Content -Keywords $keywords -ChangedFiles $changedFiles -Branch $branchValue
    $scored += [pscustomobject]@{
        File = $i.File
        Title = $i.Title
        OpenTasks = $i.OpenTasks
        DoneTasks = $i.DoneTasks
        Score = $s
    }
}
$top = $scored | Sort-Object -Property @{Expression = "Score"; Descending = $true}, @{Expression = "OpenTasks"; Descending = $true} | Select-Object -First 3

$ghAll = Try-Run "gh issue list --state all --limit 200" -AllowFail
$ghMine = Try-Run "gh issue list --assignee ""@me"" --limit 200" -AllowFail

$forbidden = @("TODO.md","TASKS.md","PLANNING.md","NOTES.md")
$forbiddenFound = @()
foreach ($f in $forbidden) {
    $hits = Get-ChildItem -Recurse -File -Filter $f -ErrorAction SilentlyContinue
    if ($hits) { $forbiddenFound += $hits.FullName }
}

Write-Output "# GitCore Context Snapshot"
Write-Output ""
Write-Output "## Likely Active Work"
if ($top -and $top.Count -gt 0) {
    foreach ($t in $top) {
        Write-Output ("- {0} (score={1}, open={2}, done={3}) [{4}]" -f $t.Title, $t.Score, $t.OpenTasks, $t.DoneTasks, $t.File)
    }
} else {
    Write-Output "- No local issue candidates found."
}

Write-Output ""
Write-Output "## Git State"
Write-Output "### Branch"
Write-Output $branch.Trim()
Write-Output ""
Write-Output "### Status"
Write-Output "---"
Write-Output $status.TrimEnd()
Write-Output "---"
Write-Output ""
Write-Output "### Recent Commits"
Write-Output "---"
Write-Output $log.TrimEnd()
Write-Output "---"
Write-Output ""
Write-Output "### Diff Stat"
Write-Output "---"
Write-Output $diffStat.TrimEnd()
Write-Output "---"
Write-Output ""
Write-Output "### Changed Path Keywords"
Write-Output ("- " + (($keywords | Select-Object -First 20) -join ", "))

Write-Output ""
Write-Output "## Local Issues (.github/issues)"
if ($scored.Count -eq 0) {
    Write-Output "- None found."
} else {
    foreach ($s in ($scored | Sort-Object -Property @{Expression = "Score"; Descending = $true})) {
        Write-Output ("- {0} | score={1} | open={2} done={3} | {4}" -f $s.Title, $s.Score, $s.OpenTasks, $s.DoneTasks, $s.File)
    }
}

Write-Output ""
Write-Output "## GitHub Issues (gh)"
Write-Output "### Assigned To Me"
Write-Output "---"
Write-Output $ghMine.TrimEnd()
Write-Output "---"
Write-Output ""
Write-Output "### All Issues"
Write-Output "---"
Write-Output $ghAll.TrimEnd()
Write-Output "---"

Write-Output ""
Write-Output "## Protocol Signals"
if ($forbiddenFound.Count -gt 0) {
    Write-Output "- Forbidden planning files present:"
    foreach ($p in $forbiddenFound) {
        Write-Output ("  - " + $p)
    }
} else {
    Write-Output "- No forbidden planning files detected."
}

$currentWorkstream = if ($top -and $top.Count -gt 0) { $top[0].Title } else { "Unknown (no local issue match)" }
$topFiles = ($changedFiles | Select-Object -First 8) -join ", "
$topIssues = ($top | ForEach-Object { "#$($_.File): $($_.Title) score=$($_.Score)" }) -join "; "
$risk = if ($forbiddenFound.Count -gt 0) { "Protocol drift: forbidden planning files present." } else { "No forbidden planning files found." }
$hasOnlineAgenticIssue = $ghAll.ToLowerInvariant().Contains("transition gestalt core to agentic framework") -or $ghAll.ToLowerInvariant().Contains("agentic")
$nextAction = if ($currentWorkstream.ToLowerInvariant().Contains("agentic") -or $currentWorkstream.ToLowerInvariant().Contains("transition")) {
    if ($hasOnlineAgenticIssue) {
        "Implement remaining checklist: finalize autonomous action mapping and define LLM-to-Tool replacement integration."
    } else {
        "Sync local/online tracking: create or update GitHub issue for synapse-agentic transition and map completed checklist items."
    }
} else {
    "Update issue checklists and align branch work with highest-scoring issue."
}

$statusReport = @"
# Project Status (GitCore)

## Current Workstream
- $currentWorkstream

## Evidence
- Branch: $branchValue
- Changed files (top): $topFiles
- Matching local issues: $topIssues

## Risk / Drift
- $risk

## Next Action
- $nextAction
"@

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $SnapshotOut) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $StatusOut) | Out-Null

if ($SnapshotOut -ne "__internal_skip__") {
    $snapshotSummary = @"
# Context Snapshot Summary

## Likely Active Work
$(
    if ($top -and $top.Count -gt 0) {
        ($top | ForEach-Object { "- $($_.Title) (score=$($_.Score), open=$($_.OpenTasks), done=$($_.DoneTasks))" }) -join "`n"
    } else {
        "- No local issue candidates found."
    }
)

## Git
- Branch: $branchValue
- Changed files count: $($changedFiles.Count)
- Top changed keywords: $(($keywords | Select-Object -First 15) -join ", ")

## GitHub Issues
- Assigned to me: $(if ([string]::IsNullOrWhiteSpace($ghMine)) { "none" } else { "present" })
- All issues fetched: $(if ([string]::IsNullOrWhiteSpace($ghAll)) { "no" } else { "yes" })

## Protocol Signals
$(
    if ($forbiddenFound.Count -gt 0) {
        "- Forbidden planning files present:`n" + (($forbiddenFound | ForEach-Object { "  - $_" }) -join "`n")
    } else {
        "- No forbidden planning files detected."
    }
)
"@
    $snapshotSummary | Set-Content -Path $SnapshotOut -Encoding UTF8
}
if ($StatusOut -ne "__internal_skip__") {
    $statusReport | Set-Content -Path $StatusOut -Encoding UTF8
}
