from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List


@dataclass(frozen=True)
class BenchmarkTask:
    task_id: str
    category: str
    prompt: str
    expected_keywords: List[str]
    difficulty: str


def tasks_dir(repo_root: Path) -> Path:
    return repo_root / "benchmarks" / "tasks"


def load_task(repo_root: Path, task_id: str) -> BenchmarkTask:
    path = tasks_dir(repo_root) / f"{task_id}.json"
    if not path.exists():
        raise FileNotFoundError(f"Task '{task_id}' not found at {path}")
    payload = json.loads(path.read_text(encoding="utf-8"))
    return BenchmarkTask(
        task_id=payload["task_id"],
        category=payload["category"],
        prompt=payload["prompt"],
        expected_keywords=list(payload.get("expected_keywords", [])),
        difficulty=payload.get("difficulty", "medium"),
    )


def list_tasks(repo_root: Path) -> Dict[str, BenchmarkTask]:
    result: Dict[str, BenchmarkTask] = {}
    for file in sorted(tasks_dir(repo_root).glob("*.json")):
        payload = json.loads(file.read_text(encoding="utf-8"))
        task = BenchmarkTask(
            task_id=payload["task_id"],
            category=payload["category"],
            prompt=payload["prompt"],
            expected_keywords=list(payload.get("expected_keywords", [])),
            difficulty=payload.get("difficulty", "medium"),
        )
        result[task.task_id] = task
    return result

