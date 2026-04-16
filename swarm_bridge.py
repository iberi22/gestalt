#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Gestalt Swarm — Parallel Agent Execution Bridge
Called by OpenClaw skill/tool to execute N agents in parallel.

Usage:
    python swarm_bridge.py --goal "analyze gestalt-rust" --max-agents 10
    python swarm_bridge.py --goal "find todos" --agents 5 --json
    python swarm_bridge.py --goal "analyze gestalt-rust codebase" --dry-run
    python swarm_bridge.py --goal "comprehensive security audit" --max-agents 5 --rate-limit 50
    GESTALT_MAX_AGENTS=20 GESTALT_RATE_LIMIT=50 python swarm_bridge.py --goal "deep analysis"

Streaming mode:
    python swarm_bridge.py --goal "git status" --agents "git_analyzer,git_status,env_check" --watch --timeout 10
    python swarm_bridge.py --goal "analyze code" --watch --output C:\\temp\\swarm_results.json --poll-interval 200
"""

import argparse
import asyncio
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import uuid
from datetime import datetime, timezone
from typing import Any

# ─────────────────────────────────────────────────────────────
# REPO CONFIGURATION
# ─────────────────────────────────────────────────────────────

# Dynamic repository root (absolute path)
REPO_ROOT = os.path.abspath(os.path.dirname(__file__))

# ─────────────────────────────────────────────────────────────
# AGENT DEFINITIONS — Real CLI commands
# ─────────────────────────────────────────────────────────────

AGENTS = [
    {
        "id": "code_analyzer",
        "name": "Code Analyzer",
        "cmd": ["rg", "-c", ".", REPO_ROOT, "-g", "*.rs"],
        "timeout": 15,
    },
    {
        "id": "dep_check",
        "name": "Dependency Check",
        "cmd": ["cargo", "tree", "--manifest-path", os.path.join(REPO_ROOT, "Cargo.toml"), "--depth", "1", "--format", "plain"],
        "timeout": 30,
    },
    {
        "id": "test_runner",
        "name": "Cargo Check",
        "cmd": ["cargo", "check", "--manifest-path", os.path.join(REPO_ROOT, "Cargo.toml"), "--message-format=short"],
        "timeout": 60,
    },
    {
        "id": "git_analyzer",
        "name": "Git Analyzer",
        "cmd": ["git", "-C", REPO_ROOT, "log", "--oneline", "-20"],
        "timeout": 10,
    },
    {
        "id": "file_scanner",
        "name": "File Scanner",
        "cmd": ["rg", "--files", REPO_ROOT],
        "timeout": 10,
    },
    {
        "id": "log_parser",
        "name": "Log Parser",
        "cmd": ["rg", "ERROR", REPO_ROOT, "--type", "log", "-l"],
        "timeout": 10,
    },
    {
        "id": "security_audit",
        "name": "Security Audit",
        "cmd": ["rg", "TODO|FIXME|XXX|unsafe", REPO_ROOT, "-l"],
        "timeout": 15,
    },
    {
        "id": "metrics",
        "name": "Cargo Stats",
        "cmd": ["cargo", "tree", "--manifest-path", os.path.join(REPO_ROOT, "Cargo.toml"), "--depth", "2"],
        "timeout": 20,
    },
    {
        "id": "doc_gen",
        "name": "Doc Generator",
        "cmd": ["rg", "--type", "md", "-l", ".", REPO_ROOT],
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
        "cmd": ["cargo", "check", "--manifest-path", os.path.join(REPO_ROOT, "Cargo.toml")],
        "timeout": 30,
    },
    {
        "id": "git_status",
        "name": "Git Status",
        "cmd": ["git", "-C", REPO_ROOT, "status", "--short"],
        "timeout": 5,
    },
    {
        "id": "find_todos",
        "name": "TODO Finder",
        "cmd": ["rg", "TODO|FIXME|HACK", REPO_ROOT, "-n", "--color", "never"],
        "timeout": 10,
    },
    {
        "id": "rust_files",
        "name": "Rust Files",
        "cmd": ["rg", "--files", REPO_ROOT, "--type", "rs"],
        "timeout": 10,
    },
    {
        "id": "env_check",
        "name": "Env Checker",
        "cmd": ["rg", "^[^#]", os.path.join(REPO_ROOT, ".env.example")],
        "timeout": 5,
    },
]

# ─────────────────────────────────────────────────────────────
# COMPLEXITY SCORING
# ─────────────────────────────────────────────────────────────

def score_goal_complexity(goal: str) -> int:
    """
    Score goal complexity from 1-10 based on word count and keywords.
    """
    words = goal.split()
    word_count = len(words)
    goal_lower = goal.lower()

    # Complex keywords indicate deeper analysis needed
    complex_keywords = [
        "analyze", "security", "audit", "deep", "comprehensive",
        "refactor", "optimize", "benchmark", "performance", "vulnerability",
        "migration", "architecture", "design", "review", "investigation",
    ]
    keyword_count = sum(1 for kw in complex_keywords if kw in goal_lower)

    # Base score from word count (more words = more complex)
    score = min(10, max(1, word_count // 3))

    # Add weight for complex keywords
    score += keyword_count * 2

    return min(10, max(1, score))


def calculate_optimal_n(goal: str, user_max: int, rate_limit: int) -> int:
    """
    Calculate optimal number of parallel agents based on:
    - user_max: user's configured maximum (CLI flag or env)
    - rate_limit: API rate limit (requests per minute)
    - goal complexity: scored 1-10
    """
    complexity = score_goal_complexity(goal)
    from_rate = rate_limit // 10
    return min(user_max, from_rate, max(1, complexity))


# ─────────────────────────────────────────────────────────────
# CONFIGURATION — Environment + CLI
# ─────────────────────────────────────────────────────────────

def get_config() -> dict:
    """
    Load configuration from environment variables.
    CLI flags override these values in main().
    """
    return {
        "max_agents": int(os.environ.get("GESTALT_MAX_AGENTS", "10")),
        "rate_limit": int(os.environ.get("GESTALT_RATE_LIMIT", "100")),
    }


# ─────────────────────────────────────────────────────────────
# SMART AGENT SELECTION
# ─────────────────────────────────────────────────────────────

def select_agents(goal: str) -> list[str]:
    """
    Analyze goal keywords and return list of relevant agent IDs.
    Falls back to code_analyzer + file_scanner if no keywords match.
    """
    goal_lower = goal.lower()

    selected = set()

    # analyze + code → code_analyzer, file_scanner, security_audit
    if "analyze" in goal_lower and "code" in goal_lower:
        selected.update(["code_analyzer", "file_scanner", "security_audit"])

    # deps / dependencies → dep_check, cargo_check
    if any(kw in goal_lower for kw in ["deps", "dependencies", "dependancy"]):
        selected.update(["dep_check", "cargo_check"])

    # git / commit / history → git_analyzer, git_status
    if any(kw in goal_lower for kw in ["git", "commit", "history", "log"]):
        selected.update(["git_analyzer", "git_status"])

    # api / endpoint / http → api_tester
    if any(kw in goal_lower for kw in ["api", "endpoint", "http", "rest"]):
        selected.update(["api_tester"])

    # todo / fixme / hack → find_todos
    if any(kw in goal_lower for kw in ["todo", "fixme", "hack", "xxx"]):
        selected.update(["find_todos"])

    # security / audit / vuln → security_audit, find_todos
    if any(kw in goal_lower for kw in ["security", "audit", "vuln", "vulnerability"]):
        selected.update(["security_audit", "find_todos"])

    # env / environment / config → env_check
    if any(kw in goal_lower for kw in ["env", "environment", "config", "variable"]):
        selected.update(["env_check"])

    # file / list / scan → file_scanner, rust_files
    if any(kw in goal_lower for kw in ["file", "list", "scan", "directory"]):
        selected.update(["file_scanner", "rust_files"])

    # doc / docs / readme → doc_gen
    if any(kw in goal_lower for kw in ["doc", "docs", "readme", "documentation"]):
        selected.update(["doc_gen"])

    # metric / size / binary → metrics
    if any(kw in goal_lower for kw in ["metric", "size", "binary", "stats"]):
        selected.update(["metrics"])

    # If nothing matched, default to code analysis
    if not selected:
        selected.update(["code_analyzer", "file_scanner"])

    return list(selected)


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


async def run_agent_async(agent: dict) -> dict:
    """Run agent in thread pool (to avoid blocking)."""
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(None, run_agent_sync, agent)


async def run_swarm_parallel(goal: str, max_agents: int = 10, selected: list[str] = None, watch: bool = False, output_file: str = None, poll_interval_ms: int = 100, timeout_sec: int = 0) -> dict:
    """Run N agents in parallel, return consolidated JSON.

    When watch=True, writes incremental JSON to output_file as each agent completes.
    """
    start = time.time()
    selected_ids = set(selected) if selected else None

    agents_to_run = [
        a for a in AGENTS
        if selected_ids is None or a["id"] in selected_ids
    ][:max_agents]

    if not agents_to_run:
        return {"error": "No agents to run", "goal": goal}

    # Generate unique run ID and output file for streaming
    run_id = str(uuid.uuid4())[:8]
    if output_file:
        output_path = output_file
    else:
        output_path = os.path.join(tempfile.gettempdir(), f"swarm_{run_id}.json")

    # Track running/completed agents
    running_ids = {a["id"] for a in agents_to_run}
    completed = {}

    def update_stream_file():
        """Write current state to stream file."""
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump({
                "run_id": run_id,
                "completed_count": len(completed),
                "total_count": len(agents_to_run),
                "running": list(running_ids - set(completed.keys())),
                "results": completed
            }, f)

    # Write initial state
    update_stream_file()

    async def run_agent_with_stream(agent: dict) -> dict:
        """Run agent and update stream file on completion."""
        result = await run_agent_async(agent)
        agent_id = agent["id"]
        if watch:
            completed[agent_id] = result
            running_ids.discard(agent_id)
            update_stream_file()
        return result

    # Run all agents concurrently using asyncio
    tasks = [run_agent_with_stream(a) for a in agents_to_run]
    results = await asyncio.gather(*tasks)

    # If not watch mode, write final state for consistency
    if not watch:
        for r in results:
            completed[r["id"]] = r
        update_stream_file()

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
        "stream_file": output_path if watch else None,
    }


# ─────────────────────────────────────────────────────────────
# MAIN
# ─────────────────────────────────────────────────────────────

def main():
    # Ensure stdout supports UTF-8 (Windows compatibility)
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

    parser = argparse.ArgumentParser(description="Gestalt Swarm Bridge")
    parser.add_argument("--goal", type=str, required=True, help="Goal to execute")
    parser.add_argument("--max-agents", type=int, default=None, help="Max agents to run (default: 10)")
    parser.add_argument("--rate-limit", type=int, default=None, help="Rate limit per minute (default: 100)")
    parser.add_argument("--agents", type=str, default=None, help="Comma-separated agent IDs (overrides auto-selection)")
    parser.add_argument("--dry-run", action="store_true", help="Show which agents would be selected without running")
    parser.add_argument("--json", action="store_true", help="Output JSON only")
    parser.add_argument("--quiet", action="store_true", help="Minimal output")
    parser.add_argument("--watch", action="store_true", help="Enable streaming mode: poll output file as agents complete")
    parser.add_argument("--output", type=str, default=None, help="Output file path for streaming mode (default: temp file)")
    parser.add_argument("--poll-interval", type=int, default=100, help="Polling interval in milliseconds for --watch mode (default: 100)")
    parser.add_argument("--timeout", type=int, default=0, help="Max time to wait in --watch mode in seconds (0 = no limit)")
    args = parser.parse_args()

    # Load config: CLI flags override environment variables
    cfg = get_config()
    user_max = args.max_agents if args.max_agents is not None else cfg["max_agents"]
    rate_limit = args.rate_limit if args.rate_limit is not None else cfg["rate_limit"]

    # Smart agent selection or manual override
    if args.agents:
        selected = args.agents.split(",")
    else:
        selected = select_agents(args.goal)

    # Calculate optimal N based on complexity, user max, and rate limit
    # When agents are explicitly specified, run all of them (ignore optimal_n)
    optimal_n = calculate_optimal_n(args.goal, user_max, rate_limit)
    if args.agents:
        optimal_n = max(optimal_n, len(selected))
    complexity = score_goal_complexity(args.goal)

    # Dry run: show selected agents and exit
    if args.dry_run:
        print(f"🐝 Goal: {args.goal}")
        print(f"📋 Selected agents ({len(selected)}): {', '.join(selected)}")
        print(f"Using {optimal_n}/{user_max} agents (rate_limit={rate_limit}, complexity={complexity})")
        return

    if args.watch:
        # Streaming mode: start agents and poll for results
        result = asyncio.run(run_swarm_parallel(
            args.goal, optimal_n, selected,
            watch=True,
            output_file=args.output,
            poll_interval_ms=args.poll_interval,
            timeout_sec=args.timeout
        ))
        stream_file = result.get("stream_file")
        poll_interval_sec = args.poll_interval / 1000.0
        deadline = time.time() + args.timeout if args.timeout > 0 else None

        print(f"🐝 Streaming mode enabled — output: {stream_file}")
        print(f"📊 Polling every {poll_interval_sec:.1f}s | timeout={args.timeout}s")
        print("─" * 60)

        # Poll until all agents done or timeout
        while True:
            if os.path.exists(stream_file):
                with open(stream_file, "r", encoding="utf-8") as f:
                    state = json.load(f)
                completed_count = state.get("completed_count", 0)
                total_count = state.get("total_count", optimal_n)
                running = state.get("running", [])
                results_dict = state.get("results", {})

                print(f"📈 {completed_count}/{total_count} complete | running: {running}")

                if completed_count >= total_count:
                    print("─" * 60)
                    print(f"✅ All {total_count} agents finished!")
                    for agent_id, agent_result in results_dict.items():
                        status_icon = {"success": "✅", "warn": "⚠️", "error": "❌", "timeout": "⏱️"}.get(agent_result.get("status", "?"), "❓")
                        print(f"{status_icon} [{agent_id}] {agent_result.get('name', agent_id)} ({agent_result.get('duration_ms', 0)}ms)")
                    break

            if deadline and time.time() >= deadline:
                print(f"⏱️  Timeout after {args.timeout}s")
                sys.exit(124)

            time.sleep(poll_interval_sec)

        # Exit with error count from final state
        if os.path.exists(stream_file):
            with open(stream_file, "r", encoding="utf-8") as f:
                final_state = json.load(f)
            error_count = sum(1 for r in final_state.get("results", {}).values() if r.get("status") in ("error", "timeout"))
            sys.exit(min(error_count, 255))
        sys.exit(0)

    result = asyncio.run(run_swarm_parallel(args.goal, optimal_n, selected))

    if args.json:
        print(json.dumps(result, indent=2))
    elif args.quiet:
        for agent in result["agents"]:
            status_icon = {"success": "✅", "warn": "⚠️", "error": "❌", "timeout": "⏱️"}.get(agent["status"], "❓")
            print(f"{status_icon} {agent['name']}: {agent['status']} ({agent['duration_ms']}ms)")
    else:
        # Human-readable output
        print(f"🐝 Gestalt Swarm — Goal: {result['goal']}")
        print(f"Using {optimal_n}/{user_max} agents (rate_limit={rate_limit}, complexity={complexity})")
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
