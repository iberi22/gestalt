# TODO.md - Gestalt-Rust 1.0.0 Tasks

**Last Updated:** 2026-03-19
**Target:** Gestalt-Rust 1.0.0 Release

---

## 🔴 Critical (Must Complete)

### Documentation

| Task | Priority | Issue | Status |
|------|----------|-------|--------|
| Complete STATE.md | P0 | - | ✅ Done |
| Complete TODO.md | P0 | - | 🔄 In Progress |
| Create CHANGELOG.md | P0 | - | ⬜ Pending |
| Update ARCHITECTURE.md | P1 | - | ⬜ Pending |

### CI/CD

| Task | Priority | Issue | Status |
|------|----------|-------|--------|
| Audit workflow failures | P0 | #87 | ⬜ Pending |
| Fix any flaky tests | P0 | - | ⬜ Pending |

---

## 🟡 High Priority

### Code Quality

| Task | Priority | Notes |
|------|----------|-------|
| Run `cargo clippy --workspace` | P1 | Fix any warnings |
| Run `cargo test --workspace` | P1 | Ensure all tests pass |
| Verify `cargo fmt` | P1 | Run before PR |

### Module Completion

| Task | Priority | Module | Notes |
|------|----------|--------|-------|
| Complete GitHub integration | P1 | gestalt_infra_github | Octocrab implementation |
| Complete embeddings | P1 | gestalt_infra_embeddings | BERT/candle |
| Complete UI components | P2 | gestalt_ui | TBD scope |

---

## 🟢 Medium Priority

### Performance

| Task | Priority | Notes |
|------|----------|-------|
| Benchmark critical paths | P2 | Use agent_benchmark |
| Optimize VFS operations | P2 | May have IO bottlenecks |
| Profile memory usage | P2 | SurrealDB embedded |

### Documentation

| Task | Priority | Notes |
|------|----------|-------|
| Add API documentation | P2 | rustdoc comments |
| Create user guide | P2 | CLI usage |
| Add examples | P2 | Common use cases |

---

## 🔵 Technical Debt

| Item | Priority | Notes |
|------|----------|-------|
| Error handling | P2 | Improve error messages |
| Logging | P2 | Structured logging |
| Config management | P2 | Environment variables |

---

## 📋 1.0.0 Release Checklist

- [ ] All CI workflows passing
- [ ] Documentation complete (STATE.md, TODO.md, CHANGELOG.md)
- [ ] No production blockers
- [ ] `cargo test --workspace --all-targets` passes
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace` passes (no warnings)
- [ ] Benchmarks run successfully
- [ ] CHANGELOG.md updated with release notes

---

## 🔗 Dependencies

### Internal
- `gestalt_core` (stable) → All modules
- `gestalt_timeline` (stable) → CLI, MCP

### External
- `synapse-agentic` - Hive actor model
- `tokio` - Async runtime
- `surrealdb` - Embedded database

---

## 📅 Timeline

| Phase | Target | Tasks |
|-------|--------|-------|
| Documentation | 2026-03-19 | Complete STATE.md, TODO.md, CHANGELOG.md |
| CI Audit | 2026-03-20 | Fix workflow failures (#87) |
| Module Completion | 2026-03-21 | Complete infra modules |
| Testing | 2026-03-22 | Full test suite |
| Release | 2026-03-23 | 1.0.0 |

---

*This file tracks prioritized tasks for 1.0.0 release.*
