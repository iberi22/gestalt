# Build Strategy (Fast Iteration)

This project now uses a minimal default compile graph and optional infrastructure adapters.

## Principles
- Keep `gestalt_core` focused on domain logic and traits.
- Put heavy provider implementations in `gestalt_infra_*` crates.
- Enable expensive integrations only through explicit features.

## Infra Crates
- `gestalt_infra_embeddings`: BERT/candle embedding adapter.
- `gestalt_infra_github`: GitHub Octocrab adapter.

## Timeline Feature Matrix
- default: minimal runtime (no Telegram, no BERT)
- `telegram`: enables Telegram bot service
- `rag-embeddings`: enables BERT embeddings through `gestalt_infra_embeddings`

## Recommended Commands
```bash
# Fast local check (minimal graph)
cargo check-fast

# Full check with optional integrations
cargo check-full

# Core-only iteration
cargo check -p gestalt_core
```

## Notes
- CI/release quality gates should run formatting and tests before artifact build jobs.
- Avoid workspace-wide `--all-targets` loops during local development unless needed.
