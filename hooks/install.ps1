#!/usr/bin/env pwsh
# Gestalt Git Hooks Installer
# Run this script to install pre-commit hooks.

$repoRoot = Split-Path -Parent $PSCommandPath
Set-Location $repoRoot

Write-Host "Installing Gestalt pre-commit hooks..."

# Copy hooks
Copy-Item -Path "hooks/pre-commit" -Destination ".git/hooks/pre-commit" -Force

Write-Host "✅ Hooks installed successfully!"
Write-Host ""
Write-Host "To run validation manually:"
Write-Host "  hooks/pre-commit        (PowerShell)"
Write-Host "  cargo fmt --check        (format check)"
Write-Host "  cargo clippy --all       (lint check)"
Write-Host "  cargo check --workspace  (build check)"
