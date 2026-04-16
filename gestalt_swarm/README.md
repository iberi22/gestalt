# Gestalt Swarm

Gestalt Swarm is a high-throughput parallel execution bridge for AI agent tasks. It is designed to run many short-lived tasks in parallel, making it ideal for large-scale codebase analysis or refactoring.

## CLI Usage

The `swarm` command provides several subcommands for managing and running parallel tasks.

### 1. Check Status
Verify that the swarm is active and ready to accept tasks.

```bash
cargo run -p gestalt_swarm -- status
```

### 2. Run a Task
Submit a goal to the swarm for parallel execution.

```bash
cargo run -p gestalt_swarm -- run --goal "Refactor all unwrap() calls in gestalt_core"
```

### 3. Verbose Output
Use the `--verbose` or `-v` flag to enable debug logging.

```bash
cargo run -p gestalt_swarm -- -v status
```

## Architecture

Swarm utilizes a lead agent to decompose complex goals into smaller, independent tasks which are then dispatched to a pool of worker agents. It leverages the Virtual File System (VFS) to ensure that parallel modifications do not conflict and can be merged safely.
