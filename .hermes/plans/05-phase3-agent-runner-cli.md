# Phase 3: AgentRunner + CLI + Xavier Bridge

## Task 3.1: AgentRunner — Spawn Agents Externos

**Archivo nuevo:** `gestalt_core/src/orchestrator/agent_runner.rs`

```rust
pub struct AgentRunner {
    /// Timeout por agente
    timeout: Duration,
}

pub struct AgentResult {
    pub agent_id: String,
    pub success: bool,
    pub exit_code: i32,
    pub worktree_path: PathBuf,
}

impl AgentRunner {
    /// Spawnea un agente CLI externo en un worktree
    pub async fn spawn_agent(
        &self,
        agent_type: AgentType,
        worktree: &AgentWorkspace,
        task: &str,
    ) -> Result<AgentHandle> {
        let cmd = match agent_type {
            AgentType::Agy => {
                // agy -p "task" --model gemini-3.6-flash-high
                //   --effort high --dangerously-skip-permissions
                //   --add-dir {worktree}
                let mut c = Command::new("agy");
                c.args(["-p", task])
                 .args(["--model", "gemini-3.6-flash-high"])
                 .args(["--effort", "high"])
                 .args(["--dangerously-skip-permissions"])
                 .args(["--add-dir", worktree.worktree_path.to_str().unwrap()])
                 .current_dir(&worktree.worktree_path);
                c
            }
            AgentType::Kimi => {
                let mut c = Command::new("kimi");
                c.args(["-p", task])
                 .current_dir(&worktree.worktree_path);
                c
            }
            AgentType::Codex => {
                let mut c = Command::new("codex");
                c.args(["--prompt", task])
                 .current_dir(&worktree.worktree_path);
                c
            }
            AgentType::Claude => {
                let mut c = Command::new("claude");
                c.args(["-p", task])
                 .current_dir(&worktree.worktree_path);
                c
            }
        };
        // Spawn y trackear PID
        // Timeout configurable
        // Log output
    }

    /// Monitorea agente hasta completar o timeout
    pub async fn wait_agent(handle: AgentHandle) -> Result<AgentResult> {
        // wait, check exit code, return result
    }
}

pub enum AgentType {
    Agy,
    Kimi,
    Codex,
    Claude,
    Jules,
}
```

## Task 3.2: Xavier CLI Bridge

**Archivo nuevo:** `gestalt_core/src/xavier/bridge.rs`

```rust
pub struct XavierClient {
    base_url: String,
    token: String,
}

impl XavierClient {
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("XAVIER_URL")
                .unwrap_or("http://127.0.0.1:8006".into()),
            token: std::env::var("XAVIER_TOKEN")
                .unwrap_or_default(),
        }
    }

    /// Guardar contexto de una tarea en Xavier
    pub async fn save_context(
        &self, key: &str, content: &str,
    ) -> Result<()> {
        // POST /v1/memories
    }

    /// Buscar contexto relevante de tareas anteriores
    pub async fn search_context(
        &self, query: &str, limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        // POST /v1/memories/search
    }

    /// Cargar skill para el agente
    pub async fn load_skill(&self, skill_name: &str) -> Result<String> {
        // GET /v1/skills/{name}
    }
}
```

## Task 3.3: CLI Entry Point — Gestalt Wave

**Archivo a modificar:** `gestalt_timeline/src/main.rs`

```bash
# Nueva interfaz CLI
gestalt wave init --id wave-07 --agents 15
gestalt wave status --id wave-07
gestalt wave collect --id wave-07
gestalt wave merge --id wave-07 --auto
gestalt wave merge --id wave-07 --llm-resolve
gestalt wave finalize --id wave-07

# Control de agentes
gestalt agent run --wave wave-07 --agent agy --task "fix lint in file.rs"
gestalt agent status --id agent-42

# Merge individual
gestalt merge file.rs --ours agent-a --theirs agent-b

# Sin Database
gestalt --no-db status
```

## Task 3.4: Config Minimalista

**Archivo:** `config/default.toml`

```toml
[waves]
base_dir = "/tmp/gestalt"
default_timeout_s = 120

[agents]
timeout_s = 60
default_type = "agy"
model = "gemini-3.6-flash-high"

[merge]
algorithm = "histogram"
style = "zdiff3"
favor = "union"
max_auto_conflicts = 3

[xavier]
url = "http://127.0.0.1:8006"
```
