# gestalt-swarm Skill

## Estado

⛔ **EXCLUIDO del Cargo workspace** — `gestalt_swarm` ya no es miembro de `Cargo.toml`.
No uses `cargo build/run/test -p gestalt_swarm` (falla fuera del workspace).

## Alternativas operativas

```bash
# Bridge Python (paralelo)
python scripts/swarm_bridge.py --goal "analyze codebase"

# Swarm vía CLI (workspace activo)
cargo run --release -p gestalt_cli -- swarm --help
```

## Legacy

El directorio `gestalt_swarm/` permanece en el repo como código legacy (no compilado por `--workspace`).
No reincorporarlo a `Cargo.toml` sin migración explícita (ver `AGENTS.md`).

---

*Gestalt Swarm — legacy; use swarm_bridge / gestalt_cli.*
