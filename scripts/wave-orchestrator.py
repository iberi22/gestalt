#!/usr/bin/env python3
"""
Wave Orchestrator — Emula el patrón de oleadas de Jules pero en LOCAL.
Prueba el ciclo completo Gestalt ↔ Xavier:

  PRE:  Buscar contexto en Xavier (memoria relevante)
  RUN:  Lanzar subagentes (agy, kimi, cursor-agent, opencode)
  POST: Almacenar resultados en Xavier

Uso: XAVIER_TOKEN=<token> python3 wave-orchestrator.py
"""

import json
import os
import subprocess
import sys
import time
import urllib.request
import urllib.error
from datetime import datetime, timezone
from dataclasses import dataclass, field, asdict
from typing import Optional

XAVIER_URL = os.environ.get("XAVIER_URL", "http://localhost:8006")
XAVIER_TOKEN = os.environ.get("XAVIER_TOKEN", "")

# ─── Subagentes disponibles ────────────────────────────────────────
SUBAGENTS = {}

# Check agy
agy_path = os.path.expanduser("~/.local/bin/agy")
if os.path.isfile(agy_path) and os.access(agy_path, os.X_OK):
    SUBAGENTS["agy"] = {
        "cmd": agy_path,
        "args": [],
        "task_position": "via_flag",  # agy --print "task" --model X --effort high
        "task_flag": "--print",
        "suffix_args": ["--model", "gemini-3.6-flash-high", "--effort", "high"],
        "description": "Google Gemini 3.6 Flash — implementación",
    }

# Check kimi
kimi_result = subprocess.run(
    ["which", "kimi"], capture_output=True, text=True, timeout=5
)
if kimi_result.returncode == 0:
    SUBAGENTS["kimi"] = {
        "cmd": "kimi",
        "args": [],
        "task_position": "via_flag",  # kimi -p "task" -m kimi-code/k3 ...
        "base_args": [],  # no positional flags, use -p
        "task_flag": "-p",  # kimi -p "task" -m kimi-code/k3 ...
        "suffix_args": ["-m", "kimi-code/k3", "--output-format", "text"],
        "description": "Moonshot Kimi K3 — diseño/review",
    }

# Check cursor-agent
ca_result = subprocess.run(
    ["which", "cursor-agent"], capture_output=True, text=True, timeout=5
)
if ca_result.returncode == 0:
    SUBAGENTS["cursor-agent"] = {
        "cmd": "cursor-agent",
        "args": [],
        "task_position": "last",  # cursor-agent --model X --print -f "task"
        "base_args": ["--model", "cursor-grok-4.5-low", "--print", "-f"],
        "description": "Cursor Agent (Grok 4.5 LOW) — implementación",
    }

# Check opencode
oc_result = subprocess.run(
    ["which", "opencode"], capture_output=True, text=True, timeout=5
)
if oc_result.returncode == 0:
    SUBAGENTS["opencode"] = {
        "cmd": "opencode",
        "args": [],
        "task_position": "last",  # opencode run "task"
        "base_args": ["run"],
        "description": "OpenCode CLI — implementación general",
    }


@dataclass
class WaveResult:
    wave: int
    name: str
    agent: str
    task: str
    status: str  # success | failure | timeout
    duration_sec: float
    xavier_pre_results: int = 0
    xavier_post_id: str = ""
    output: str = ""
    error: str = ""


# ─── Xavier API helpers ────────────────────────────────────────────

def xavier_request(method: str, path: str, body: Optional[dict] = None) -> dict:
    """Make a request to Xavier API."""
    url = f"{XAVIER_URL}{path}"
    headers = {
        "X-Xavier-Token": XAVIER_TOKEN,
        "Content-Type": "application/json",
    }
    data = json.dumps(body).encode() if body else None
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return {
            "error": f"HTTP {e.code}: {e.read().decode()[:200]}",
            "status": "error",
        }
    except Exception as e:
        return {"error": str(e), "status": "error"}


def xavier_health() -> bool:
    """Check if Xavier is healthy."""
    result = xavier_request("GET", "/health")
    return result.get("status") in ("healthy", "degraded")


def xavier_search_pre(query: str, limit: int = 3) -> list:
    """PRE-execution: fetch context from Xavier."""
    result = xavier_request("POST", "/v1/memories/search", {
        "query": query,
        "limit": limit,
        "mode": "hybrid",
    })
    if "results" in result:
        return result["results"]
    if "error" in result:
        print(f"  ⚠️  Xavier pre-search error: {result['error']}")
    return []


def xavier_store_post(content: str, path: str, kind: str, metadata: dict) -> Optional[str]:
    """POST-execution: store result in Xavier."""
    result = xavier_request("POST", "/v1/memories", {
        "content": content,
        "path": path,
        "kind": kind,
        "metadata": metadata,
    })
    if "id" in result:
        return result["id"]
    if "error" in result:
        print(f"  ⚠️  Xavier post-store error: {result['error']}")
    return None


# ─── Subagent runner ───────────────────────────────────────────────

def run_agent(agent_name: str, task: str, timeout: int = 60) -> tuple[str, str, float]:
    """
    Run a subagent with the given task.
    Returns (stdout, stderr, duration_seconds).
    """
    info = SUBAGENTS[agent_name]
    
    # Build command based on task_position strategy
    task_pos = info.get("task_position", "last")
    if task_pos == "via_flag":
        # kimi -p "task" -m kimi-code/k3 --output-format text
        task_flag = info.get("task_flag", "-p")
        cmd = [info["cmd"]] + info.get("base_args", []) + [task_flag, task] + info.get("suffix_args", [])
    elif task_pos == "after_model":
        # agy --print --model X --effort high "task"
        cmd = [info["cmd"]] + info.get("base_args", []) + [task]
    else:
        # default: task at end
        cmd = [info["cmd"]] + info.get("base_args", []) + [task]

    start = time.time()
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            env={**os.environ, "GESTALT_TASK": task},
        )
        duration = time.time() - start
        return result.stdout, result.stderr, duration
    except subprocess.TimeoutExpired:
        duration = time.time() - start
        return "", f"TIMEOUT after {timeout}s", duration
    except Exception as e:
        duration = time.time() - start
        return "", str(e), duration


# ─── Wave definitions ──────────────────────────────────────────────

WAVES = [
    # Wave 1: Single agent (agy) — base test
    {
        "name": "Q1: Gestalt Router scan",
        "agent": "agy",
        "task": "Analiza la arquitectura de gestalt-router/src/ en 3 líneas: qué módulos tiene y cómo se relacionan",
        "timeout": 60,
    },
    # Wave 2: Two agents parallel (agy + kimi)
    {
        "name": "Q2: XavierClient review",
        "agent": "kimi",
        "task": "Evalúa el diseño del XavierClient en gestalt_core: ¿sigue el patrón correcto de integración? Responde en 3 líneas",
        "timeout": 60,
    },
    {
        "name": "Q3: Wave pattern audit",
        "agent": "agy",
        "task": "Revisa el código de gestalt_cli y gestalt-router para identificar si el ciclo PRE-context → RUN → POST-store funciona correctamente. Responde en 3 líneas",
        "timeout": 60,
    },
    # Wave 3: Three agents (cursor-agent review)
    {
        "name": "Q4: Code quality scan",
        "agent": "cursor-agent",
        "task": "Revisa la calidad del código en gestalt-router/src/router.rs: unsafe blocks, errores comunes, patrones. Responde en 3 líneas",
        "timeout": 90,
    },
    {
        "name": "Q5: Architecture critique",
        "agent": "kimi",
        "task": "Crítica arquitectónica del gestalt_core::application::agent::xavier::XavierClient: ¿es suficientemente robusto para producción? 3 líneas",
        "timeout": 60,
    },
    {
        "name": "Q6: Test coverage check",
        "agent": "agy",
        "task": "Revisa los tests en gestalt-router/src/ ¿Hay suficiente cobertura del ciclo Router+Xavier? 3 líneas",
        "timeout": 60,
    },
    # Wave 4: Full battery (all agents)
    {
        "name": "Q7: Integration review",
        "agent": "opencode",
        "task": "Revisa la integración entre gestalt_cli, gestalt-router y gestalt_core. ¿El XavierClient se usa correctamente en el pipeline? 3 líneas",
        "timeout": 60,
    },
    {
        "name": "Q8: Final wave summary",
        "agent": "agy",
        "task": "Genera un resumen de 3 líneas de la arquitectura Gestalt ↔ Xavier: qué funciona, qué falta, qué mejorar",
        "timeout": 60,
    },
]


# ─── Wave execution ────────────────────────────────────────────────

def print_header(text: str):
    print()
    print("═" * 60)
    print(f"  {text}")
    print("═" * 60)


def run_wave(wave_num: int, wave_defs: list, results: list):
    """Run a set of tasks in parallel (within the wave)."""
    if not wave_defs:
        return

    print_header(f"🌊 Wave {wave_num} — {len(wave_defs)} tareas")

    for w in wave_defs:
        agent = w["agent"]
        task = w["task"]
        timeout = w.get("timeout", 60)
        task_name = w["name"]

        print(f"\n  🚀 [{agent}] {task_name}")
        print(f"     Task: {task[:80]}...")

        # ── PRE: Xavier context fetch ──
        pre_results = xavier_search_pre(task, limit=2)
        print(f"     📖 Xavier PRE: {len(pre_results)} contextos encontrados")

        # ── RUN: Subprocess ──
        stdout, stderr, duration = run_agent(agent, task, timeout)

        # Determine status
        if stderr.startswith("TIMEOUT"):
            status = "timeout"
            output = f"⏰ Timeout after {timeout}s"
        elif stderr and not stdout:
            status = "failure"
            output = f"❌ Error: {stderr[:200]}"
        else:
            status = "success"
            output = stdout.strip() or "(no output)"

        # ── POST: Xavier store ──
        post_id = xavier_store_post(
            content=output,
            path=f"gestalt/wave-test/{agent}/{task_name.lower().replace(':', '').replace(' ', '-')}",
            kind="wave_test",
            metadata={
                "wave": wave_num,
                "agent": agent,
                "task": task_name,
                "duration_sec": round(duration, 2),
                "status": status,
                "wave_test": True,
                "timestamp": datetime.now(timezone.utc).isoformat(),
            },
        )

        result = WaveResult(
            wave=wave_num,
            name=task_name,
            agent=agent,
            task=task,
            status=status,
            duration_sec=round(duration, 2),
            xavier_pre_results=len(pre_results),
            xavier_post_id=post_id or "",
            output=output[:200],
            error=stderr[:200] if stderr else "",
        )
        results.append(result)

        # Print result
        icon = {"success": "✅", "failure": "❌", "timeout": "⏰"}.get(status, "❓")
        print(f"     {icon} {status.upper()} — {duration:.1f}s")
        if post_id:
            print(f"     💾 Xavier POST: {post_id}")
        print(f"     Output: {output[:120]}...")


def print_summary(results: list):
    print_header("📊 Summary — Cycle Test Report")
    successes = sum(1 for r in results if r.status == "success")
    failures = sum(1 for r in results if r.status == "failure")
    timeouts = sum(1 for r in results if r.status == "timeout")
    total = len(results)

    print(f"\n  Total: {total}  ✅ {successes}  ❌ {failures}  ⏰ {timeouts}")
    print(f"  Tiempo total: {sum(r.duration_sec for r in results):.1f}s")
    print(f"  Xavier PRE hits: {sum(r.xavier_pre_results for r in results)}")
    print(f"  Xavier POST stores: {sum(1 for r in results if r.xavier_post_id)}")

    print()
    for r in results:
        icon = {"success": "✅", "failure": "❌", "timeout": "⏰"}.get(r.status, "❓")
        pre_icon = "📖" if r.xavier_pre_results > 0 else "📭"
        post_icon = "💾" if r.xavier_post_id else "🚫"
        print(f"  {icon} {pre_icon} {post_icon}  W{r.wave} {r.agent:14s} {r.name[:45]:45s}  {r.duration_sec:5.1f}s")

    # Full JSON report
    report_path = f"/tmp/gestalt-wave-cycle-report-{int(time.time())}.json"
    with open(report_path, "w") as f:
        json.dump({
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "xavier_url": XAVIER_URL,
            "total_tasks": total,
            "successes": successes,
            "failures": failures,
            "timeouts": timeouts,
            "subagents_available": list(SUBAGENTS.keys()),
            "results": [asdict(r) for r in results],
        }, f, indent=2, default=str)
    print(f"\n  📄 Reporte completo: {report_path}")


# ─── Main ──────────────────────────────────────────────────────────

def main():
    print_header("🧪 Gestalt ↔ Xavier Cycle Wave Test")
    print(f"  Xavier: {XAVIER_URL}")
    print(f"  Subagentes disponibles: {', '.join(SUBAGENTS.keys()) or 'NINGUNO!'}")

    if not SUBAGENTS:
        print("\n  ❌ No hay subagentes disponibles. Abortando.")
        sys.exit(1)

    # Health check
    if not xavier_health():
        print("\n  ❌ Xavier no responde. Abortando.")
        sys.exit(1)
    print("  ✅ Xavier saludable")

    # Check subagent health
    for name in SUBAGENTS:
        info = SUBAGENTS[name]
        try:
            result = subprocess.run(
                [info["cmd"], "--version"],
                capture_output=True, text=True, timeout=10,
            )
            ver = result.stdout.strip()[:30] or "(no version)"
            print(f"  ✅ {name}: {ver}")
        except Exception as e:
            print(f"  ⚠️  {name}: {e}")

    # Run waves
    results = []

    # Wave 1: Single agent
    run_wave(1, WAVES[0:1], results)

    # Wave 2: Two agents
    run_wave(2, WAVES[1:3], results)

    # Wave 3: Three agents
    run_wave(3, WAVES[3:6], results)

    # Wave 4: Full battery
    run_wave(4, WAVES[6:8], results)

    # Summary
    print_summary(results)

    # Final Xavier store with full report
    report_text = json.dumps({
        "successes": sum(1 for r in results if r.status == "success"),
        "failures": sum(1 for r in results if r.status == "failure"),
        "timeouts": sum(1 for r in results if r.status == "timeout"),
        "total": len(results),
        "subagents": list(SUBAGENTS.keys()),
    })
    final_id = xavier_store_post(
        content=report_text,
        path="gestalt/wave-test/final-report",
        kind="wave_report",
        metadata={
            "total_tasks": len(results),
            "successes": sum(1 for r in results if r.status == "success"),
            "failures": sum(1 for r in results if r.status == "failure"),
            "wave_test_complete": True,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        },
    )
    if final_id:
        print(f"\n  💾 Reporte final almacenado en Xavier: {final_id}")

    # Exit with error if any failures
    fail_count = sum(1 for r in results if r.status in ("failure", "timeout"))
    if fail_count > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
