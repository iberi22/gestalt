from __future__ import annotations


def clamp_score(value: float) -> float:
    if value < 0.0:
        return 0.0
    if value > 100.0:
        return 100.0
    return value


def composite_score(
    correctness: float,
    efficiency: float,
    readability: float,
    success_rate: float,
) -> float:
    correctness = clamp_score(correctness)
    efficiency = clamp_score(efficiency)
    readability = clamp_score(readability)
    success_rate = clamp_score(success_rate)
    return (
        correctness * 0.4
        + efficiency * 0.2
        + readability * 0.2
        + success_rate * 0.2
    )

