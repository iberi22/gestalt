from __future__ import annotations

from pathlib import Path
from typing import Dict, List, Optional

from .storage import connect, leaderboard as query_leaderboard, latest_runs


def leaderboard(
    db_path: Path, task_id: Optional[str] = None, limit: int = 20
) -> List[Dict[str, object]]:
    conn = connect(db_path)
    try:
        rows = query_leaderboard(conn, task_id=task_id, limit=limit)
        return [dict(row) for row in rows]
    finally:
        conn.close()


def latest(
    db_path: Path, task_id: Optional[str] = None, limit: int = 20
) -> List[Dict[str, object]]:
    conn = connect(db_path)
    try:
        rows = latest_runs(conn, task_id=task_id, limit=limit)
        return [dict(row) for row in rows]
    finally:
        conn.close()

