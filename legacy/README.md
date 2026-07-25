# Legacy — Documentos Superseded

> Estos documentos pertenecen a la arquitectura v1.0.0 (pre-Julio 2026).
> La v2.0.0 (Router) reemplazó SurrealDB, FUSE, Timeline y Lock Server.
> Ver `docs/REDESIGN.md` para el diseño actual.

## Archivos Legacy

| Archivo | Reemplazado por |
|---------|----------------|
| TIMELINE_ANALYSIS_REPORT.md | timeline.rs en gestalt-router |
| SRC.md (legacy) | `SRC.md` (raíz) |
| SKILL.md (legacy) | `docs/REDESIGN.md` |
| QUICKSTART.md (legacy) | `README.md` |
| SECURITY_ISSUES.md | Sin reemplazo (no aplica) |
| WALKTHROUGH.md | Sin reemplazo (no aplica) |
| PR_BODY.md | Automatizado en Fase 2 |
| MINIMAX.md | Provider removido |
| Python bridge/* | Reemplazado por SubprocessRunner en Rust |
| vfs_evaluation_report.json | Diseño validado en REDESIGN.md |

## Historial

- **2026-03 a 2026-07**: v1.0.0 — Gestalt como plataforma de agentes con SurrealDB, FUSE, Swarm
- **2026-07-25**: v2.0.0 — Rediseño completo como Codebase Router tras validación AGY + Kimi
