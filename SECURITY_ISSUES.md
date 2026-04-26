## Security Vulnerability Report - Gestalt Swarm System

### Severity: CRITICAL

### Summary
ExecuteShellTool in `gestalt_core/src/application/agent/tools.rs` is vulnerable to **command injection** because user-provided input is passed directly to `powershell -Command` or `sh -c` without sanitization.

---

### Vulnerable Code

**File:** `gestalt_core/src/application/agent/tools.rs` (lines ~155-170)

```rust
async fn call(&self, _ctx: &dyn ToolContext, args: Value) -> anyhow::Result<Value> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

    #[cfg(target_os = "windows")]
    let mut cmd = tokio::process::Command::new("powershell");
    #[cfg(target_os = "windows")]
    cmd.arg("-Command").arg(command);  // <-- INJECTION POINT

    #[cfg(not(target_os = "windows"))]
    let mut cmd = tokio::process::Command::new("sh");
    #[cfg(not(target_os = "windows"))]
    cmd.arg("-c").arg(command);  // <-- INJECTION POINT
```

### Attack Vector
An attacker can pass a malicious `command` parameter with shell metacharacters (semicolon, pipe, &&, ||, etc) to execute arbitrary commands.

### Impact
- **Remote Code Execution** on the host system
- **Full system compromise** if running with elevated privileges
- **Data exfiltration** via arbitrary command execution

---

### Recommended Fix

1. **Allowlist approach** - Create a limited set of permitted commands
2. **Input validation** - Strict regex validation for allowed characters
3. **Shell execution disabled** - Use `Command::new("program")` with `.arg()` for each token instead of shell execution
4. **Timeout** - Add execution timeout to prevent resource exhaustion

Example fix pattern:
```rust
// Only allow alphanumeric, spaces, hyphens, underscores, and limited special chars
fn validate_command(cmd: &str) -> anyhow::Result<()> {
    if cmd.len() > 500 {
        anyhow::bail!("command too long");
    }
    if !cmd.chars().all(|c| c.is_alphanumeric() || c.is_ascii_whitespace() || "_-./".contains(c)) {
        anyhow::bail!("invalid characters in command");
    }
    Ok(())
}
```

---

### Additional Findings

#### MEDIUM: Hardcoded OAuth Fallback Secrets
**File:** `gestalt_core/src/adapters/auth/google_oauth.rs`

The code defaults to placeholder strings when OAuth credentials are not set:
```rust
fn get_oauth_client_id() -> String {
    std::env::var("GOOGLE_OAUTH_CLIENT_ID").unwrap_or_else(|_| {
        tracing::warn!("GOOGLE_OAUTH_CLIENT_ID not set, using default");  // Leaks to logs!
        "YOUR_CLIENT_ID_HERE".to_string()
    })
}
```

**Issue:** Warning message leaks the fact that default credentials are in use.

#### MEDIUM: Incomplete Path Traversal Validation
**File:** `gestalt_core/src/application/agent/tools.rs`

Git path validation checks for `..` but doesn't handle all edge cases.

---

### Files Affected
- `gestalt_core/src/application/agent/tools.rs` - ExecuteShellTool command injection
- `gestalt_core/src/adapters/auth/google_oauth.rs` - OAuth fallback secrets

### Recommendation
**Do not deploy** until ExecuteShellTool is fixed. The command injection vulnerability is critical and allows full system compromise.

---

*Found during automated security audit*
