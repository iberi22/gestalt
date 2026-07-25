#!/usr/bin/env pwsh
# Gestalt Release Checklist
# Run before creating a release tag.
# Part of GitCore Protocol.

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $repoRoot

Write-Host "═══════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  🏷️  Gestalt Release Checklist" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════
" -ForegroundColor Cyan

$failed = False

# 1. Branch check
Write-Host "[1/7] Branch: main" -ForegroundColor Yellow
if ((git branch --show-current) -ne "main") {
    Write-Host "  ❌ Must be on 'main' branch to release" -ForegroundColor Red
    $failed = $true
} else {
    Write-Host "  ✅ On main" -ForegroundColor Green
}

# 2. Clean working tree
Write-Host "
[2/7] Working tree status..." -ForegroundColor Yellow
$status = git status --porcelain
if ($status) {
    Write-Host "  ❌ Uncommitted changes:" -ForegroundColor Red
    $status | ForEach-Object { Write-Host "     " -ForegroundColor Gray }
    $failed = $true
} else {
    Write-Host "  ✅ Clean working tree" -ForegroundColor Green
}

# 3. Pull latest
Write-Host "
[3/7] Pulling latest from origin..." -ForegroundColor Yellow
git pull --ff-only origin main 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ Up to date with origin/main" -ForegroundColor Green
} else {
    Write-Host "  ❌ Failed to pull — resolve conflicts first" -ForegroundColor Red
    $failed = $true
}

# 4. Full build
Write-Host "
[4/7] Full release build..." -ForegroundColor Yellow
cargo build --release 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ Release build: PASS" -ForegroundColor Green
} else {
    Write-Host "  ❌ Release build: FAIL" -ForegroundColor Red
    $failed = $true
}

# 5. Tests
Write-Host "
[5/7] Running tests..." -ForegroundColor Yellow
cargo test --workspace 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ Tests: PASS" -ForegroundColor Green
} else {
    Write-Host "  ❌ Tests: FAIL" -ForegroundColor Red
    $failed = $true
}

# 6. CHANGELOG check
Write-Host "
[6/7] CHANGELOG..." -ForegroundColor Yellow
if (Test-Path "CHANGELOG.md") {
    Write-Host "  ✅ CHANGELOG.md exists" -ForegroundColor Green
} else {
    Write-Host "  ❌ CHANGELOG.md missing" -ForegroundColor Red
    $failed = $true
}

# 7. Final summary
Write-Host "
[7/7] Summary..." -ForegroundColor Yellow
if (-not $failed) {
    Write-Host "
═══════════════════════════════════════════" -ForegroundColor Green
    Write-Host "  ✅ ALL CHECKS PASSED — Ready to release!" -ForegroundColor Green
    Write-Host "═══════════════════════════════════════════" -ForegroundColor Green
    Write-Host "
To create a release tag:" -ForegroundColor Cyan
    Write-Host "  git tag -a v<version> -m 'Release v<version>'" -ForegroundColor Gray
    Write-Host "  git push origin v<version>" -ForegroundColor Gray
} else {
    Write-Host "
═══════════════════════════════════════════" -ForegroundColor Red
    Write-Host "  ❌ RELEASE BLOCKED — Fix issues above" -ForegroundColor Red
    Write-Host "═══════════════════════════════════════════" -ForegroundColor Red
    exit 1
}
