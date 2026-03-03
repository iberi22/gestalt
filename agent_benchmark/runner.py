from __future__ import annotations

import time
from dataclasses import dataclass
from typing import Optional

from .adapters import estimate_tokens, run_agent
from .scoring import clamp_score, composite_score
from .storage import BenchmarkRun
from .tasks import BenchmarkTask


@dataclass
class RunOptions:
    price_per_token: float = 0.000002
    attempts: int = 1
    command_template: Optional[str] = None
    timeout_sec: int = 60
    correctness: Optional[float] = None
    efficiency: Optional[float] = None
    readability: Optional[float] = None


def _keyword_correctness(task: BenchmarkTask, output: str) -> float:
    if not task.expected_keywords:
        return 60.0
    lowered = output.lower()
    hits = 0
    for keyword in task.expected_keywords:
        if keyword.lower() in lowered:
            hits += 1
    return clamp_score((hits / len(task.expected_keywords)) * 100.0)


def _efficiency_score(execution_time_ms: int, tokens_used: int) -> float:
    time_penalty = min(60.0, execution_time_ms / 1000.0 * 5.0)
    token_penalty = min(35.0, tokens_used / 200.0)
    return clamp_score(100.0 - time_penalty - token_penalty)


def _readability_score(output: str) -> float:
    if not output.strip():
        return 0.0
    line_count = max(1, len(output.splitlines()))
    avg_line_len = len(output) / line_count
    structure_bonus = 15.0 if "\n" in output else 0.0
    length_penalty = max(0.0, (avg_line_len - 120.0) / 4.0)
    return clamp_score(70.0 + structure_bonus - length_penalty)


def run_task(task: BenchmarkTask, agent: str, options: RunOptions) -> BenchmarkRun:
    started = time.perf_counter()
    result = run_agent(
        agent=agent,
        prompt=task.prompt,
        command_template=options.command_template,
        timeout_sec=options.timeout_sec,
    )
    elapsed_ms = int((time.perf_counter() - started) * 1000)

    joined_text = f"{task.prompt}\n{result.stdout}\n{result.stderr}"
    tokens_used = estimate_tokens(joined_text)
    cost = float(tokens_used) * float(options.price_per_token)
    success_rate = 100.0 if result.exit_code == 0 else 0.0

    correctness = (
        clamp_score(options.correctness)
        if options.correctness is not None
        else _keyword_correctness(task, result.stdout)
    )
    efficiency = (
        clamp_score(options.efficiency)
        if options.efficiency is not None
        else _efficiency_score(elapsed_ms, tokens_used)
    )
    readability = (
        clamp_score(options.readability)
        if options.readability is not None
        else _readability_score(result.stdout)
    )
    score = composite_score(correctness, efficiency, readability, success_rate)

    excerpt = (result.stdout or result.stderr or "").strip().replace("\n", " ")
    if len(excerpt) > 240:
        excerpt = excerpt[:237] + "..."

    return BenchmarkRun(
        task_id=task.task_id,
        agent=agent,
        execution_time_ms=elapsed_ms,
        tokens_used=tokens_used,
        cost=cost,
        success_rate=success_rate,
        attempts=max(1, options.attempts),
        correctness=correctness,
        efficiency=efficiency,
        readability=readability,
        score=score,
        output_excerpt=excerpt,
        simulated=result.simulated,
    )

