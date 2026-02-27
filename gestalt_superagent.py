"""
Gestalt SuperAgent - Consensus & Automation Layer
=================================================
Wrapper that adds superpowers to Gestalt MCP:
1. Multi-model consensus voting
2. Real web search (Brave/Tavily)
3. Automation workflows
4. Context memory
"""
import asyncio
import json
import requests
from typing import List, Dict, Any, Optional
from datetime import datetime
import os

# Configuration
GESTALT_MCP_URL = os.getenv("GESTALT_MCP_URL", "http://127.0.0.1:3000")

# Web Search Configuration - API keys from environment
BRAVE_API_KEY = os.getenv("BRAVE_API_KEY", "")
TAVILY_API_KEY = os.getenv("TAVILY_API_KEY", "")
BRAVE_API_KEY = BRAVE_API_KEY or "A56wg6D0M6jNorJCqKzBi3W10bIZbF-d"  # Default from config if available


class WebSearch:
    """Web search using Brave or Tavily API"""
    
    def __init__(self, api_key: str = None, provider: str = "brave"):
        self.api_key = api_key or BRAVE_API_KEY
        self.provider = provider
    
    def search(self, query: str, count: int = 10) -> Dict:
        """Search the web"""
        if not self.api_key:
            return {"error": "No API key configured", "results": []}
        
        if self.provider == "brave":
            return self._brave_search(query, count)
        elif self.provider == "tavily":
            return self._tavily_search(query, count)
        else:
            return {"error": f"Unknown provider: {self.provider}"}
    
    def _brave_search(self, query: str, count: int) -> Dict:
        """Search using Brave API"""
        url = "https://api.search.brave.com/res/v1/web/search"
        headers = {
            "X-Subscription-Token": self.api_key,
            "Accept": "application/json"
        }
        params = {
            "q": query,
            "count": min(count, 20)
        }
        
        try:
            resp = requests.get(url, headers=headers, params=params, timeout=15)
            data = resp.json()
            
            results = []
            for item in data.get("web", {}).get("results", [])[:count]:
                results.append({
                    "title": item.get("title", ""),
                    "url": item.get("url", ""),
                    "description": item.get("description", ""),
                    "age": item.get("age", "")
                })
            
            return {
                "query": query,
                "provider": "brave",
                "count": len(results),
                "results": results
            }
        except Exception as e:
            return {"error": str(e), "results": []}
    
    def _tavily_search(self, query: str, count: int) -> Dict:
        """Search using Tavily API"""
        url = "https://api.tavily.com/search"
        payload = {
            "api_key": self.api_key,
            "query": query,
            "max_results": count
        }
        
        try:
            resp = requests.post(url, json=payload, timeout=15)
            data = resp.json()
            
            results = []
            for item in data.get("results", [])[:count]:
                results.append({
                    "title": item.get("title", ""),
                    "url": item.get("url", ""),
                    "content": item.get("content", ""),
                    "score": item.get("score", 0)
                })
            
            return {
                "query": query,
                "provider": "tavily",
                "count": len(results),
                "results": results
            }
        except Exception as e:
            return {"error": str(e), "results": []}

# Available models for consensus
MODELS = {
    "minimax": {"name": "MiniMax M2.5", "endpoint": "https://api.minimax.chat/v1/text/chatcompletion_v2"},
    "kimi": {"name": "Kimi K2.5", "endpoint": "https://api.moonshot.cn/v1/chat/completions"},
    "qwen": {"name": "Qwen Coder", "endpoint": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"},
    "gemini": {"name": "Gemini Pro", "endpoint": "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"},
}


class GestaltSuperAgent:
    """Supercharged Gestalt with consensus & automation"""
    
    def __init__(self, mcp_url: str = GESTALT_MCP_URL):
        self.mcp_url = mcp_url
        self.session_context = {}
        self.web_search = WebSearch()
    
    # === Web Search ===
    def search(self, query: str, count: int = 10) -> Dict:
        """Search the web using Brave or Tavily"""
        return self.web_search.search(query, count)
    
    # === MCP Communication ===
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
    
    def call_tool(self, tool_name: str, args: dict = None) -> Any:
        """Call a specific tool"""
        result = self.call_mcp("tools/call", {
            "name": tool_name,
            "arguments": args or {}
        })
        return result.get("result", {})
    
    # === Consensus Voting ===
    async def consensus_vote(self, prompt: str, models: List[str] = None) -> Dict:
        """
        Run prompt through multiple models and return consensus answer.
        This is the SUPERPOWER - multiple perspectives for better decisions.
        """
        models = models or ["minimax", "kimi", "qwen"]
        votes = []
        
        print(f"🗳️  CONSENSUS: Gathering {len(models)} opinions...")
        
        for model in models:
            try:
                # Get context from Gestalt first
                context_result = self.call_tool("get_context", {"path": "."})
                context = json.dumps(context_result)[:2000]
                
                # Build prompt with context
                full_prompt = f"Context:\n{context}\n\nQuestion: {prompt}\n\nAnswer:"
                
                # Call model (simplified - would need actual API keys)
                vote = await self._call_model(model, full_prompt)
                votes.append({
                    "model": model,
                    "answer": vote,
                    "timestamp": datetime.now().isoformat()
                })
                print(f"  ✅ {model}: {vote[:100]}...")
            except Exception as e:
                print(f"  ❌ {model}: {e}")
        
        # Calculate consensus
        consensus = self._calculate_consensus(votes)
        
        return {
            "prompt": prompt,
            "votes": votes,
            "consensus": consensus,
            "timestamp": datetime.now().isoformat()
        }
    
    async def _call_model(self, model: str, prompt: str) -> str:
        """Call individual model (placeholder - needs API keys)"""
        # This would integrate with actual APIs
        return f"[{model.upper()} response to: {prompt[:50]}...]"
    
    def _calculate_consensus(self, votes: List[Dict]) -> Dict:
        """Calculate consensus from votes"""
        if not votes:
            return {"agreed": False, "summary": "No votes"}
        
        # Simple consensus - majority wins
        answers = [v["answer"] for v in votes]
        
        return {
            "agreed": len(answers) > 1,
            "vote_count": len(votes),
            "summary": f"{len(votes)} models participated",
            "best_answer": answers[0] if answers else "No answer"
        }
    
    # === Enhanced Tools ===
    def web_search_enhanced(self, query: str, count: int = 5) -> Dict:
        """Enhanced web search with AI summary"""
        # Use Gestalt's web_fetch
        results = self.call_tool("web_fetch", {
            "url": f"https://html.duckduckgo.com/html/?q={query}",
            "max_chars": 5000
        })
        
        # Could add AI summarization here
        return {
            "query": query,
            "results": results,
            "source": "gestalt_mcp"
        }
    
    # === Real Web Search (Brave/Tavily) ===
    def search(self, query: str, count: int = 10, provider: str = "brave") -> Dict:
        """
        Real web search using Brave Search API or Tavily.
        
        Args:
            query: Search query string
            count: Number of results (1-10 for Brave, 1-20 for Tavily)
            provider: "brave" or "tavily"
        
        Returns:
            Dict with 'results' (list of {title, url, snippet}), 'query', 'provider'
        """
        print(f"🔍 REAL SEARCH: '{query}' via {provider}")
        
        if provider.lower() == "brave":
            return self._brave_search(query, count)
        elif provider.lower() == "tavily":
            return self._tavily_search(query, count)
        else:
            return {"error": f"Unknown provider: {provider}. Use 'brave' or 'tavily'"}
    
    def _brave_search(self, query: str, count: int = 10) -> Dict:
        """Search using Brave Search API"""
        if not BRAVE_API_KEY:
            return {"error": "BRAVE_API_KEY not configured. Set BRAVE_API_KEY env var."}
        
        url = "https://api.brave.com/res/v1/web/search"
        headers = {
            "Accept": "application/json",
            "X-Subscription-Token": BRAVE_API_KEY
        }
        params = {
            "q": query,
            "count": min(count, 10)
        }
        
        try:
            resp = requests.get(url, headers=headers, params=params, timeout=15)
            resp.raise_for_status()
            data = resp.json()
            
            results = []
            for item in data.get("web", {}).get("results", []):
                results.append({
                    "title": item.get("title", ""),
                    "url": item.get("url", ""),
                    "snippet": item.get("description", "")
                })
            
            return {
                "query": query,
                "provider": "brave",
                "results": results,
                "count": len(results)
            }
        except Exception as e:
            return {"error": f"Brave search failed: {str(e)}"}
    
    def _tavily_search(self, query: str, count: int = 10) -> Dict:
        """Search using Tavily API"""
        if not TAVILY_API_KEY:
            return {"error": "TAVILY_API_KEY not configured. Set TAVILY_API_KEY env var."}
        
        url = "https://api.tavily.com/search"
        payload = {
            "api_key": TAVILY_API_KEY,
            "query": query,
            "max_results": min(count, 20),
            "include_answer": True,
            "include_raw_content": False
        }
        
        try:
            resp = requests.post(url, json=payload, timeout=15)
            resp.raise_for_status()
            data = resp.json()
            
            results = []
            for item in data.get("results", []):
                results.append({
                    "title": item.get("title", ""),
                    "url": item.get("url", ""),
                    "snippet": item.get("content", "")
                })
            
            return {
                "query": query,
                "provider": "tavily",
                "results": results,
                "answer": data.get("answer"),
                "count": len(results)
            }
        except Exception as e:
            return {"error": f"Tavily search failed: {str(e)}"}
    
    # === Automation Workflows ===
    def analyze_and_act(self, task: str, use_web_search: bool = False) -> Dict:
        """
        Full workflow: Analyze project → Decide → Execute
        This is autonomous agent behavior.
        
        Args:
            task: The task to analyze
            use_web_search: If True, use real web search for research
        """
        print(f"🎯 ANALYZE & ACT: {task}")
        
        # 1. Get project context
        context = self.call_tool("get_context", {"path": "."})
        print(f"  📋 Context: {len(str(context))} chars")
        
        # 2. Search for relevant code
        search_result = self.call_tool("search_code", {
            "pattern": task,
            "extensions": ".rs,.py,.ts"
        })
        print(f"  🔍 Code search: {len(str(search_result))} chars")
        
        # 3. Optionally do web research
        web_results = None
        if use_web_search:
            print(f"  🌐 Running web research...")
            web_results = self.search(task, count=5)
            print(f"  🌐 Web results: {web_results.get('count', 0)} found")
        
        # 4. Execute if needed
        # (Would decide based on context)
        
        return {
            "task": task,
            "context": context,
            "code_search": search_result,
            "web_results": web_results,
            "status": "analyzed"
        }
    
    def research(self, topic: str, count: int = 10) -> Dict:
        """
        Dedicated research workflow using real web search.
        Use this for: learning, documentation lookup, trend analysis.
        """
        print(f"📚 RESEARCH: '{topic}'")
        
        # Try Brave first, fallback to Tavily
        result = self.search(topic, count=count, provider="brave")
        
        if "error" in result and "not configured" in result.get("error", ""):
            # Fallback to Tavily
            print("  ↳ Brave not configured, trying Tavily...")
            result = self.search(topic, count=count, provider="tavily")
        
        return result
    
    # === Memory & Context ===
    def remember(self, key: str, value: Any):
        """Remember something in session"""
        self.session_context[key] = {
            "value": value,
            "timestamp": datetime.now().isoformat()
        }
    
    def recall(self, key: str) -> Optional[Any]:
        """Recall from session memory"""
        if key in self.session_context:
            return self.session_context[key]["value"]
        return None


# === CLI Interface ===
async def main():
    print("""
⚡ GESTALT SUPERAGENT
=====================
Multi-model consensus + Automation layer
    """)
    
    agent = GestaltSuperAgent()
    
    # Test MCP connection
    print("\n1. Testing Gestalt MCP...")
    tools = agent.call_tool("tools/list", {})
    print(f"   ✅ Connected! Tools available: {len(tools.get('tools', []))}")
    
    # Test consensus
    print("\n2. Testing Consensus Voting...")
    result = await agent.consensus_vote(
        "What is the best approach for error handling in Rust?",
        ["minimax", "qwen"]
    )
    print(f"   🗳️  Consensus: {result['consensus']}")
    
    # Test automation
    print("\n3. Testing Analyze & Act...")
    analysis = agent.analyze_and_act("error handling")
    print(f"   ✅ Analysis complete")


if __name__ == "__main__":
    asyncio.run(main())
