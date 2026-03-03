from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict

from .api import latest as api_latest
from .api import leaderboard as api_leaderboard
from .runner import RunOptions, run_task
from .storage import connect, insert_run, insert_rust_metrics
from .tasks import list_tasks, load_task


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def default_db_path() -> Path:
    return repo_root() / "benchmarks" / "agent_benchmark.sqlite"


def _print_json(payload: Dict[str, object]) -> None:
    print(json.dumps(payload, indent=2, ensure_ascii=False))


def cmd_run(args: argparse.Namespace) -> int:
    root = repo_root()
    task = load_task(root, args.task)
    options = RunOptions(
        price_per_token=args.price_per_token,
        attempts=args.attempts,
        command_template=args.command,
        timeout_sec=args.timeout_sec,
        correctness=args.correctness,
        efficiency=args.efficiency,
        readability=args.readability,
    )
    run = run_task(task=task, agent=args.agent, options=options)

    db_path = Path(args.db)
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = connect(db_path)
    try:
        run_id = insert_run(conn, run)
    finally:
        conn.close()

    _print_json(
        {
            "run_id": run_id,
            "task_id": run.task_id,
            "agent": run.agent,
            "execution_time_ms": run.execution_time_ms,
            "tokens_used": run.tokens_used,
            "cost": run.cost,
            "success_rate": run.success_rate,
            "attempts": run.attempts,
            "correctness": run.correctness,
            "efficiency": run.efficiency,
            "readability": run.readability,
            "score": run.score,
            "simulated": run.simulated,
            "output_excerpt": run.output_excerpt,
            "db": str(db_path),
        }
    )
    return 0


def cmd_leaderboard(args: argparse.Namespace) -> int:
    rows = api_leaderboard(Path(args.db), task_id=args.task, limit=args.limit)
    _print_json({"task": args.task, "rows": rows})
    return 0


def cmd_latest(args: argparse.Namespace) -> int:
    rows = api_latest(Path(args.db), task_id=args.task, limit=args.limit)
    _print_json({"task": args.task, "rows": rows})
    return 0


def cmd_list_tasks(args: argparse.Namespace) -> int:
    root = repo_root()
    tasks = list_tasks(root)
    payload = {
        task_id: {
            "category": task.category,
            "difficulty": task.difficulty,
            "expected_keywords": task.expected_keywords,
        }
        for task_id, task in tasks.items()
    }
    _print_json({"tasks": payload})
    return 0


def cmd_sync_rust(args: argparse.Namespace) -> int:
    source = Path(args.file)
    if not source.exists():
        fallback = repo_root() / "benchmarks" / "baseline" / "rust_baseline.json"
        if fallback.exists():
            source = fallback
    if not source.exists():
        raise FileNotFoundError(f"Rust benchmark file not found: {source}")
    payload = json.loads(source.read_text(encoding="utf-8"))
    metrics = {str(k): float(v) for k, v in payload.items()}

    db_path = Path(args.db)
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = connect(db_path)
    try:
        inserted = insert_rust_metrics(conn, metrics, str(source))
    finally:
        conn.close()

    _print_json({"inserted_metrics": inserted, "db": str(db_path), "source": str(source)})
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="agent_benchmark")
    parser.set_defaults(func=lambda _args: parser.print_help() or 0)

    sub = parser.add_subparsers(dest="command")

    run_p = sub.add_parser("run", help="Run one benchmark task for an agent")
    run_p.add_argument("--task", required=True, help="Task id (e.g. code_sum_001)")
    run_p.add_argument("--agent", required=True, help="Agent name (codex/gemini/jules/...)")
    run_p.add_argument(
        "--command",
        help="Optional command template. Use {prompt} placeholder for prompt injection.",
    )
    run_p.add_argument("--attempts", type=int, default=1)
    run_p.add_argument("--timeout-sec", type=int, default=60)
    run_p.add_argument("--price-per-token", type=float, default=0.000002)
    run_p.add_argument("--correctness", type=float)
    run_p.add_argument("--efficiency", type=float)
    run_p.add_argument("--readability", type=float)
    run_p.add_argument("--db", default=str(default_db_path()))
    run_p.set_defaults(func=cmd_run)

    lb_p = sub.add_parser("leaderboard", help="Show leaderboard")
    lb_p.add_argument("--task", help="Optional task id filter")
    lb_p.add_argument("--limit", type=int, default=20)
    lb_p.add_argument("--db", default=str(default_db_path()))
    lb_p.set_defaults(func=cmd_leaderboard)

    latest_p = sub.add_parser("latest", help="Show latest runs")
    latest_p.add_argument("--task", help="Optional task id filter")
    latest_p.add_argument("--limit", type=int, default=20)
    latest_p.add_argument("--db", default=str(default_db_path()))
    latest_p.set_defaults(func=cmd_latest)

    list_p = sub.add_parser("list-tasks", help="List available benchmark tasks")
    list_p.set_defaults(func=cmd_list_tasks)

    rust_p = sub.add_parser(
        "sync-rust", help="Import metrics from benchmarks/rust_current.json"
    )
    rust_p.add_argument("--file", default=str(repo_root() / "benchmarks" / "rust_current.json"))
    rust_p.add_argument("--db", default=str(default_db_path()))
    rust_p.set_defaults(func=cmd_sync_rust)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
