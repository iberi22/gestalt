from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional


@dataclass
class BenchmarkRun:
    task_id: str
    agent: str
    execution_time_ms: int
    tokens_used: int
    cost: float
    success_rate: float
    attempts: int
    correctness: float
    efficiency: float
    readability: float
    score: float
    output_excerpt: str
    simulated: bool


def connect(db_path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    ensure_schema(conn)
    return conn


def ensure_schema(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS benchmark_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            agent TEXT NOT NULL,
            execution_time_ms INTEGER NOT NULL,
            tokens_used INTEGER NOT NULL,
            cost REAL NOT NULL,
            success_rate REAL NOT NULL,
            attempts INTEGER NOT NULL,
            correctness REAL NOT NULL,
            efficiency REAL NOT NULL,
            readability REAL NOT NULL,
            score REAL NOT NULL,
            output_excerpt TEXT NOT NULL,
            simulated INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        """
    )
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS rust_benchmarks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_name TEXT NOT NULL,
            metric_value REAL NOT NULL,
            source_file TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        """
    )
    conn.commit()


def insert_run(conn: sqlite3.Connection, run: BenchmarkRun) -> int:
    cur = conn.execute(
        """
        INSERT INTO benchmark_runs (
            task_id, agent, execution_time_ms, tokens_used, cost, success_rate, attempts,
            correctness, efficiency, readability, score, output_excerpt, simulated
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.task_id,
            run.agent,
            run.execution_time_ms,
            run.tokens_used,
            run.cost,
            run.success_rate,
            run.attempts,
            run.correctness,
            run.efficiency,
            run.readability,
            run.score,
            run.output_excerpt,
            1 if run.simulated else 0,
        ),
    )
    conn.commit()
    return int(cur.lastrowid)


def insert_rust_metrics(
    conn: sqlite3.Connection, metrics: Dict[str, float], source_file: str
) -> int:
    rows = 0
    for metric_name, metric_value in metrics.items():
        conn.execute(
            """
            INSERT INTO rust_benchmarks (metric_name, metric_value, source_file)
            VALUES (?, ?, ?)
            """,
            (metric_name, float(metric_value), source_file),
        )
        rows += 1
    conn.commit()
    return rows


def leaderboard(
    conn: sqlite3.Connection, task_id: Optional[str] = None, limit: int = 20
) -> List[sqlite3.Row]:
    where = ""
    params: List[object] = []
    if task_id:
        where = "WHERE task_id = ?"
        params.append(task_id)
    params.append(limit)
    query = f"""
        SELECT
            agent,
            COUNT(*) AS runs,
            ROUND(AVG(score), 2) AS avg_score,
            ROUND(AVG(success_rate), 2) AS avg_success_rate,
            ROUND(AVG(execution_time_ms), 2) AS avg_execution_time_ms,
            ROUND(SUM(cost), 6) AS total_cost,
            MAX(created_at) AS last_run_at
        FROM benchmark_runs
        {where}
        GROUP BY agent
        ORDER BY avg_score DESC, avg_success_rate DESC, runs DESC
        LIMIT ?
    """
    return list(conn.execute(query, params))


def latest_runs(
    conn: sqlite3.Connection, task_id: Optional[str] = None, limit: int = 20
) -> List[sqlite3.Row]:
    where = ""
    params: List[object] = []
    if task_id:
        where = "WHERE task_id = ?"
        params.append(task_id)
    params.append(limit)
    query = f"""
        SELECT *
        FROM benchmark_runs
        {where}
        ORDER BY id DESC
        LIMIT ?
    """
    return list(conn.execute(query, params))

