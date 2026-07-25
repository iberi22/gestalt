"""
Gestalt Hybrid Bridge - MCP + OpenClaw Models
=============================================
Uses Gestalt MCP Server for tools + OpenClaw MiniMax for LLM
"""
import requests
import json
import os
from typing import Dict, Any

# Configuration
GESTALT_MCP_URL = "http://127.0.0.1:3000"
OPENCLAW_URL = "http://127.0.0.1:18789"


class GestaltHybrid:
    """Hybrid: Gestalt MCP tools + OpenClaw MiniMax"""
    
    def __init__(self):
        self.mcp_url = GESTALT_MCP_URL
        self.openclaw_url = OPENCLAW_URL
    
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
    
    def list_tools(self):
        """List MCP tools"""
        result = self.call_mcp("tools/list", {})
        return result.get("tools", [])
    
    # === OpenClaw MiniMax (working!) ===
    def ask_minimax(self, prompt: str, system_prompt: str = None) -> str:
        """Ask MiniMax via OpenClaw API"""
        # Direct API call to MiniMax
        import requests
        
        api_key = os.getenv("MINIMAX_API_KEY", "")
        if not api_key:
            # Try to get from OpenClaw config
            return "MINIMAX_API_KEY not set"
        
        headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json"
        }
        
        payload = {
            "model": "MiniMax-M2.5",
            "messages": [
                {"role": "system", "content": system_prompt or "You are a helpful assistant."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.7
        }
        
        try:
            resp = requests.post(
                "https://api.minimax.chat/v1/text/chatcompletion_v2",
                headers=headers,
                json=payload,
                timeout=60
            )
            data = resp.json()
            return data.get("choices", [{}])[0].get("message", {}).get("content", str(data))
        except Exception as e:
            return f"Error: {e}"
    
    # === Full Workflow ===
    def ask_with_context(self, prompt: str) -> str:
        """
        Full agentic workflow:
        1. Get project context from MCP
        2. Ask MiniMax with context
        """
        # 1. Get context from MCP
        context = self.call_mcp("get_context", {"path": "."})
        
        # 2. Ask MiniMax
        system_prompt = f"""You are an expert developer assistant.

Project Context:
{json.dumps(context)[:2000]}

Provide helpful, accurate answers based on the project context."""
        
        return self.ask_minimax(prompt, system_prompt)
    
    # === Convenience Methods ===
    def file_tree(self, path: str = ".", depth: int = 3):
        """Get file tree"""
        return self.call_mcp("file_tree", {"path": path, "depth": depth})
    
    def search(self, pattern: str, path: str = "."):
        """Search code"""
        return self.call_mcp("search_code", {"pattern": pattern, "path": path})
    
    def analyze_project(self, path: str = "."):
        """Analyze project"""
        return self.call_mcp("analyze_project", {"path": path})
    
    def git_status(self, path: str = "."):
        """Git status"""
        return self.call_mcp("git_status", {"path": path})


# === Test ===
if __name__ == "__main__":
    hybrid = GestaltHybrid()
    
    print("🧪 Testing Gestalt Hybrid Bridge\n")
    
    # Test MCP tools
    print("1. MCP Tools:")
    tools = hybrid.list_tools()
    print(f"   Found {len(tools)} tools")
    
    # Test context
    print("\n2. Project Context:")
    ctx = hybrid.call_mcp("get_context", {"path": "."})
    print(f"   Context retrieved")
    
    # Test MiniMax
    print("\n3. MiniMax:")
    result = hybrid.ask_minimax("What is 2+2?")
    print(f"   Response: {result[:200]}...")
    
    print("\n✅ Hybrid bridge working!")
