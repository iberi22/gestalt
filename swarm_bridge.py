#!/usr/bin/env python3
"""
Gestalt Swarm — Parallel Agent Execution Bridge
Called by OpenClaw skill/tool to execute N agents in parallel.

Usage:
    python swarm_bridge.py --goal "analyze gestalt-rust" --max-agents 10
    python swarm_bridge.py --goal "find todos" --agents 5 --json
"""

import argparse
import asyncio
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from typing import Any

# ─────────────────────────────────────────────────────────────
# AGENT DEFINITIONS — Real CLI commands
# ─────────────────────────────────────────────────────────────

AGENTS = [
    {
        "id": "code_analyzer",
        "name": "Code Analyzer",
        "cmd": ["rg", "-c", ".", "E:\\scripts-python\\gestalt-rust\\src", "-g", "*.rs"],
        "timeout": 15,
    },
    {
        "id": "dep_check",
        "name": "Dependency Check",
        "cmd": ["cargo", "tree", "--manifest-path", "E:\\scripts-python\\gestalt-rust\\Cargo.toml", "--depth", "1", "--format", "plain"],
        "timeout": 30,
    },
    {
        "id": "test_runner",
        "name": "Cargo Check",
        "cmd": ["cargo", "check", "--manifest-path", "E:\\scripts-python\\gestalt-rust\\Cargo.toml", "--message-format=short"],
        "timeout": 60,
    },
    {
        "id": "git_analyzer",
        "name": "Git Analyzer",
        "cmd": ["git", "-C", "E:\\scripts-python\\gestalt-rust", "log", "--oneline", "-20"],
        "timeout": 10,
    },
    {
        "id": "file_scanner",
        "name": "File Scanner",
        "cmd": ["rg", "--files", "E:\\scripts-python\\gestalt-rust\\src"],
        "timeout": 10,
    },
    {
        "id": "log_parser",
        "name": "Log Parser",
        "cmd": ["rg", "ERROR", "E:\\scripts-python\\gestalt-rust", "--type", "log", "-l"],
        "timeout": 10,
    },
    {
        "id": "security_audit",
        "name": "Security Audit",
        "cmd": ["rg", "TODO|FIXME|XXX|unsafe", "E:\\scripts-python\\gestalt-rust\\src", "-l"],
        "timeout": 15,
    },
    {
        "id": "metrics",
        "name": "Cargo Stats",
        "cmd": ["cargo", "tree", "--manifest-path", "E:\\scripts-python\\gestalt-rust\\Cargo.toml", "--depth", "2"],
        "timeout": 20,
    },
    {
        "id": "doc_gen",
        "name": "Doc Generator",
        "cmd": ["rg", "--type", "md", "-l", ".", "E:\\scripts-python\\gestalt-rust"],
        "timeout": 10,
    },
    {
        "id": "api_tester",
        "name": "API Tester",
        "cmd": ["curl", "-s", "http://localhost:8003/health"],
        "timeout": 5,
    },
    {
        "id": "cargo_check",
        "name": "Cargo Check",
        "cmd": ["cargo", "check", "--manifest-path", "E:\\scripts-python\\gestalt-rust\\Cargo.toml"],
        "timeout": 30,
    },
    {
        "id": "git_status",
        "name": "Git Status",
        "cmd": ["git", "-C", "E:\\scripts-python\\gestalt-rust", "status", "--short"],
        "timeout": 5,
    },
    {
        "id": "find_todos",
        "name": "TODO Finder",
        "cmd": ["rg", "TODO|FIXME|HACK", "E:\\scripts-python\\gestalt-rust\\src", "-n", "--color", "never"],
        "timeout": 10,
    },
    {
        "id": "rust_files",
        "name": "Rust Files",
        "cmd": ["rg", "--files", "E:\\scripts-python\\gestalt-rust\\src", "--type", "rs"],
        "timeout": 10,
    },
    {
        "id": "env_check",
        "name": "Env Checker",
        "cmd": ["rg", "^[^#]", "E:\\scripts-python\\gestalt-rust\\.env.example"],
        "timeout": 5,
    },
]


def run_agent_sync(agent: dict) -> dict:
    """Run a single agent synchronously."""
    start = time.time()
    agent_id = agent["id"]
    agent_name = agent["name"]
    cmd = agent["cmd"]
    timeout = agent.get("timeout", 20)

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            shell=False,
        )
        duration_ms = int((time.time() - start) * 1000)
        return {
            "id": agent_id,
            "name": agent_name,
            "status": "success" if result.returncode == 0 else "warn",
            "returncode": result.returncode,
            "duration_ms": duration_ms,
            "stdout": result.stdout.strip()[:2000],
            "stderr": result.stderr.strip()[:500] if result.stderr else "",
            "lines": result.stdout.strip().split("\n"),
        }
    except subprocess.TimeoutExpired:
        return {
            "id": agent_id,
            "name": agent_name,
            "status": "timeout",
            "duration_ms": int(timeout * 1000),
            "stdout": "",
            "stderr": f"Timeout after {timeout}s",
        }
    except FileNotFoundError as e:
        return {
            "id": agent_id,
            "name": agent_name,
            "status": "error",
            "duration_ms": 0,
            "stdout": "",
            "stderr": f"Command not found: {e}",
        }
    except Exception as e:
        return {
            "id": agent_id,
            "name": agent_name,
            "status": "error",
            "duration_ms": 0,
            "stdout": "",
            "stderr": str(e)[:200],
        }


async def run_swarm_parallel(goal: str, max_agents: int = 10, selected: list[str] = None) -> dict:
    """Run N agents in parallel, return consolidated JSON."""
    start = time.time()
    selected_ids = set(selected) if selected else None

    agents_to_run = [
        a for a in AGENTS
        if selected_ids is None or a["id"] in selected_ids
    ][:max_agents]

    if not agents_to_run:
        return {"error": "No agents to run", "goal": goal}

    # Run all agents concurrently using asyncio
    tasks = [run_agent_async(a) for a in agents_to_run]
    results = await asyncio.gather(*tasks)

    total_ms = int((time.time() - start) * 1000)
    successful = sum(1 for r in results if r["status"] == "success")
    warnings = sum(1 for r in results if r["status"] == "warn")
    errors = sum(1 for r in results if r["status"] in ("error", "timeout"))

    return {
        "goal": goal,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "duration_ms": total_ms,
        "stats": {
            "total": len(results),
            "successful": successful,
            "warnings": warnings,
            "errors": errors,
        },
        "agents": results,
    }


async def run_agent_async(agent: dict) -> dict:
    """Run agent in thread pool (to avoid blocking)."""
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(None, run_agent_sync, agent)


# ─────────────────────────────────────────────────────────────
# MAIN
# ─────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Gestalt Swarm Bridge")
    parser.add_argument("--goal", type=str, required=True, help="Goal to execute")
    parser.add_argument("--max-agents", type=int, default=10, help="Max agents to run")
    parser.add_argument("--agents", type=str, default=None, help="Comma-separated agent IDs")
    parser.add_argument("--json", action="store_true", help="Output JSON only")
    parser.add_argument("--quiet", action="store_true", help="Minimal output")
    args = parser.parse_args()

    selected = args.agents.split(",") if args.agents else None
    result = asyncio.run(run_swarm_parallel(args.goal, args.max_agents, selected))

    if args.json:
        print(json.dumps(result, indent=2))
    elif args.quiet:
        for agent in result["agents"]:
            status_icon = {"success": "✅", "warn": "⚠️", "error": "❌", "timeout": "⏱️"}.get(agent["status"], "❓")
            print(f"{status_icon} {agent['name']}: {agent['status']} ({agent['duration_ms']}ms)")
    else:
        # Human-readable output
        print(f"🐝 Gestalt Swarm — Goal: {result['goal']}")
        print(f"⏱️  Duration: {result['duration_ms']}ms | ✅ {result['stats']['successful']} | ⚠️ {result['stats']['warnings']} | ❌ {result['stats']['errors']}")
        print("─" * 60)
        for agent in result["agents"]:
            status_icon = {"success": "✅", "warn": "⚠️", "error": "❌", "timeout": "⏱️"}.get(agent["status"], "❓")
            print(f"{status_icon} [{agent['id']}] {agent['name']} ({agent['duration_ms']}ms)")
            if agent["stdout"]:
                for line in agent["stdout"].split("\n")[:5]:
                    print(f"   {line}")
            if agent["stderr"]:
                print(f"   ⚠ {agent['stderr'][:100]}")
        print("─" * 60)
        print(f"Total: {result['duration_ms']}ms | {result['stats']['successful']}/{result['stats']['total']} successful")

    # Exit code = number of errors
    sys.exit(min(result["stats"]["errors"], 255))


if __name__ == "__main__":
    main()
