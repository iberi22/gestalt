# Benchmarking Gestalt

This document describes how to run benchmarks and manage performance baselines.

## Prerequisites

- Rust (Cargo)
- Python 3

## Running Benchmarks

### Rust Core Benchmarks
To run the core Rust benchmarks:
```bash
cargo bench -p gestalt_core
```
Results are saved to `benchmarks/rust_current.json`.

### Python Memory Benchmarks
To run the memory system benchmarks:
```bash
python skills/benchmark_memory.py
```
Results are saved to `benchmarks/memory_current.json`.

## Regression Detection

After running both benchmark suites, you can compare the results against the baseline:
```bash
python scripts/compare_benchmarks.py
```
The threshold for regression alerts can be adjusted via the `BENCHMARK_THRESHOLD` environment variable (default is `1.10` for a 10% tolerance).

## Updating Baseline

If performance changes are expected and acceptable, update the baseline by copying the current results:
```bash
cp benchmarks/memory_current.json benchmarks/baseline/memory_baseline.json
cp benchmarks/rust_current.json benchmarks/baseline/rust_baseline.json
```
