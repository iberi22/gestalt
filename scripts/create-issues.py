#!/usr/bin/env python3
"""Create 15 GitHub Issues for Gestalt Phase 2 — Merge Inteligente."""
import subprocess, json, time

def gh_issue(title, body, labels="ola1,wave-1"):
    """Create a GitHub issue and return its number."""
    result = subprocess.run(
        ["gh", "issue", "create", "--title", title, "--body", body, "--label", labels],
        capture_output=True, text=True, cwd="/home/belal/proyectosSWAL/gestalt"
    )
    if result.returncode != 0:
        print(f"ERROR: {result.stderr}")
        return None
    # Extract issue number from URL
    url = result.stdout.strip()
    num = url.split('/')[-1]
    print(f"  ✅ #{num}: {title[:60]}...")
    return num

BASE = """# [Ola 1.{n:02d}] {title}

> Wave 1 — Foundation. Epic: gestalt Phase 2 (Merge Inteligente).
> Label: `ola1`, `wave-1`

## Current State
{current}

## Desired State
{desired}

## Web Research Required
{research}

## Exact Technical Context
{context}

## Problem
{problem}

## Acceptance Criteria
{ac}

## Files to Modify
{files}

## DO NOT touch
- gestalt_core/, synapse-agentic/
- gestalt_cli/src/repl.rs (unrelated)
- Any file not listed in 'Files to Modify'
- Note: gestalt_swarm is legacy and excluded from the Cargo workspace — never use cargo -p gestalt_swarm

## Anti-Hallucination Guard
1. Verify file existence with `ls path/to/file` before modifying
2. All paths are relative to repo root
3. Do NOT create .patch, .py, .txt files in repo root
4. Do NOT modify Cargo.lock unless adding new dependencies
5. Verify compilation with `cargo check -p gestalt-merge` after changes

## Verification
```bash
PKG_CONFIG_PATH=/nix/store/5bfcsl07gyqym27g9sfgcfg5mzr8r4s0-openssl-3.4.3-dev/lib/pkgconfig cargo check -p gestalt-merge 2>&1 | tail -5
```

## Dependencies & Merge Order
- Depends on: {depends}
- Parallel with: {parallel}
- Merge order: {order}
- Expected effort: {effort}

## Failure Recovery
| If this happens | Action |
|----------------|--------|
| `cargo check` fails | Fix compilation errors, do NOT commit broken code |
| File already exists | Read existing content, merge changes |
| PR conflicts with parallel work | Rebase on main, re-run verification |
"""

issues = [
    {
        "n": 1,
        "title": "gestalt-merge Cargo dependencies — similar, git2, thiserror, serde, uuid",
        "current": "File: `gestalt-merge/Cargo.toml` (8 lines) — only `thiserror = \"2\"` in deps. No merge engine deps.",
        "desired": "Add deps: `similar = \"2\"` (diff), `git2 = \"0.19\"` (git merges), `serde = {version=\"1\", features=[\"derive\"]}`, `uuid = {version=\"1\", features=[\"v4\"]}`, `serde_json = \"1\"`, `tracing = \"0.1\"`, `tempfile = \"3\"` (dev).",
        "research": "1. search: \"similar crate rust v2 changelog 2026\"\n2. search: \"git2 crate v0.19 rust bindings breaking changes\"\n3. search: \"gestalt-merge/Cargo.toml current content\"",
        "context": "File: `gestalt-merge/Cargo.toml`. Project root is `/home/belal/proyectosSWAL/gestalt`. Workspace members include gestalt_core, gestalt_cli, synapse-agentic, gestalt-router, gestalt-merge.",
        "problem": "The gestalt-merge crate has no dependencies for git operations, diff comparison, or serialization needed for 3-way merge engine.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes (0 errors)\n- [ ] `grep -c \"similar\" gestalt-merge/Cargo.toml` >= 1\n- [ ] `grep -c \"git2\" gestalt-merge/Cargo.toml` >= 1\n- [ ] `grep -c \"serde\" gestalt-merge/Cargo.toml` >= 3",
        "files": "| `gestalt-merge/Cargo.toml` | 8 lines, 1 dep | Add 6+ deps | LOW |",
        "depends": "None",
        "parallel": "#2, #3, #4",
        "order": 1,
        "effort": "Small (<1h)",
    },
    {
        "n": 2,
        "title": "gestalt-merge lib.rs — module declarations, re-exports, crate docs",
        "current": "File: `gestalt-merge/src/lib.rs` (1 line) — only `//! Gestalt Merge — 3-way merge engine. Fase 2: not yet implemented.`",
        "desired": "Full crate doc with module declarations for: `error`, `types`, `engine`, `git_utils`, `git_three_way`, `pr`, `semantic`. Public re-exports of key types (MergeEngine trait, GitThreeWay, MergeOutput, MergeError). Use `pub mod` + `pub use` pattern.",
        "research": "1. search: \"rust crate module structure best practices\"\n2. search: \"gestalt-router/src/lib.rs for reference pattern\"",
        "context": "File: `gestalt-merge/src/lib.rs`. Follow same pattern as `gestalt-router/src/lib.rs` in same workspace. Use `#![doc = include_str!(\"../README.md\")]` pattern if README exists.",
        "problem": "The crate has no module structure, making it impossible to add engine implementations.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"pub mod error\" gestalt-merge/src/lib.rs` >= 1\n- [ ] `grep -c \"pub mod engine\" gestalt-merge/src/lib.rs` >= 1\n- [ ] `grep -c \"pub mod git_three_way\" gestalt-merge/src/lib.rs` >= 1\n- [ ] `grep -c \"pub mod pr\" gestalt-merge/src/lib.rs` >= 1\n- [ ] `grep -c \"pub mod semantic\" gestalt-merge/src/lib.rs` >= 1",
        "files": "| `gestalt-merge/src/lib.rs` | 1 line | Module declarations + re-exports (~40 lines) | LOW |",
        "depends": "#1",
        "parallel": "None (sequential after #1)",
        "order": 2,
        "effort": "Small (<1h)",
    },
    {
        "n": 3,
        "title": "gestalt-merge error.rs — MergeError enum with From impls",
        "current": "File does not exist. Error handling is missing.",
        "desired": "Create `gestalt-merge/src/error.rs` with:\n- `MergeError` enum with variants: `GitError(String)`, `IoError(std::io::Error)`, `ConflictError(String)`, `InternalError(String)`, `PrError(String)`\n- `impl std::fmt::Display` for MergeError\n- `impl std::error::Error` for MergeError\n- `From<git2::Error>` impl\n- `From<std::io::Error>` impl\n- `From<serde_json::Error>` impl",
        "research": "1. search: \"rust thiserror enum variants example\"\n2. search: \"gestalt-router/src/run.rs error pattern for reference\"",
        "context": "File: `gestalt-merge/src/error.rs`. Follow same thiserror pattern as `gestalt-router/src/run.rs` (RouterError enum in same workspace). Use `#[derive(Debug, thiserror::Error)]`.",
        "problem": "No error types exist for gestalt-merge, making it impossible to return typed errors from merge operations.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"MergeError\" gestalt-merge/src/error.rs` >= 1\n- [ ] `grep -c \"impl From<git2::Error>\" gestalt-merge/src/error.rs` >= 1\n- [ ] `grep -c \"impl From<std::io::Error>\" gestalt-merge/src/error.rs` >= 1",
        "files": "| `gestalt-merge/src/error.rs` | Does not exist | Create ~80 lines with MergeError + From impls | LOW |",
        "depends": "#2 (lib.rs must declare `pub mod error`)",
        "parallel": "#4, #5, #6",
        "order": 3,
        "effort": "Small (<1h)",
    },
    {
        "n": 4,
        "title": "gestalt-merge types.rs — MergeOutput, ConflictInfo, DiffRegion, MergeTestResult",
        "current": "File does not exist. No shared types for merge operations.",
        "desired": "Create `gestalt-merge/src/types.rs` with:\n- `MergeOutput { merge_sha: String, merged_branches: Vec<String>, conflicts: Vec<ConflictInfo> }`\n- `ConflictInfo { agent_id: String, path: String, conflict_type: ConflictKind }`\n- `ConflictKind` enum: `Overlap`, `MergeConflict`, `BinaryConflict`\n- `DiffRegion { start_line: usize, end_line: usize, content: String }`\n- `MergeTestResult` enum: `Clean`, `Conflicts(Vec<ConflictInfo>)`\nAll with Serialize/Deserialize derives.",
        "research": "1. search: \"gestalt-router/src/run.rs existing ConflictInfo for compatibility\"\n2. search: \"serde derive Serialize Deserialize on enum with data\"",
        "context": "File: `gestalt-merge/src/types.rs`. Must be compatible with `gestalt-router/src/run.rs` ConflictInfo type for future router integration. Use same field names (agent_id, path).",
        "problem": "No shared types exist for representing merge results, conflicts, or diff regions.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"struct MergeOutput\" gestalt-merge/src/types.rs` >= 1\n- [ ] `grep -c \"struct ConflictInfo\" gestalt-merge/src/types.rs` >= 1\n- [ ] `grep -c \"enum MergeTestResult\" gestalt-merge/src/types.rs` >= 1\n- [ ] `grep -c \"Serialize\" gestalt-merge/src/types.rs` >= 3",
        "files": "| `gestalt-merge/src/types.rs` | Does not exist | Create ~60 lines with data types | LOW |",
        "depends": "#2",
        "parallel": "#3, #5, #6",
        "order": 3,
        "effort": "Small (<1h)",
    },
    {
        "n": 5,
        "title": "gestalt-merge engine.rs — MergeEngine trait with merge() and test_mergeability()",
        "current": "File does not exist. No merge engine interface defined.",
        "desired": "Create `gestalt-merge/src/engine.rs` with:\n- `#[async_trait] pub trait MergeEngine: Send + Sync`\n- `async fn merge(&self, repo_path: &Path, base_sha: &str, ours: &str, theirs: &str) -> Result<MergeOutput, MergeError>`\n- `fn test_mergeability(&self, repo_path: &Path, base_sha: &str, ours: &str, theirs: &str) -> Result<MergeTestResult, MergeError>`\n- `fn name(&self) -> &'static str`\nDocument each method with doc comments.",
        "research": "1. search: \"rust async_trait pattern Send + Sync trait\"\n2. search: \"gestalt-router/src/agent.rs AgentRunner trait for reference pattern\"",
        "context": "File: `gestalt-merge/src/engine.rs`. Import types from `crate::types::{MergeOutput, MergeTestResult}` and error from `crate::error::MergeError`. Follow same async trait pattern as `gestalt-router/src/agent.rs`.",
        "problem": "No trait interface exists for merge engines, preventing polymorphic merge implementations.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"trait MergeEngine\" gestalt-merge/src/engine.rs` >= 1\n- [ ] `grep -c \"async fn merge\" gestalt-merge/src/engine.rs` >= 1\n- [ ] `grep -c \"fn test_mergeability\" gestalt-merge/src/engine.rs` >= 1\n- [ ] `grep -c \"fn name\" gestalt-merge/src/engine.rs` >= 1",
        "files": "| `gestalt-merge/src/engine.rs` | Does not exist | Create ~50 lines with MergeEngine trait | LOW |",
        "depends": "#2, #3, #4",
        "parallel": "None (blocker for impls)",
        "order": 4,
        "effort": "Small (<1h)",
    },
    {
        "n": 6,
        "title": "gestalt-merge git_utils.rs — shared git helpers (run_git_cmd, verify_git)",
        "current": "File does not exist. No shared git helper functions.",
        "desired": "Create `gestalt-merge/src/git_utils.rs` with:\n- `pub fn run_git_cmd(repo_path: &Path, args: &[&str]) -> Result<String, MergeError>` — similar to checkpoint.rs but with MergeError\n- `pub fn run_git_cmd_with_retry(repo_path: &Path, args: &[&str], retries: usize) -> Result<String, MergeError>` — with exponential backoff for lock conflicts\n- `pub fn verify_git() -> Result<(), MergeError>` — check git --version works\n- `pub fn get_repo_root(repo_path: &Path) -> Result<PathBuf, MergeError>` — `git rev-parse --show-toplevel`",
        "research": "1. search: \"gestalt-router/src/worktree.rs run_git_command_locked for retry pattern\"\n2. search: \"std::process::Command rust error handling\"",
        "context": "File: `gestalt-merge/src/git_utils.rs`. Import `MergeError` from `crate::error`. Use same pattern as `gestalt-router/src/checkpoint.rs::run_git_cmd` but with MergeError and retry logic from worktree.rs.",
        "problem": "No shared git utility functions exist in gestalt-merge, forcing each module to duplicate git command execution.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"pub fn run_git_cmd\" gestalt-merge/src/git_utils.rs` >= 1\n- [ ] `grep -c \"pub fn run_git_cmd_with_retry\" gestalt-merge/src/git_utils.rs` >= 1\n- [ ] `grep -c \"pub fn verify_git\" gestalt-merge/src/git_utils.rs` >= 1",
        "files": "| `gestalt-merge/src/git_utils.rs` | Does not exist | Create ~80 lines with git helpers | LOW |",
        "depends": "#2, #3",
        "parallel": "#3, #4, #5",
        "order": 3,
        "effort": "Small (<1h)",
    },
    {
        "n": 7,
        "title": "gestalt-merge git_three_way.rs — GitThreeWay engine (git merge-tree wrapper v1)",
        "current": "File does not exist. No git-based merge implementation.",
        "desired": "Create `gestalt-merge/src/git_three_way.rs` with:\n- `pub struct GitThreeWay` (holds no state, or config for git binary path)\n- `impl GitThreeWay { pub fn new() -> Self }`\n- `#[async_trait] impl MergeEngine for GitThreeWay`:\n  - `merge()`: Use `git merge-tree --write-tree` with sequential intermediate commits (same pattern as gestalt-router/src/integrate.rs)\n  - `test_mergeability()`: Use `git merge-tree` in dry-run mode to detect conflicts\n  - `name()`: Returns `\"git-three-way\"`\n- Handle binary file detection via `git diff --numstat`\n- Handle merge-tree stdout parsing for tree SHA extraction",
        "research": "1. search: \"git merge-tree --write-tree usage example v2.38+\"\n2. search: \"gestalt-router/src/integrate.rs::integrate_branches for intermediate commit pattern\"\n3. search: \"git merge-tree exit code merge conflict detection\"",
        "context": "File: `gestalt-merge/src/git_three_way.rs`. Use `crate::engine::MergeEngine`, `crate::types::{MergeOutput, ConflictInfo, MergeTestResult}`, `crate::error::MergeError`, `crate::git_utils::run_git_cmd`. Follow the same intermediate commit pattern as `gestalt-router/src/integrate.rs` (commit-tree after each merge).",
        "problem": "No working git merge implementation exists. The integrate.rs in gestalt-router has the intermediate commit pattern but it's not exposed as a reusable engine.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"impl MergeEngine for GitThreeWay\" gestalt-merge/src/git_three_way.rs` >= 1\n- [ ] `grep -c \"fn merge\" gestalt-merge/src/git_three_way.rs` >= 1\n- [ ] `grep -c \"fn test_mergeability\" gestalt-merge/src/git_three_way.rs` >= 1\n- [ ] `grep -c \"crate::engine::MergeEngine\" gestalt-merge/src/git_three_way.rs` >= 1",
        "files": "| `gestalt-merge/src/git_three_way.rs` | Does not exist | Create ~150 lines with GitThreeWay impl | MEDIUM |",
        "depends": "#5 (MergeEngine trait), #6 (git_utils)",
        "parallel": "#8, #9",
        "order": 5,
        "effort": "Medium (1-4h)",
    },
    {
        "n": 8,
        "title": "gestalt-merge pr.rs — GitHub PR creation via gh CLI",
        "current": "File does not exist. No PR creation capability.",
        "desired": "Create `gestalt-merge/src/pr.rs` with:\n- `pub struct PrManager { repo: String, gh_path: String }`\n- `impl PrManager { pub fn new(repo: String) -> Self }`\n- `pub async fn create_pr(&self, branch: &str, title: &str, body: &str, base: &str) -> Result<String, MergeError>` — runs `gh pr create --base {base} --head {branch} --title {title} --body {body}` and returns URL\n- `pub async fn create_draft_pr(&self, ...)` — adds `--draft` flag\n- `pub async fn list_prs(&self, state: &str) -> Result<Vec<PrInfo>, MergeError>` — `gh pr list --state {state} --json number,title,url`\n- `PrInfo` struct with number, title, url fields\n- Handle gh CLI not found error gracefully",
        "research": "1. search: \"gh pr create --json fields 2026\"\n2. search: \"std::process::Command tokio async\"\n3. search: \"gh CLI exit codes and error handling\"",
        "context": "File: `gestalt-merge/src/pr.rs`. Use tokio::process::Command for async execution. gh binary is at standard PATH location. The gh auth is already configured (gh auth status shows logged in as iberi22).",
        "problem": "No automated PR creation capability exists. Merge results must be pushed and PR'd manually.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"struct PrManager\" gestalt-merge/src/pr.rs` >= 1\n- [ ] `grep -c \"async fn create_pr\" gestalt-merge/src/pr.rs` >= 1\n- [ ] `grep -c \"fn new\" gestalt-merge/src/pr.rs` >= 1\n- [ ] `grep -c \"gh pr create\" gestalt-merge/src/pr.rs` >= 1",
        "files": "| `gestalt-merge/src/pr.rs` | Does not exist | Create ~120 lines with PR Manager | MEDIUM |",
        "depends": "#2, #3, #4",
        "parallel": "#7, #9",
        "order": 5,
        "effort": "Medium (1-4h)",
    },
    {
        "n": 9,
        "title": "gestalt-merge semantic.rs — Tree-sitter AST merge + similar fallback (v1 stub)",
        "current": "File does not exist. No AST-aware merge capability.",
        "desired": "Create `gestalt-merge/src/semantic.rs` with:\n- `pub struct SemanticMerger { language: Option<String> }`\n- `impl SemanticMerger { pub fn new(language: Option<String>) -> Self }`\n- `#[async_trait] impl MergeEngine for SemanticMerger`:\n  - `merge()`: v1 uses `similar` crate for line-level diff as fallback. Tree-sitter integration as optional (compile-time feature flag). For v1, implement: parse base/ours/theirs → find diff regions via `similar::capture_diff_slices` → merge non-overlapping regions → return result or conflict\n  - `test_mergeability()`: Similar diff-based pre-check\n  - `name()`: Returns `\"semantic\"`\n- `fn resolve_same_file(&self, base: &str, ours: &str, theirs: &str) -> Result<String, MergeError>` — the core 3-way merge for a single file content using `similar` crate",
        "research": "1. search: \"similar crate rust changelog 2026\"\n2. search: \"similar::capture_diff_slices example 3-way merge\"\n3. search: \"tree-sitter rust crate v0.24\"",
        "context": "File: `gestalt-merge/src/semantic.rs`. For v1, focus on `similar` crate line-diff approach. Tree-sitter integration can be feature-gated. `similar = \"2\"` is in Cargo.toml deps (issue #1).",
        "problem": "No file-content merge capability exists. GitThreeWay handles branch-level merge but not individual file conflict resolution.",
        "ac": "- [ ] `cargo check -p gestalt-merge` passes\n- [ ] `grep -c \"impl MergeEngine for SemanticMerger\" gestalt-merge/src/semantic.rs` >= 1\n- [ ] `grep -c \"fn resolve_same_file\" gestalt-merge/src/semantic.rs` >= 1\n- [ ] `grep -c \"similar::\" gestalt-merge/src/semantic.rs` >= 1",
        "files": "| `gestalt-merge/src/semantic.rs` | Does not exist | Create ~180 lines with SemanticMerger + similar fallback | MEDIUM |",
        "depends": "#5 (MergeEngine trait), #4 (types)",
        "parallel": "#7, #8",
        "order": 5,
        "effort": "Medium (1-4h)",
    },
    {
        "n": 10,
        "title": "gestalt-merge tests/engine_tests.rs — MergeEngine trait contract tests",
        "current": "File does not exist. No tests for merge engine trait.",
        "desired": "Create `gestalt-merge/tests/engine_tests.rs` with:\n- `test_merge_engine_trait_object` — verify trait is object-safe via Box<dyn MergeEngine>\n- `test_merge_engine_send_sync` — compile-time check that MergeEngine: Send + Sync\n- `test_integrate_result_construction` — create MergeOutput and verify fields\n- `test_conflict_info_serde` — serialize/deserialize ConflictInfo roundtrip\n- `test_merge_test_result_serde` — serialize/deserialize MergeTestResult\nUse temp git repos for integration tests.",
        "research": "1. search: \"gestalt-router/tests/router_tests.rs existing test patterns\"\n2. search: \"tempfile crate test pattern git repo rust\"",
        "context": "File: `gestalt-merge/tests/engine_tests.rs`. Use `gestalt_merge::engine::MergeEngine`, `gestalt_merge::types::*`. Follow same test pattern as `gestalt-router/tests/router_tests.rs`. Use `tempfile::TempDir` for temp repos.",
        "problem": "No tests exist for the MergeEngine trait contract, making it impossible to verify engine implementations.",
        "ac": "- [ ] `cargo test -p gestalt-merge --test engine_tests` passes\n- [ ] Test count >= 5\n- [ ] All tests pass (0 failures)",
        "files": "| `gestalt-merge/tests/engine_tests.rs` | Does not exist | Create ~100 lines with trait tests | LOW |",
        "depends": "#5 (MergeEngine trait)",
        "parallel": "#11, #12, #13, #14",
        "order": 6,
        "effort": "Small (<1h)",
    },
    {
        "n": 11,
        "title": "gestalt-merge tests/git_three_way_tests.rs — GitThreeWay integration tests",
        "current": "File does not exist. No tests for GitThreeWay engine.",
        "desired": "Create `gestalt-merge/tests/git_three_way_tests.rs` with:\n- `test_git_three_way_clean_merge` — two branches with disjoint file changes → Clean\n- `test_git_three_way_conflict_detection` — two branches modifying same file lines → Conflicts detected\n- `test_git_three_way_binary_conflict` — two branches modifying same binary → BinaryConflict\n- `test_git_three_way_test_mergeability` — verify test_mergeability returns correct result\n- `test_git_three_way_no_changes` — branches with no changes → Clean\nUse `tempfile::TempDir`, initialize git repos, create branches, commit changes, call GitThreeWay.",
        "research": "1. search: \"gestalt-router/tests/router_tests.rs::test_find_overlaps_* patterns\"\n2. search: \"gestalt-router/tests/integration_test.rs git repo setup\"",
        "context": "File: `gestalt-merge/tests/git_three_way_tests.rs`. Use `gestalt_merge::git_three_way::GitThreeWay`. Follow same git-repo-in-tempdir pattern as `gestalt-router/tests/router_tests.rs`. Build with `PKG_CONFIG_PATH=... cargo test -p gestalt-merge --test git_three_way_tests`.",
        "problem": "No tests exist for the GitThreeWay engine, risking regressions.",
        "ac": "- [ ] `cargo test -p gestalt-merge --test git_three_way_tests` passes\n- [ ] Test count >= 5\n- [ ] All tests pass (0 failures)",
        "files": "| `gestalt-merge/tests/git_three_way_tests.rs` | Does not exist | Create ~150 lines with GitThreeWay tests | MEDIUM |",
        "depends": "#7 (GitThreeWay impl)",
        "parallel": "#10, #12, #13, #14",
        "order": 7,
        "effort": "Medium (1-4h)",
    },
    {
        "n": 12,
        "title": "gestalt-merge tests/pr_tests.rs — PR creation tests (dry-run / mock)",
        "current": "File does not exist. No tests for PR creation.",
        "desired": "Create `gestalt-merge/tests/pr_tests.rs` with:\n- `test_pr_manager_new` — verify PrManager::new sets repo correctly\n- `test_pr_info_struct` — PrInfo creation and field access\n- `test_pr_manager_dry_run` — test error handling for invalid repo (expected failure, not panic)\n- `test_pr_manager_send_sync` — compile-time Send + Sync check\nUse mock/unit tests (no real gh calls).",
        "research": "1. search: \"rust unit test mock pattern without external calls\"\n2. search: \"gestalt-merge/src/pr.rs PrManager struct signatures\"",
        "context": "File: `gestalt-merge/tests/pr_tests.rs`. Use `gestalt_merge::pr::PrManager`, `gestalt_merge::pr::PrInfo`. These are unit tests — no external gh calls (use mock expectations or test error paths).",
        "problem": "No tests exist for PR creation logic, risking regressions in CI integration.",
        "ac": "- [ ] `cargo test -p gestalt-merge --test pr_tests` passes\n- [ ] Test count >= 4\n- [ ] All tests pass (0 failures)",
        "files": "| `gestalt-merge/tests/pr_tests.rs` | Does not exist | Create ~80 lines with PR tests | LOW |",
        "depends": "#8 (pr.rs)",
        "parallel": "#10, #11, #13, #14",
        "order": 7,
        "effort": "Small (<1h)",
    },
    {
        "n": 13,
        "title": "gestalt-merge tests/semantic_tests.rs — SemanticMerger tests",
        "current": "File does not exist. No tests for semantic/AST merge.",
        "desired": "Create `gestalt-merge/tests/semantic_tests.rs` with:\n- `test_semantic_clean_merge` — two changes in different parts of same file → merged content\n- `test_semantic_conflict_detection` — two changes on same lines → conflict\n- `test_semantic_no_changes` — identical files → clean\n- `test_resolve_same_file_identical` — base=ours=theirs → identical output\n- `test_resolve_same_file_conflict` — ours and theirs differ on same line → conflict\nUse `similar` crate directly to test diff logic. No git repos needed — pure string content tests.",
        "research": "1. search: \"gestalt-merge/src/semantic.rs::resolve_same_file function signature\"\n2. search: \"similar crate test examples\"",
        "context": "File: `gestalt-merge/tests/semantic_tests.rs`. Use `gestalt_merge::semantic::SemanticMerger`. These file-content tests work on strings, not git repos. Simpler than git-based tests.",
        "problem": "No tests exist for file-content merge logic.",
        "ac": "- [ ] `cargo test -p gestalt-merge --test semantic_tests` passes\n- [ ] Test count >= 5\n- [ ] All tests pass (0 failures)",
        "files": "| `gestalt-merge/tests/semantic_tests.rs` | Does not exist | Create ~100 lines with SemanticMerger tests | LOW |",
        "depends": "#9 (semantic.rs)",
        "parallel": "#10, #11, #12, #14",
        "order": 7,
        "effort": "Small (<1h)",
    },
    {
        "n": 14,
        "title": "gestalt-merge tests/integration_tests.rs — Full pipeline end-to-end tests",
        "current": "File does not exist. No end-to-end integration tests.",
        "desired": "Create `gestalt-merge/tests/integration_tests.rs` with:\n- `test_full_merge_pipeline` — real git repo, 2 branches with disjoint files, GitThreeWay.merge succeeds\n- `test_full_pipeline_with_conflict` — 2 branches modifying same file, GitThreeWay.merge returns conflicts\n- `test_sequential_multi_merge` — merge 3+ branches in sequence, verify tree chain\n- `test_merge_then_test_mergeability` — after merge, verify test_mergeability on result\n- `test_pr_integration_flow` — run git operations that PR creation would use (no actual gh call)\nUse `tempfile::TempDir`, real git commands (via std::process::Command).",
        "research": "1. search: \"gestalt-router/tests/integration_test.rs full pipeline pattern\"\n2. search: \"gestalt-router/tests/router_tests.rs::test_integrate_branches_*\"",
        "context": "File: `gestalt-merge/tests/integration_tests.rs`. Use both `gestalt_merge::git_three_way::GitThreeWay` and `gestalt_merge::engine::MergeEngine`. Build the same git-repo-in-tempdir setup as router_tests.rs.",
        "problem": "No end-to-end tests exist that verify the full merge pipeline from git operations to merge result.",
        "ac": "- [ ] `cargo test -p gestalt-merge --test integration_tests` passes\n- [ ] Test count >= 5\n- [ ] All tests pass (0 failures)",
        "files": "| `gestalt-merge/tests/integration_tests.rs` | Does not exist | Create ~200 lines with E2E tests | MEDIUM |",
        "depends": "#7 (GitThreeWay), #5 (MergeEngine trait)",
        "parallel": "#10, #11, #12, #13",
        "order": 7,
        "effort": "Medium (1-4h)",
    },
    {
        "n": 15,
        "title": "Integrate gestalt-merge into gestalt-router + update documentation",
        "current": "gestalt-router does not use gestalt-merge. No MergeEngine in Router. Docs reflect v1 only.",
        "desired": "1. `gestalt-router/Cargo.toml`: Add `gestalt-merge` as workspace dependency\n2. `gestalt-router/src/router.rs`: Add `merger: Box<dyn MergeEngine>` field to Router struct, initialize in new(), use in execute()\n3. `gestalt_cli/src/main.rs`: Wire merger into CLI run command\n4. `.gitcore/features.json`: Add `feat-merge-engine` feature with 100%\n5. `SRC.md`: Add gestalt-merge crate to directory structure\n6. `CHANGELOG.md`: Add Phase 2 entries\n7. `TODO.md`: Mark Phase 2 complete",
        "research": "1. search: \"gestalt-router/Cargo.toml for dependency pattern\"\n2. search: \"gestalt-router/src/router.rs::Router struct definition\"\n3. search: \"gestalt_cli/src/main.rs gestalt run command\"",
        "context": "Files:\n- `gestalt-router/Cargo.toml`: Add `gestalt-merge = { path = \"../gestalt-merge\" }`\n- `gestalt-router/src/router.rs`: Import MergeEngine, add `merger: Box<dyn MergeEngine>` field\n- `gestalt_cli/src/main.rs`: Add merger parameter to Router::new\n- `.gitcore/features.json`: Update metadata\n- `SRC.md`: Add gestalt-merge crate map entry\n- `CHANGELOG.md`: Add Phase 2 section\n- `TODO.md`: Update completed items",
        "problem": "The gestalt-merge crate exists but is not integrated into the Router, making it unusable from the CLI.",
        "ac": "- [ ] `cargo check --workspace` passes (0 errors)\n- [ ] `grep -c \"gestalt-merge\" gestalt-router/Cargo.toml` >= 1\n- [ ] `grep -c \"MergeEngine\" gestalt-router/src/router.rs` >= 1\n- [ ] `.gitcore/features.json` has `feat-merge-engine` at 100%\n- [ ] `SRC.md` has gestalt-merge in directory tree",
        "files": "| `gestalt-router/Cargo.toml` | ~20 lines | Add gestalt-merge dep | LOW |\n| `gestalt-router/src/router.rs` | ~100 lines | Add merger field + init | MED |\n| `.gitcore/features.json` | ~200 lines | Add feat-merge-engine 100% | LOW |\n| `SRC.md` | ~85 lines | Add gestalt-merge section | LOW |\n| `CHANGELOG.md` | ~70 lines | Add Phase 2 entries | LOW |\n| `TODO.md` | ~70 lines | Mark Phase 2 complete | LOW |",
        "depends": "#5 (MergeEngine trait), #7 (GitThreeWay impl)",
        "parallel": "#10, #11, #12, #13, #14",
        "order": 8,
        "effort": "Medium (1-4h)",
    },
]

# Create all 15 issues
print("=" * 60)
print("Creating 15 GitHub Issues for Gestalt Phase 2")
print("=" * 60)

for issue in issues:
    n = issue["n"]
    body = BASE.format(
        n=n,
        title=issue["title"],
        current=issue["current"],
        desired=issue["desired"],
        research=issue["research"],
        context=issue["context"],
        problem=issue["problem"],
        ac=issue["ac"],
        files=issue["files"],
        depends=issue["depends"],
        parallel=issue["parallel"],
        order=issue["order"],
        effort=issue["effort"],
    )
    gh_issue(issue["title"], body, labels="ola1,wave-1")

print("\n✅ Done! 15 issues created.")
print("\nExecution order:")
print("  Sequential: #1 (Cargo.toml) → #2 (lib.rs)")
print("  Parallel batch 1: #3, #4, #5, #6 (types + traits)")
print("  Sequential: #5 (engine.rs) — blocker for impls")
print("  Parallel batch 2: #7, #8, #9 (implementations)")
print("  Parallel batch 3: #10, #11, #12, #13, #14 (tests)")
print("  Sequential: #15 (integration + docs)")
print("\nLabel strategy:")
print("  Apply `jules` label one batch at a time, in order above")
