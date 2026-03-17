from __future__ import annotations

import os
import shlex
import subprocess
import time
from dataclasses import dataclass
from typing import Optional


@dataclass
class AdapterResult:
    stdout: str
    stderr: str
    exit_code: int
    simulated: bool
    execution_time_ms: int = 0


def estimate_tokens(text: str) -> int:
    words = len(text.split())
    return max(1, int(words * 1.3))


def _env_command(agent: str) -> Optional[str]:
    key = f"AGENT_BENCHMARK_{agent.upper().replace('-', '_')}_CMD"
    return os.getenv(key)


def run_agent(
    agent: str,
    prompt: str,
    command_template: Optional[str] = None,
    timeout_sec: int = 60,
) -> AdapterResult:
    template = command_template or _env_command(agent)

    started = time.perf_counter()
    if not template:
        simulated_text = (
            f"[simulated:{agent}] Completed task based on prompt length={len(prompt)}"
        )
        elapsed_ms = int((time.perf_counter() - started) * 1000)
        return AdapterResult(
            stdout=simulated_text,
            stderr="",
            exit_code=0,
            simulated=True,
            execution_time_ms=elapsed_ms,
        )

    command_str = template.replace("{prompt}", prompt.replace('"', '\\"'))
    args = shlex.split(command_str, posix=False)
    proc = subprocess.run(
        args,
        capture_output=True,
        text=True,
        timeout=timeout_sec,
        check=False,
    )
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    return AdapterResult(
        stdout=proc.stdout,
        stderr=proc.stderr,
        exit_code=proc.returncode,
        simulated=False,
        execution_time_ms=elapsed_ms,
    )

