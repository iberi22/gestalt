# Gestalt Bridge Skill

## Descripción
Puente de integración entre OpenClaw y el framework Gestalt. Permite usar todas las instancias de Gestalt desde OpenClaw.

## Ubicación
`E:\scripts-python\gestalt-rust\gestalt_bridge.py`

## Instancias Gestalt Disponibles

| Instancia | Auth | Descripción |
|-----------|------|-------------|
| **MCP Server** | - | 17 tools (file_tree, search, git, etc) |
| **Gemini CLI** | OAuth | gemini-2.0-flash |
| **Qwen** | OAuth | qwen-coder |
| **OpenAI** | API Key | gpt-4 |
| **Ollama** | Local | llama2, etc |

## Uso

### Python
```python
from gestalt_bridge import GestaltBridge, handle_gestalt_command

bridge = GestaltBridge()

# Ver estado
print(bridge.status())

# Ask a model
response = bridge.run_prompt("Hello", model="gemini")

# Consensus
result = bridge.consensus("What is the best approach?")

# MCP tools
bridge.mcp_call("file_tree", {"path": ".", "depth": 2})
bridge.mcp_call("search_code", {"pattern": "error", "extensions": ".rs"})
```

### CLI
```bash
cd E:\scripts-python\gestalt-rust
python gestalt_bridge.py status
python gestalt_bridge.py ask "Hello world"
python gestalt_bridge.py consensus "What is Rust?"
```

## Comandos desde OpenClaw

```python
# Status
handle_gestalt_command("status")

# Ask with specific model
handle_gestalt_command("ask", {"prompt": "...", "model": "gemini"})

# Consensus (multi-model)
handle_gestalt_command("consensus", {"prompt": "..."})

# Analyze project
handle_gestalt_command("analyze", {"path": "E:\\scripts-python\\myproject"})

# Search code
handle_gestalt_command("search", {"pattern": "TODO", "path": "."})

# Call MCP tool directly
handle_gestalt_command("mcp", {"tool": "file_tree", "args": {"depth": 3}})
```

## MCP Tools Disponibles

1. `echo` - Echo back
2. `analyze_project` - Analyze project structure
3. `list_files` - List files with filters
4. `read_file` - Read file contents
5. `get_context` - Get AI context
6. `search_code` - Search in code
7. `exec_command` - Execute shell command
8. `git_status` - Git status
9. `git_log` - Git log
10. `file_tree` - Directory tree
11. `grep` - Grep with context
12. `create_file` - Create file
13. `web_fetch` - Fetch URL
14. `system_info` - System info
15. `task_create` - Create task
16. `task_status` - Task status
17. `task_list` - List tasks

## Iniciar MCP Server

```bash
cd E:\scripts-python\gestalt-rust
cargo run -p gestalt_mcp -- --http
```

## Autenticación

```bash
# Ver estado de auth
E:\scripts-python\gestalt-rust\target\debug\gestalt_cli.exe status
```

---

*Skill para integrar Gestalt en OpenClaw*
