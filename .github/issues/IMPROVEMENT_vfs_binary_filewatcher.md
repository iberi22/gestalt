---
github_issue: 83
title: "[IMPROVEMENT] VFS: Agregar soporte binario y FileWatcher"
labels:
  - enhancement
  - vfs
assignees: []
status: open
last_reviewed: 2026-03-03
---

## Objective
Extender VFS con soporte binario y watcher para sincronizacion externa.

## Target file
- `gestalt_timeline/src/services/vfs.rs`

## Acceptance
- [ ] `read_bytes(&self, path: &Path) -> Result<Vec<u8>>`
- [ ] `write_bytes(&self, path: &Path, data: Vec<u8>, owner: &str) -> Result<()>`
- [ ] Trait `FileWatcher` con metodo `watch`.
- [ ] Tests unitarios de binario y watcher.
- [ ] Documentacion actualizada.
