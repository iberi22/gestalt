//! Discovery and ranking of the agent provider CLIs available on this host.
//!
//! [`ProviderRegistry`] scans `PATH` (or an explicit list of directories) for
//! the known provider binaries, attaches the typed descriptor from
//! [`crate::domain::capabilities::ProviderCapabilities`] to each one it finds,
//! and ranks the result against a list of required
//! [`Capability`](crate::domain::capabilities::Capability)s.

use crate::domain::capabilities::{Capability, ProviderCapabilities};
use std::cmp::Ordering;
use std::path::PathBuf;

/// Constructor returning the default [`ProviderCapabilities`] of a provider.
pub type ProviderConstructor = fn() -> ProviderCapabilities;

/// Provider binaries this registry knows how to describe, paired with the
/// constructor that returns their default [`ProviderCapabilities`].
///
/// Order matters only for documentation; discovery probes each entry
/// independently.
pub const KNOWN_PROVIDERS: [(&str, ProviderConstructor); 5] = [
    ("claude", ProviderCapabilities::claude),
    ("opencode", ProviderCapabilities::opencode),
    ("agy", ProviderCapabilities::agy),
    ("codex", ProviderCapabilities::codex),
    ("wrangler", ProviderCapabilities::wrangler),
];

/// A provider binary that was actually found on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredProvider {
    /// Provider name, e.g. `"claude"`. Matches the binary name.
    pub name: String,
    /// Absolute-or-relative path to the binary as found in a `PATH` entry.
    pub binary_path: PathBuf,
    /// What this provider can do.
    pub capabilities: ProviderCapabilities,
}

impl RegisteredProvider {
    /// Convenience constructor, mainly for tests and mock injection.
    pub fn new(
        name: impl Into<String>,
        binary_path: impl Into<PathBuf>,
        capabilities: ProviderCapabilities,
    ) -> Self {
        Self {
            name: name.into(),
            binary_path: binary_path.into(),
            capabilities,
        }
    }
}

/// The set of provider CLIs discovered on this host.
///
/// Cheap to clone and safe to share: it is a plain `Vec` of owned data with no
/// open handles, therefore `Send + Sync`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProviderRegistry {
    providers: Vec<RegisteredProvider>,
}

impl ProviderRegistry {
    /// An empty registry. Use [`ProviderRegistry::discover`] to populate it from
    /// the host `PATH`, or [`ProviderRegistry::register`] to inject entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Discovers provider binaries by scanning the directories in `PATH`.
    ///
    /// Only `Path::is_file` is used — no process is spawned, and no executable
    /// permission bit is checked (that is the OS's job when the provider is
    /// actually launched).
    ///
    /// Binary names are probed verbatim (Unix-style, no `.exe` suffix), which
    /// matches the platforms Gestalt targets.
    pub fn discover() -> Self {
        let directories: Vec<PathBuf> = match std::env::var_os("PATH") {
            Some(path) => std::env::split_paths(&path).collect(),
            None => Vec::new(),
        };
        Self::scan(&directories)
    }

    /// Builds a registry by scanning exactly the given directories, in order.
    ///
    /// This is the deterministic, `PATH`-independent entry point used by tests
    /// and by callers that manage their own toolchain directories.
    pub fn from_path_list(entries: &[&str]) -> Self {
        let directories: Vec<PathBuf> = entries.iter().map(PathBuf::from).collect();
        Self::scan(&directories)
    }

    /// Shared scanning logic for [`ProviderRegistry::discover`] and
    /// [`ProviderRegistry::from_path_list`].
    ///
    /// For each known provider the directories are probed in order and the
    /// first hit wins, so a duplicated `PATH` entry cannot produce duplicates.
    fn scan(directories: &[PathBuf]) -> Self {
        let mut registry = Self::new();
        for (name, constructor) in KNOWN_PROVIDERS {
            for directory in directories {
                let candidate = directory.join(name);
                if candidate.is_file() {
                    registry.register(RegisteredProvider::new(name, candidate, constructor()));
                    break;
                }
            }
        }
        registry
    }

    /// Adds a provider, replacing any entry with the same `name`.
    ///
    /// Replacing keeps names unique, so the ordering produced by
    /// [`ProviderRegistry::rank_for`] is total and stable.
    pub fn register(&mut self, provider: RegisteredProvider) {
        match self
            .providers
            .iter_mut()
            .find(|existing| existing.name == provider.name)
        {
            Some(existing) => *existing = provider,
            None => self.providers.push(provider),
        }
    }

    /// All discovered providers, in discovery order.
    pub fn list(&self) -> &[RegisteredProvider] {
        &self.providers
    }

    /// Looks up a provider by name.
    pub fn get(&self, name: &str) -> Option<&RegisteredProvider> {
        self.providers.iter().find(|p| p.name == name)
    }

    /// Number of discovered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns `true` when nothing was discovered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Picks the best provider able to satisfy `required`, or `None` when no
    /// discovered provider can.
    ///
    /// "Best" means highest
    /// [`ProviderCapabilities::score`](crate::domain::capabilities::ProviderCapabilities::score);
    /// ties are broken by lowest input price, then lowest output price, then
    /// name (ascending) so the result is deterministic.
    pub fn select_for(&self, required: &[Capability]) -> Option<&RegisteredProvider> {
        self.providers
            .iter()
            .filter(|provider| provider.capabilities.satisfies(required))
            .min_by(|a, b| Self::rank_cmp(a, b, required))
    }

    /// Ranks **every** discovered provider for `required`, best first.
    ///
    /// Providers that cannot satisfy the requirement are still returned, at the
    /// bottom, which makes this useful for diagnostics ("who is closest?").
    /// The first element equals [`ProviderRegistry::select_for`] whenever at
    /// least one provider satisfies the requirement.
    ///
    /// The order is total: score descending, then input price ascending, then
    /// output price ascending, then name ascending.
    pub fn rank_for(&self, required: &[Capability]) -> Vec<&RegisteredProvider> {
        let mut ranked: Vec<&RegisteredProvider> = self.providers.iter().collect();
        ranked.sort_by(|a, b| Self::rank_cmp(a, b, required));
        ranked
    }

    /// Total order over providers for a requirement: `Less` means "ranks
    /// better". Score dominates; price and name only break ties.
    fn rank_cmp(
        a: &RegisteredProvider,
        b: &RegisteredProvider,
        required: &[Capability],
    ) -> Ordering {
        let by_score = b
            .capabilities
            .score(required)
            .partial_cmp(&a.capabilities.score(required))
            .unwrap_or(Ordering::Equal);

        by_score
            .then_with(|| {
                a.capabilities
                    .cost_per_mtok_in
                    .partial_cmp(&b.capabilities.cost_per_mtok_in)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                a.capabilities
                    .cost_per_mtok_out
                    .partial_cmp(&b.capabilities.cost_per_mtok_out)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| a.name.cmp(&b.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: `path` rendered for assertions and error messages.
    fn display_path(path: &std::path::Path) -> String {
        path.display().to_string()
    }

    /// Builds a throwaway directory tree of fake provider binaries.
    ///
    /// Returns the temp dir (kept alive by the caller) and its path as a
    /// `String`, which is what [`ProviderRegistry::from_path_list`] takes.
    fn fake_bin_dir(files: &[&str]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("create temp dir");
        for name in files {
            let path = dir.path().join(name);
            fs::write(&path, b"#!/bin/sh\nexit 0\n").expect("write fake binary");
            assert!(path.is_file(), "{} should exist", display_path(&path));
        }
        let as_string = dir
            .path()
            .to_str()
            .expect("temp dir path is valid UTF-8")
            .to_string();
        (dir, as_string)
    }

    fn mock(name: &str, capabilities: ProviderCapabilities) -> RegisteredProvider {
        RegisteredProvider::new(
            name,
            PathBuf::from(format!("/fake/bin/{name}")),
            capabilities,
        )
    }

    #[test]
    fn registry_is_send_sync_and_clone() {
        fn assert_traits<T: Send + Sync + Clone>() {}
        assert_traits::<ProviderRegistry>();
        assert_traits::<RegisteredProvider>();
    }

    #[test]
    fn from_path_list_discovers_exactly_the_present_binaries() {
        // `kimi` is not a known provider; `codex` is created as a *directory*
        // to prove that only regular files are accepted.
        let (dir, path) = fake_bin_dir(&["claude", "agy", "kimi"]);
        fs::create_dir(dir.path().join("codex")).expect("create decoy dir");

        let registry = ProviderRegistry::from_path_list(&[path.as_str()]);

        let names: Vec<&str> = registry.list().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["claude", "agy"]);

        let claude = registry.get("claude").expect("claude discovered");
        assert_eq!(claude.binary_path, dir.path().join("claude"));
        assert_eq!(claude.capabilities, ProviderCapabilities::claude());
        assert!(registry.get("codex").is_none());
        assert!(registry.get("kimi").is_none());
    }

    #[test]
    fn from_path_list_with_no_matches_is_empty() {
        let (_dir, path) = fake_bin_dir(&["not-a-provider"]);
        let registry = ProviderRegistry::from_path_list(&[path.as_str()]);
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert_eq!(registry.list().len(), 0);
        assert!(registry.select_for(&[Capability::ShellExec]).is_none());
    }

    #[test]
    fn from_path_list_handles_missing_directories() {
        let registry =
            ProviderRegistry::from_path_list(&["/nonexistent/gestalt/bin", "/also/missing"]);
        assert!(registry.is_empty());
    }

    #[test]
    fn discovery_uses_the_first_matching_directory() {
        let (_first, first) = fake_bin_dir(&["codex"]);
        let (_second, second) = fake_bin_dir(&["codex", "claude"]);

        let registry = ProviderRegistry::from_path_list(&[first.as_str(), second.as_str()]);

        let codex = registry.get("codex").expect("codex discovered");
        assert!(
            codex.binary_path.starts_with(&first),
            "first PATH hit must win, got {}",
            display_path(&codex.binary_path)
        );
        assert!(registry.get("claude").is_some());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn register_injects_a_mock_and_replaces_by_name() {
        let mut registry = ProviderRegistry::new();
        assert!(registry.is_empty());

        registry.register(mock("mock-a", ProviderCapabilities::claude()));
        assert_eq!(registry.len(), 1);

        registry.register(mock("mock-a", ProviderCapabilities::wrangler()));
        assert_eq!(registry.len(), 1, "same name must replace, not duplicate");
        assert_eq!(
            registry.get("mock-a").unwrap().capabilities,
            ProviderCapabilities::wrangler()
        );
    }

    #[test]
    fn select_for_returns_none_when_the_requirement_cannot_be_met() {
        let mut registry = ProviderRegistry::new();
        registry.register(mock("wrangler", ProviderCapabilities::wrangler()));

        assert!(registry.select_for(&[Capability::Vision]).is_none());
        assert!(registry
            .select_for(&[Capability::ShellExec, Capability::Streaming])
            .is_none());
        // ... but the capability it does have is still selectable.
        assert_eq!(
            registry
                .select_for(&[Capability::ShellExec])
                .map(|p| p.name.as_str()),
            Some("wrangler")
        );
    }

    #[test]
    fn select_for_picks_the_cheapest_among_equally_capable_providers() {
        let capabilities = |price: f64| {
            ProviderCapabilities::builder()
                .streaming(true)
                .tool_calling(true)
                .max_context_tokens(200_000)
                .cost_per_mtok_in(price)
                .cost_per_mtok_out(price * 4.0)
                .build()
        };

        let mut registry = ProviderRegistry::new();
        registry.register(mock("pricey", capabilities(5.0)));
        registry.register(mock("cheap", capabilities(0.5)));
        registry.register(mock("middling", capabilities(2.0)));

        let required = [Capability::Streaming, Capability::ToolCalling];
        assert_eq!(
            registry.select_for(&required).map(|p| p.name.as_str()),
            Some("cheap")
        );
    }

    #[test]
    fn select_for_prefers_more_capabilities_over_a_lower_price() {
        let free_but_limited = ProviderCapabilities::builder()
            .shell_exec(true)
            .cost_per_mtok_in(0.0)
            .build();
        let expensive_but_complete = ProviderCapabilities::builder()
            .shell_exec(true)
            .streaming(true)
            .cost_per_mtok_in(10.0)
            .build();

        let mut registry = ProviderRegistry::new();
        registry.register(mock("limited", free_but_limited));
        registry.register(mock("complete", expensive_but_complete));

        let required = [Capability::ShellExec, Capability::Streaming];
        assert_eq!(
            registry.select_for(&required).map(|p| p.name.as_str()),
            Some("complete")
        );
    }

    #[test]
    fn select_for_breaks_exact_ties_by_name() {
        let identical = ProviderCapabilities::builder()
            .streaming(true)
            .cost_per_mtok_in(1.0)
            .cost_per_mtok_out(2.0)
            .build();

        let mut registry = ProviderRegistry::new();
        registry.register(mock("zulu", identical.clone()));
        registry.register(mock("alpha", identical));

        assert_eq!(
            registry
                .select_for(&[Capability::Streaming])
                .map(|p| p.name.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn rank_for_returns_a_descending_order() {
        let mut registry = ProviderRegistry::new();
        registry.register(mock("wrangler", ProviderCapabilities::wrangler()));
        registry.register(mock("codex", ProviderCapabilities::codex()));
        registry.register(mock("claude", ProviderCapabilities::claude()));

        let required = [
            Capability::Streaming,
            Capability::ToolCalling,
            Capability::Vision,
        ];
        let ranked = registry.rank_for(&required);

        assert_eq!(
            ranked.len(),
            3,
            "every provider is ranked, not just matches"
        );
        let scores: Vec<f64> = ranked
            .iter()
            .map(|p| p.capabilities.score(&required))
            .collect();
        for pair in scores.windows(2) {
            assert!(
                pair[0] >= pair[1],
                "rank_for must be sorted descending: {scores:?}"
            );
        }
        // wrangler supports none of the requirement, so it must come last.
        assert_eq!(ranked.last().map(|p| p.name.as_str()), Some("wrangler"));
        // The first ranked entry is exactly what select_for would return.
        assert_eq!(
            ranked.first().map(|p| p.name.as_str()),
            registry.select_for(&required).map(|p| p.name.as_str())
        );
    }

    #[test]
    fn rank_for_is_empty_for_an_empty_registry() {
        let registry = ProviderRegistry::new();
        assert!(registry.rank_for(&[Capability::Streaming]).is_empty());
        assert!(registry.select_for(&[]).is_none());
    }

    #[test]
    fn empty_requirement_selects_the_cheapest_provider() {
        let mut registry = ProviderRegistry::new();
        registry.register(mock("expensive", ProviderCapabilities::claude()));
        registry.register(mock("free", ProviderCapabilities::wrangler()));

        // With nothing required every provider qualifies; price decides.
        assert_eq!(
            registry.select_for(&[]).map(|p| p.name.as_str()),
            Some("free")
        );
    }

    #[test]
    fn discover_is_safe_to_call_on_this_host() {
        // Smoke test: discovery must never panic, whatever PATH contains.
        let registry = ProviderRegistry::discover();
        for provider in registry.list() {
            assert!(
                KNOWN_PROVIDERS
                    .iter()
                    .any(|(name, _)| *name == provider.name),
                "discovered unknown provider {}",
                provider.name
            );
            assert!(provider.binary_path.is_file());
        }
    }
}
