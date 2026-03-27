# ⚠️ Known Issues

## Security Vulnerabilities

### RUSTSEC-2026-0049: rustls-pemfile (Unmaintained)

**Package:** `rustls-pemfile`  
**Affected Versions:** `<1.0.0`  
**Status:** Blocked by transitive dependencies

The `rustls-pemfile` crate is unmaintained. This affects TLS connections through the `rustls` ecosystem.

**Impact:** No direct security vulnerability, but the crate may have unpatched issues.

**Resolution:** Requires upstream fix. Monitor https://github.com/rustls/pemfile/issues/61

---

### RUSTSEC-2026-0002: lru IterMut Soundness

**Package:** `lru`  
**Affected Versions:** `0.12.5` (and likely earlier)  
**Patched Version:** `>=0.16.3`  
**Status:** Blocked by transitive dependencies  

The `IterMut` iterator in the `lru` crate violates Stacked Borrows rules, potentially causing memory corruption.

**Impact:** Soundness issue - could cause undefined behavior in concurrent scenarios.

**Workaround:** No workaround available until upstream releases `>=0.16.3`

**Resolution:** Requires upstream fix. Monitor https://github.com/jeromefroe/lru-rs/pull/224

---

## Known Limitations

### MCP Tools Not Wired to Core ToolRegistry

**Issue:** The `gestalt_mcp` crate defines 18 MCP tools (see `gestalt_mcp/src/lib.rs`), but these are **not wired** to the `ToolRegistry` in `gestalt_core`.

**Status:** **Known Gap** - Issue tracked in project

**Impact:** The MCP server runs as a standalone HTTP server on port 3000, but the core `gestalt_core::application::agent::tools::create_gestalt_tools()` function registers a separate set of tools directly in the Rust code.

**Affected Tools (gestalt_mcp):**
- echo, analyze_project, list_files, read_file, get_context
- search_code, exec_command, git_status, git_log, file_tree
- grep, create_file, web_fetch, system_info
- task_create, task_status, task_list

**Tools Registered in gestalt_core (tools.rs):**
- ScanWorkspaceTool, SearchCodeTool, ExecuteShellTool, ReadFileTool
- WriteFileTool, GitStatusTool, GitLogTool, GitBranchTool
- GitAddTool, GitCommitTool, GitPushTool, CloneRepoTool
- ListReposTool, AskAiTool

**Note:** There is overlap but the two registries are independent.

---

### Mock LLM Providers (Historical)

**Status:** **Partially Resolved**

The `synapse-agentic` crate originally contained mock LLM providers that returned `"mock".to_string()` instead of calling actual APIs.

**Current State:** 
- API key reading from environment variables (`GEMINI_API_KEY`, `MINIMAX_API_KEY`) is implemented
- However, the actual HTTP calls to Gemini/MiniMax APIs are not yet implemented
- The providers still return mock responses

**Workaround:** Use the `--model` flag with external API calls through the Python bridge layer (`gestalt_bridge.py`, `gestalt_superagent.py`)

---

## CI/CD Status

**PR #239:** `fix: correct artifact paths in release workflow`
- Status: CI in progress (as of 2026-03-27)
- Benchmarks: ✅ Passing
- Guardian Agent: ✅ Passing
- CI: ⏳ In Progress

Previous CI fixes in PRs #236, #237 have stabilized most workflows.
