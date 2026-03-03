---
github_issue: 83
title: "[IMPROVEMENT] VFS: Agregar soporte binario y FileWatcher"
labels:
  - enhancement
  - vfs
assignees: []
status: closed
closed_on: 2026-03-03
last_reviewed: 2026-03-03
---

## Objective
Extender VFS con soporte binario y watcher para sincronizacion externa.

## Target file
- `gestalt_timeline/src/services/vfs.rs`
- `gestalt_timeline/src/services/file_manager.rs`

## Acceptance
- [x] `read_bytes(&self, path: &Path) -> Result<Vec<u8>>`
- [x] `write_bytes(&self, path: &Path, data: Vec<u8>, owner: &str) -> Result<()>`
- [x] Trait `FileWatcher` con metodo `watch`.
- [x] Tests unitarios de binario y watcher.
- [x] Documentacion actualizada en trackers de issue.
