# gestalt-wasm Architecture & Build Instructions

This crate provides the WebAssembly interface and utilities for the Gestalt framework. It supports conditional compilation for both native environments and the `wasm32-unknown-unknown` target.

## Feature Gates

The crate defines two feature gates in its `Cargo.toml`:

- `native` (enabled by default): For native builds of the Gestalt framework.
- `wasm`: For target compilation on WebAssembly platforms (`wasm32-unknown-unknown`).

## Build Instructions

To build or verify the compilation of the WebAssembly module, follow the steps below.

### Prerequisites

Ensure you have added the `wasm32-unknown-unknown` target to your local toolchain:

```bash
rustup target add wasm32-unknown-unknown
```

### Checking Compilation

You can run a fast compilation check for the WASM target using `cargo check`:

```bash
cargo check --target wasm32-unknown-unknown -p gestalt-wasm
```

### Building the Crate

To build the crate for the WebAssembly target:

```bash
cargo build --target wasm32-unknown-unknown -p gestalt-wasm --release
```
