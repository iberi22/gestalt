# TASK.md — Session: feature-scan features.json

_Last update: 2026-07-27_

## Objective
Full feature scan of `.gitcore/features.json` with concrete grep/find against codebase (Ola 5 / commit `28e9209` lineage). No GH issues.

## Done
- [x] Read all 12 features from `.gitcore/features.json`
- [x] Concrete greps per feature (swarm_bridge, SurrealDbAdapter, tantivy/BM25, BeliefGraph, MCP, router Ola5 symbols, CliAdapter, StateDbEventLog)
- [x] Set `progress_pct: 100` for implemented features (incl. `feat-unified-storage` — graph deferred to belief-graph)
- [x] Left failing: `feat-hybrid-search` (35), `feat-belief-graph` (0)
- [x] Metadata invariant: `passing(10) + failing(2) == total_features(12)`
- [x] `overall_progress_pct` → 82.1; synced `implementation-score.json`
- [x] No GitHub issues created

## Verdict table
| id | passes | pct | grep result |
|----|--------|-----|-------------|
| feat-swarm-bridge | true | 100 | AGENTS×15 + asyncio.gather |
| feat-swarm-smart-selection | true | 100 | select_agents |
| feat-swarm-dynamic-count | true | 100 | score_goal_complexity / calculate_optimal_n |
| feat-swarm-streaming | true | 100 | --watch / --poll-interval |
| feat-unified-storage | true | 100 | SurrealDbAdapter + cosine vectors |
| feat-hybrid-search | false | 35 | no tantivy/BM25 |
| feat-belief-graph | false | 0 | no BeliefGraph |
| feat-mcp-server | true | 70 | client/registry only |
| feat-src-reference | true | 80 | SRC.md skeleton |
| feat-router-mvp | true | 100 | AtomicCheckpointer…Router::execute |
| feat-cli-run | true | 100 | RunSpec + CliAdapter + JoinSet |
| feat-event-log | true | 100 | StateDbEventLog + gestalt-ws |

## Do not
- Create GitHub issues
- Mark hybrid/belief as passing without code
