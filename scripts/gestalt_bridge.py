"""
Gestalt Bridge - OpenClaw Integration
======================================
Bridge to use all Gestalt Framework instances from OpenClaw:
- Gemini CLI (via OAuth)
- Qwen (via OAuth) 
- OpenAI (API key)
- Ollama (local)
- MCP Server (:3000)
- Consensus voting
"""
import subprocess
import json
import requests
import os
from typing import Dict, List, Optional, Any
from pathlib import Path

# Configuration
GESTALT_CLI = r"E:\scripts-python\gestalt-rust\target\debug\gestalt_cli.exe"
GESTALT_MCP_URL = "http://127.0.0.1:3000"


class GestaltBridge:
    """Bridge to connect OpenClaw with Gestalt Framework"""
    
    def __init__(self):
        self.mcp_url = GESTALT_MCP_URL
        self.cli_path = GESTALT_CLI
        self._check_status()
    
    def _check_status(self):
        """Check Gestalt authentication status"""
        try:
            result = subprocess.run(
                [self.cli_path, "status"],
                capture_output=True,
                text=True,
                timeout=10,
                encoding='utf-8',
                errors='ignore'
            )
            self.auth_status = result.stdout
        except Exception as e:
            self.auth_status = f"Error: {e}"
    
    # === MCP Server Tools ===
    def call_mcp(self, method: str, params: dict = None) -> dict:
        """Call Gestalt MCP server"""
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params or {}
        }
        try:
            resp = requests.post(f"{self.mcp_url}/mcp", json=payload, timeout=30)
            return resp.json()
        except Exception as e:
            return {"error": str(e)}
    
    def mcp_tools_list(self) -> List[str]:
        """List available MCP tools"""
        result = self.call_mcp("tools/list")
        tools = result.get("result", {}).get("tools", [])
        return [t.get("name") for t in tools]
    
    def mcp_call(self, tool_name: str, args: dict = None) -> Any:
        """Call a specific MCP tool"""
        result = self.call_mcp("tools/call", {
            "name": tool_name,
            "arguments": args or {}
        })
        return result.get("result", {})
    
    # === Gestalt CLI Commands ===
    def run_prompt(self, prompt: str, consensus: bool = False, model: str = None) -> str:
        """Run a prompt through Gestalt CLI"""
        cmd = [self.cli_path]
        
        if consensus:
            cmd.append("--consensus")
        
        if model:
            if model == "gemini":
                cmd.extend(["--gemini-model", "gemini-2.0-flash"])
            elif model == "qwen":
                cmd.extend(["--qwen-model", "qwen-coder"])
            elif model == "openai":
                cmd.extend(["--openai-model", "gpt-4"])
            elif model == "ollama":
                cmd.extend(["--ollama-model", "llama2"])
        
        cmd.extend(["-p", prompt])
        
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=60, encoding='utf-8', errors='ignore')
            return result.stdout + result.stderr
        except Exception as e:
            return f"Error: {e}"
    
    # === Multi-Model Consensus ===
    def consensus(self, prompt: str) -> Dict:
        """
        Run prompt through multiple models and get consensus.
        Uses --consensus flag with Gestalt CLI.
        """
        print(f"🗳️  Running consensus for: {prompt[:50]}...")
        
        result = self.run_prompt(prompt, consensus=True)
        
        return {
            "prompt": prompt,
            "result": result,
            "providers": ["gemini", "qwen", "openai", "ollama"]
        }
    
    # === Specific Model Calls ===
    def ask_gemini(self, prompt: str) -> str:
        """Ask Gemini via Gestalt"""
        return self.run_prompt(prompt, model="gemini")
    
    def ask_qwen(self, prompt: str) -> str:
        """Ask Qwen via Gestalt"""
        return self.run_prompt(prompt, model="qwen")
    
    def ask_ollama(self, prompt: str, model: str = "llama2") -> str:
        """Ask Ollama via Gestalt"""
        return self.run_prompt(prompt, model="ollama")
    
    # === Project Analysis ===
    def analyze_project(self, path: str = ".") -> Dict:
        """Analyze a project using MCP tools"""
        return {
            "structure": self.mcp_call("file_tree", {"path": path, "depth": 3}),
            "context": self.mcp_call("get_context", {"path": path}),
            "analysis": self.mcp_call("analyze_project", {"path": path})
        }
    
    # === Code Operations ===
    def search_code(self, pattern: str, path: str = ".", extensions: str = ".rs,.py,.ts") -> Dict:
        """Search code using MCP"""
        return self.mcp_call("search_code", {
            "pattern": pattern,
            "path": path,
            "extensions": extensions
        })
    
    def read_file(self, path: str, lines: int = 100) -> str:
        """Read file using MCP"""
        result = self.mcp_call("read_file", {"path": path, "lines": lines})
        return result
    
    # === Git Operations ===
    def git_status(self, path: str = ".") -> Dict:
        """Get git status"""
        return self.mcp_call("git_status", {"path": path})
    
    def git_log(self, path: str = ".", count: int = 10) -> Dict:
        """Get git log"""
        return self.mcp_call("git_log", {"path": path, "count": count})
    
    # === System Info ===
    def system_info(self) -> Dict:
        """Get system info"""
        return self.mcp_call("system_info", {})
    
    # === Status ===
    def status(self) -> Dict:
        """Get full Gestalt status"""
        return {
            "auth": self.auth_status,
            "mcp_url": self.mcp_url,
            "mcp_tools": self.mcp_tools_list(),
            "cli_path": self.cli_path
        }


# === OpenClaw Skill Interface ===
def handle_gestalt_command(command: str, args: dict = None) -> str:
    """Handle commands from OpenClaw"""
    bridge = GestaltBridge()
    args = args or {}
    
    if command == "status":
        return json.dumps(bridge.status(), indent=2)
    
    elif command == "ask":
        prompt = args.get("prompt", "")
        model = args.get("model")  # gemini, qwen, ollama, openai
        return bridge.run_prompt(prompt, model=model)
    
    elif command == "consensus":
        prompt = args.get("prompt", "")
        return json.dumps(bridge.consensus(prompt), indent=2)
    
    elif command == "analyze":
        path = args.get("path", ".")
        return json.dumps(bridge.analyze_project(path), indent=2)
    
    elif command == "search":
        pattern = args.get("pattern", "")
        path = args.get("path", ".")
        return json.dumps(bridge.search_code(pattern, path), indent=2)
    
    elif command == "mcp":
        tool = args.get("tool", "")
        tool_args = args.get("args", {})
        return json.dumps(bridge.mcp_call(tool, tool_args), indent=2)
    
    else:
        return f"Unknown command: {command}"


# === CLI ===
if __name__ == "__main__":
    import sys
    
    bridge = GestaltBridge()
    
    print("""
⚡ GESTALT BRIDGE
=================
Available commands:
  status              - Show Gestalt status
  ask <prompt>        - Ask a model
  consensus <prompt> - Multi-model consensus
  analyze <path>     - Analyze project
  search <pattern>   - Search code
  mcp <tool>         - Call MCP tool
    """)
    
    if len(sys.argv) > 1:
        cmd = sys.argv[1]
        if cmd == "status":
            print(bridge.status())
        elif cmd == "ask":
            prompt = " ".join(sys.argv[2:])
            print(bridge.run_prompt(prompt))
        elif cmd == "consensus":
            prompt = " ".join(sys.argv[2:])
            print(bridge.consensus(prompt))
        else:
            print(f"Unknown: {cmd}")
