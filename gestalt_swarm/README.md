# Gestalt Swarm (legacy)

⛔ **Excluded from the Cargo workspace** — not listed in root `Cargo.toml` members.
Do not run `cargo -p gestalt_swarm` from the Gestalt workspace; it will not resolve as a package.

## Current alternatives

```bash
python scripts/swarm_bridge.py --goal "Refactor unwrap() in gestalt_core"
cargo run --release -p gestalt_cli -- swarm --help
```

## Standalone (legacy only)

To build this crate in isolation (outside workspace membership), use a dedicated manifest / path build — not `cargo --workspace`. Prefer migration into `gestalt_cli` / router instead of re-adding the member.

## Architecture

Swarm originally used a lead agent to decompose goals into parallel worker tasks with VFS isolation. That role is now covered by workspace members (`gestalt_cli`, `gestalt-router`) and `scripts/swarm_bridge.py`.
