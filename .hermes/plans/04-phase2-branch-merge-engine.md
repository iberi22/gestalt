# Phase 2: BranchManager + MergeEngine (NUEVO)

## Task 2.1: BranchManager — Crear Snapshots y Workspaces por Agente

**Opción A:** Implementar BranchManager custom (control total)
**Opción B:** Usar `worktrunk` crate como library (https://crates.io/crates/worktrunk)

**Recomendación:** Opción A al principio (BranchManager simple con
`std::process::Command` para `git worktree`), migrar a worktrunk
si necesitamos features avanzados (hash_port, copy build caches).

**Archivo nuevo:** `gestalt_core/src/orchestrator/branch_manager.rs`

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct BranchManager {
    waves_dir: PathBuf,
}

pub struct AgentWorkspace {
    pub agent_id: String,
    pub worktree_path: PathBuf,
    pub base_commit: String,
}

impl BranchManager {
    pub fn new(waves_dir: PathBuf) -> Self { ... }

    /// Crea snapshot del repo y retorna el commit base
    pub fn snapshot_base(&self, repo_path: &Path, wave_id: &str) -> Result<String> {
        // 1. git commit --allow-empty -m "wave-{wave_id} base"
        // 2. guardar commit hash
        // 3. retornar hash
    }

    /// Crea un worktree aislado para un agente
    pub fn create_agent_branch(
        &self, repo_path: &Path,
        base_commit: &str, agent_id: &str,
    ) -> Result<AgentWorkspace> {
        // 1. mkdir -p waves/{wave_id}/agent-{id}
        // 2. git worktree add waves/{wave_id}/agent-{id} {base_commit}
        // 3. Crear branch específica: git checkout -b agent-{id}
        // 4. Retornar AgentWorkspace
    }

    /// Obtiene diff del worktree del agente contra base
    pub fn diff_agent(&self, workspace: &AgentWorkspace) -> Result<String> {
        // git diff {base_commit} HEAD -- *.rs *.py ...
        // También: git diff --cached para staged
    }

    /// Limpia todos los worktrees de una wave
    pub fn cleanup_wave(&self, repo_path: &Path, wave_id: &str) -> Result<()> {
        // 1. git worktree remove por cada agente
        // 2. rm -rf waves/{wave_id}/
    }
}
```

## Task 2.2: FileDiff Estructura

```rust
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    /// Patch en unified diff format
    pub patch: String,
    /// Quién hizo el cambio
    pub agent_id: String,
    /// Commit o version base sobre el que se hizo
    pub base_version: String,
}

#[derive(Debug, Clone)]
pub struct FileDiffSet {
    pub base_commit: String,
    pub files: HashMap<PathBuf, Vec<FileDiff>>,
    // path → [diff de agent-a, diff de agent-b, ...]
}
```

## Task 2.3: MergeEngine — Three-Way Merge con threeway_merge crate

**Archivo nuevo:** `gestalt_core/src/orchestrator/merge_engine.rs`

```rust
pub struct MergeEngine;

impl MergeEngine {
    /// Three-way merge: base ↔ ours ↔ theirs
    /// Retorna texto mergeado + conteo de conflictos
    pub fn three_way_merge(
        base: &str, ours: &str, theirs: &str,
    ) -> Result<MergeResult> {
        let options = MergeOptions {
            algorithm: DiffAlgorithm::Histogram,
            style: MergeStyle::ZealousDiff3,
            favor: Some(MergeFavor::Union),
            ..Default::default()
        };
        let result = merge_strings(base, ours, theirs, &options)?;
        Ok(MergeResult {
            content: result.content,
            conflicts: result.conflicts,
        })
    }

    /// Batch merge para múltiples agentes sobre múltiples archivos
    pub fn batch_merge(
        base_commit: &str,
        diffs: FileDiffSet,
        repo_path: &Path,
    ) -> Result<MergeReport> {
        // Para cada archivo:
        //   1. Leer contenido base del commit
        //   2. Por cada agente que tocó el archivo:
        //      a. Si 1 solo agente → aplicar patch directamente
        //      b. Si 2+ agentes → three-way merge
        //   3. Escribir resultado
        //   4. Reportar conflictos
    }

    /// LLM-assisted resolution via agy
    pub fn resolve_with_llm(
        base: &str, ours: &str, theirs: &str,
        conflict_markers: &str,
    ) -> Result<String> {
        // 1. Construir prompt con base + theirs + conflict
        // 2. Llamar agy con --effort high
        // 3. Parsear respuesta
    }
}

#[derive(Debug)]
pub struct MergeResult {
    pub content: String,
    pub conflicts: usize,
}

#[derive(Debug)]
pub struct MergeReport {
    pub files_merged: Vec<PathBuf>,
    pub files_conflicted: Vec<PathBuf>,
    pub total_conflicts: usize,
    pub auto_resolved: usize,
    pub llm_resolved: usize,
    pub manual_needed: Vec<PathBuf>,
}
```

## Task 2.4: WaveManager — Coordinación de Waves Completa

**Archivo nuevo:** `gestalt_core/src/orchestrator/wave_manager.rs`

```rust
pub struct WaveManager {
    branch_mgr: BranchManager,
    merge_engine: MergeEngine,
    repo_path: PathBuf,
}

impl WaveManager {
    /// Fase 0-1: Prepara la wave
    pub fn prepare_wave(&self, wave_id: &str, agents: &[String]) -> Result<WaveContext> {
        // 1. Snapshot base
        // 2. Crear worktrees
        // 3. Retornar WaveContext con paths por agente
    }

    /// Fase 2-3: Colectar diffs de agentes completados
    pub fn collect_diffs(&self, ctx: &WaveContext) -> Result<FileDiffSet> {
        // Por cada agente: diff_agent() → FileDiff
        // Agrupar por archivo
    }

    /// Fase 4: Mergear todo
    pub fn merge_wave(&self, ctx: &WaveContext, diffs: FileDiffSet) -> Result<MergeReport> {
        // 1. batch_merge()
        // 2. Escribir archivos mergeados al repo
        // 3. Si conflictos → LLM resolve o flag manual
    }

    /// Fase 5-6: Limpiar
    pub fn finalize_wave(&self, ctx: &WaveContext, report: &MergeReport) -> Result<()> {
        // 1. Si todo OK: git add, git commit, git push
        // 2. cleanup_wave()
    }
}
```

## Task 2.5: Tests del MergeEngine

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_conflict_different_lines() {
        // base: "line1\nline2\nline3"
        // ours: "line1\nCHANGED\nline3"
        // theirs: "line1\nline2\nCHANGED2"
        // result: "line1\nCHANGED\nCHANGED2" (0 conflicts)
    }

    #[test]
    fn test_no_conflict_same_change() {
        // base: "hello"
        // ours: "hello world"
        // theirs: "hello world"
        // result: "hello world" (0 conflicts)
    }

    #[test]
    fn test_conflict_different_changes_same_line() {
        // base: "hello"
        // ours: "hello world"
        // theirs: "hello beautiful"
        // result: conflict detected (>0 conflicts)
    }

    #[test]
    fn test_new_file_no_conflict() {
        // base: no existe
        // ours: "new content"
        // theirs: no tocó
        // result: "new content"
    }

    #[test]
    fn test_union_mode_merge() {
        // base: "x"
        // ours: "x\ny"
        // theirs: "x\nz"
        // union mode: "x\ny\nz" (toma ambos)
    }
}
```
