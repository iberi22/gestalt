# Build Strategy

## Incremental Build

```bash
# Fast dev build
cargo build

# Incremental (fastest, debugging)
cargo build --incremental

# Profile for speed
cargo build -p gestalt_timeline
```

## Release Build

```bash
cargo build --release --all
```

## Check (without linking)

```bash
cargo check --all
```

## Lint

```bash
cargo fmt --check
cargo clippy --all -- -D warnings
```

## Test

```bash
cargo test --all
```

## Individual Crate

```bash
cargo build -p gestalt_core
cargo test -p gestalt_cli
cargo check -p gestalt_swarm
```
