"""
Gestalt + Codex Bridge
======================
Hybrid bridge: Gestalt MCP (tools) + Codex (LLM via auth token)
"""
import subprocess
import json
import requests
import os
from typing import Dict, Any

GESTALT_MCP_URL = "http://127.0.0.1:3000"
CODEX_CLI = r"C:\Users\belal\AppData\Roaming\npm\codex.cmd"


class CodexGestaltBridge:
    """Bridge using Codex for LLM + Gestalt MCP for tools"""
    
    def __init__(self):
        self.mcp_url = GESTALT_MCP_URL
        self._check_codex()
    
    def _check_codex(self):
        """Check Codex availability"""
        try:
            result = subprocess.run(
                [CODEX_CLI, "--version"],
                capture_output=True,
                text=True,
                timeout=10,
                encoding='utf-8',
                errors='ignore'
            )
            self.codex_version = result.stdout.strip()
        except Exception as e:
            self.codex_version = f"Error: {e}"
    
    # === Codex (LLM) ===
    def ask_codex(self, prompt: str, model: str = "o3") -> str:
        """
        Ask Codex - uses auth token from login
        Models: o3, o4-mini
        """
        cmd = [CODEX_CLI, "exec", f"-c model={model}", prompt]
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=120,
                encoding='utf-8',
                errors='ignore'
            )
            return result.stdout + result.stderr
        except Exception as e:
            return f"Error: {e}"
    
    def ask_codex_simple(self, prompt: str) -> str:
        """Simple prompt to Codex"""
        return self.ask_codex(prompt, "o3")
    
    # === MCP Tools ===
    def call_mcp(self, tool: str, args: dict = None) -> Any:
        """Call Gestalt MCP tool"""
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": tool, "arguments": args or {}}
        }
        try:
            resp = requests.post(f"{self.mcp_url}/mcp", json=payload, timeout=30)
            return resp.json().get("result", {})
        except Exception as e:
            return {"error": str(e)}
    
    def mcp_tools(self) -> list:
        """List MCP tools"""
        result = self.call_mcp("tools/list", {})
        return result.get("tools", [])
    
    # === Full Workflow: Codex + MCP Context ===
    def agentic_ask(self, prompt: str, use_tools: bool = True) -> Dict:
        """
        Full agentic workflow:
        1. Optionally get project context from MCP
        2. Ask Codex with context
        """
        context = ""
        
        if use_tools:
            # Get relevant context from MCP
            try:
                project_ctx = self.call_mcp("get_context", {"path": "."})
                context = json.dumps(project_ctx)[:3000]
            except:
                pass
        
        # Build prompt with context
        full_prompt = f"""Project Context:
{context}

Question: {prompt}

Provide a helpful, accurate answer based on the project context."""
        
        # Ask Codex
        response = self.ask_codex_simple(full_prompt)
        
        return {
            "prompt": prompt,
            "context_used": bool(context),
            "response": response,
            "tools_available": len(self.mcp_tools()) if use_tools else 0
        }
    
    # === Convenience ===
    def search_code(self, pattern: str, path: str = "."):
        """Search code"""
        return self.call_mcp("search_code", {
            "pattern": pattern,
            "path": path
        })
    
    def file_tree(self, path: str = ".", depth: int = 3):
        """File tree"""
        return self.call_mcp("file_tree", {
            "path": path,
            "depth": depth
        })
    
    def git_status(self, path: str = "."):
        """Git status"""
        return self.call_mcp("git_status", {"path": path})
    
    def analyze_project(self, path: str = "."):
        """Analyze project"""
        return self.call_mcp("analyze_project", {"path": path})


# === OpenClaw Integration ===
def handle_command(command: str, args: dict = None) -> str:
    """Handle commands from OpenClaw"""
    bridge = CodexGestaltBridge()
    args = args or {}
    
    if command == "status":
        return json.dumps({
            "codex_version": bridge.codex_version,
            "mcp_tools": len(bridge.mcp_tools()),
            "status": "ready"
        }, indent=2)
    
    elif command == "ask":
        prompt = args.get("prompt", "")
        return bridge.ask_codex_simple(prompt)
    
    elif command == "agentic":
        prompt = args.get("prompt", "")
        return json.dumps(bridge.agentic_ask(prompt), indent=2)
    
    elif command == "search":
        pattern = args.get("pattern", "")
        path = args.get("path", ".")
        return json.dumps(bridge.search_code(pattern, path), indent=2)
    
    elif command == "tree":
        path = args.get("path", ".")
        depth = args.get("depth", 3)
        return json.dumps(bridge.file_tree(path, depth), indent=2)
    
    elif command == "mcp":
        tool = args.get("tool", "")
        tool_args = args.get("args", {})
        return json.dumps(bridge.call_mcp(tool, tool_args), indent=2)
    
    else:
        return f"Unknown command: {command}"


# === Test ===
if __name__ == "__main__":
    print("""
⚡ CODEX + GESTALT BRIDGE
=========================
Using Codex for LLM + Gestalt MCP for tools
    """)
    
    bridge = CodexGestaltBridge()
    
    print(f"Codex: {bridge.codex_version}")
    print(f"MCP Tools: {len(bridge.mcp_tools())}")
    
    # Test Codex
    print("\n🧪 Testing Codex...")
    result = bridge.ask_codex_simple("What is 2+2?")
    print(f"Result: {result[:300]}...")
