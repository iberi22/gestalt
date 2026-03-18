# 📝 TODO.md - Pending Tasks

## 🚀 PHASE 1: OpenClaw Integration (50% 🔄)
- [x] **Gestalt Bridge**: Implementation of `gestalt_bridge.py` and MCP tools.
- [ ] **MiniMax Integration**: Configure API key and verify model `MiniMax-M2.1` in production.
- [ ] **Refinement**: Improve search semantics in `gestalt_superagent.py`.

## 🧪 Testing & Validation
- [ ] **Granular Benchmark Coverage**: Add specific tests for `gestalt_infra_embeddings` (BERT/Candle).
- [ ] **E2E VFS Scenarios**: Verify multi-agent conflict resolution in complex workspace structures.
- [ ] **Performance Audit**: Profile SurrealDB WebSocket latencies under high concurrency.

## 🛠️ Infrastructure Improvements
- [ ] **Auto-Recovery**: Enhance `AgentRuntime` supervisor logic to handle database disconnection with exponential backoff.
- [ ] **Dynamic Plugin System**: Formalize the mechanism for external agents to register tools via MCP at runtime.
- [ ] **Cloud Sync**: Optional cloud persistence for the timeline service.

## 📦 Maintenance
- [ ] **Documentation**: Complete API reference for `gestalt_core` traits using `cargo doc`.
- [ ] **CI Optimizations**: Reduce cache miss rate in GitHub Actions for faster feedback loops.

---
*Last Update: 2026-03-03*
*Status: Synchronized with .gitcore/planning/TASK.md*
