## Objective
Calculate optimal number of parallel agents based on rate limits, user config, and goal complexity.

## Formula
```
N = min(
  user_config_max_agents,
  rate_limit_tokens // tokens_per_agent,
  complexity_score(goal)
)
```

## Implementation
```python
def calculate_optimal_n(goal: str, config: dict) -> int:
    user_max = config.get("max_agents", 20)
    complexity = score_goal_complexity(goal)  # 1-10
    rate_limit = config.get("rate_limit", 100)
    
    return min(user_max, rate_limit // 10, complexity * 2)
```

## Acceptance Criteria
- [ ] Config `max_agents` in skill config
- [ ] Rate limit awareness (configurable)
- [ ] Complexity scoring for goals (token-based estimation)
- [ ] `--dry-run` shows calculated N before execution

## Priority
**MEDIUM** — Enables safe scaling to 100+ agents.
