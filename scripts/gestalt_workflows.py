"""
Gestalt Automation Workflows
=============================
Pre-built workflows for common tasks:
1. Code Review - Analyze PRs automatically
2. Bug Hunt - Find and diagnose issues
3. Doc Generator - Generate docs from code
4. Refactor - Suggest improvements
"""
import json
import requests
from typing import Dict, List, Optional
from datetime import datetime

GESTALT_MCP_URL = "http://127.0.0.1:3000"


def call_gestalt(method: str, params: dict = None) -> dict:
    """Call Gestalt MCP"""
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params or {}
    }
    try:
        resp = requests.post(f"{GESTALT_MCP_URL}/mcp", json=payload, timeout=60)
        return resp.json()
    except Exception as e:
        return {"error": str(e)}


# === Workflow 1: Code Review ===
def workflow_code_review(path: str) -> Dict:
    """
    Automated code review workflow:
    1. Get file tree
    2. Analyze code structure
    3. Search for issues
    4. Generate review report
    """
    print(f"🔍 CODE REVIEW: {path}")
    
    # Step 1: Get project structure
    tree = call_gestalt("tools/call", {
        "name": "file_tree",
        "arguments": {"path": path, "depth": 3}
    })
    
    # Step 2: Analyze project
    analysis = call_gestalt("tools/call", {
        "name": "analyze_project",
        "arguments": {"path": path}
    })
    
    # Step 3: Search for common issues
    issues = call_gestalt("tools/call", {
        "name": "search_code",
        "arguments": {
            "pattern": "TODO|FIXME|HACK|XXX",
            "path": path,
            "extensions": ".rs,.py,.ts"
        }
    })
    
    return {
        "workflow": "code_review",
        "path": path,
        "structure": tree,
        "analysis": analysis,
        "issues": issues,
        "timestamp": datetime.now().isoformat()
    }


# === Workflow 2: Bug Hunt ===
def workflow_bug_hunt(path: str, keyword: str) -> Dict:
    """
    Find and diagnose bugs:
    1. Search for error patterns
    2. Check recent changes
    3. Find related code
    """
    print(f"🐛 BUG HUNT: {keyword} in {path}")
    
    # Search for the keyword
    results = call_gestalt("tools/call", {
        "name": "search_code",
        "arguments": {
            "pattern": keyword,
            "path": path,
            "extensions": ".rs,.py,.ts,.js"
        }
    })
    
    # Get git log
    git_log = call_gestalt("tools/call", {
        "name": "git_log",
        "arguments": {"path": path, "count": 10}
    })
    
    return {
        "workflow": "bug_hunt",
        "keyword": keyword,
        "results": results,
        "recent_commits": git_log,
        "timestamp": datetime.now().isoformat()
    }


# === Workflow 3: Doc Generator ===
def workflow_generate_docs(path: str) -> Dict:
    """
    Generate documentation:
    1. Get file structure
    2. Extract README
    3. Generate API docs
    """
    print(f"📝 DOC GENERATOR: {path}")
    
    # Get context
    context = call_gestalt("tools/call", {
        "name": "get_context",
        "arguments": {"path": path}
    })
    
    # List files
    files = call_gestalt("tools/call", {
        "name": "list_files",
        "arguments": {"path": path, "depth": 2}
    })
    
    return {
        "workflow": "doc_generator",
        "path": path,
        "context": context,
        "files": files,
        "timestamp": datetime.now().isoformat()
    }


# === Workflow 4: Refactor Suggestions ===
def workflow_refactor(path: str) -> Dict:
    """
    Suggest refactoring:
    1. Analyze code complexity
    2. Find code smells
    3. Suggest improvements
    """
    print(f"🔧 REFACTOR: {path}")
    
    # Get code statistics
    analysis = call_gestalt("tools/call", {
        "name": "analyze_project",
        "arguments": {"path": path}
    })
    
    return {
        "workflow": "refactor",
        "analysis": analysis,
        "suggestions": [
            "Consider extracting repeated code into functions",
            "Look for opportunities to use traits for polymorphism",
            "Check for unused imports and variables"
        ],
        "timestamp": datetime.now().isoformat()
    }


# === Main Runner ===
def run_workflow(workflow_name: str, path: str, **kwargs) -> Dict:
    """Run a named workflow"""
    workflows = {
        "code_review": workflow_code_review,
        "bug_hunt": lambda p: workflow_bug_hunt(p, kwargs.get("keyword", "error")),
        "docs": workflow_generate_docs,
        "refactor": workflow_refactor
    }
    
    if workflow_name not in workflows:
        return {"error": f"Unknown workflow: {workflow_name}"}
    
    return workflows[workflow_name](path)


if __name__ == "__main__":
    import sys
    
    if len(sys.argv) < 3:
        print("""
Gestalt Workflows
==================
Usage: python workflows.py <workflow> <path> [options]

Workflows:
  code_review <path>    - Review code automatically
  bug_hunt <path> <kw>  - Hunt for bugs/keywords
  docs <path>           - Generate documentation
  refactor <path>       - Suggest refactoring

Examples:
  python workflows.py code_review E:\\scripts-python\\myproject
  python workflows.py bug_hunt E:\\scripts-python\\myproject panic
  python workflows.py docs E:\\scripts-python\\myproject
        """)
        sys.exit(1)
    
    workflow = sys.argv[1]
    path = sys.argv[2]
    keyword = sys.argv[3] if len(sys.argv) > 3 else None
    
    result = run_workflow(workflow, path, keyword=keyword)
    print(json.dumps(result, indent=2))
