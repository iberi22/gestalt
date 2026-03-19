# Changelog

All notable changes to Gestalt-Rust will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [1.0.0] - TBD

### Added
- Initial documentation structure (STATE.md, TODO.md)
- CHANGELOG.md for release tracking

### Changed
- (No breaking changes yet)

### Deprecated
- (Nothing deprecated yet)

### Removed
- (Nothing removed yet)

### Fixed
- (No fixes yet)

### Security
- (No security changes yet)

---

## [0.1.0] - 2026-03-03

### Added
- `gestalt_core` - Generic logic module (no IO)
- `gestalt_cli` - Command-line interface with Clap + Rustyline
- `gestalt_timeline` - Tokio + SurrealDB orchestration
- `gestalt_mcp` - Model Context Protocol server
- `gestaltctl` - CLI tool wrapper
- `gestalt_infra_github` - GitHub integration (WIP)
- `gestalt_infra_embeddings` - Embeddings infrastructure (WIP)
- `gestalt_ui` - UI components (WIP)
- `gestalt_app` - Application layer (WIP)
- `gestalt_terminal` - Terminal interface (WIP)
- `agent_benchmark` - Benchmarking module

### Changed
- Architecture decision: Async autonomy with JobId polling
- Architecture decision: Protocol-first tooling
- Architecture decision: Context injection before prompt build
- Architecture decision: VFS overlay for volatile workspace
- Architecture decision: Elastic autonomous loops
- Architecture decision: Hive actor model via synapse-agentic

### Fixed
- Multiple CI workflow optimizations
- Test suite stability improvements

---

## [0.0.1] - 2026-02-01

### Added
- Initial project structure
- GitCore protocol implementation
- Basic agent runtime

---

<!-- Links -->
[1.0.0]: https://github.com/iberi22/gestalt-rust/releases/tag/v1.0.0
[0.1.0]: https://github.com/iberi22/gestalt-rust/releases/tag/v0.1.0
[0.0.1]: https://github.com/iberi22/gestalt-rust/releases/tag/v0.0.1
