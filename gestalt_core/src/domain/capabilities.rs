//! Typed capability descriptors for agent providers.
//!
//! Gestalt historically modelled provider capabilities as free-form
//! `Vec<String>` labels (see [`crate::application::agent::registry::AgentEntry`]).
//! That is flexible but unchecked: a typo such as `"edit"` vs `"editing"` only
//! fails at runtime, and nothing prevents asking a non-streaming provider for a
//! stream.
//!
//! This module adds a parallel, strongly typed model:
//!
//! - [`Capability`] enumerates the discrete behaviours a provider may offer.
//! - [`ProviderCapabilities`] is a serialisable descriptor for one provider:
//!   which capabilities it has, plus the budgets used to rank candidates
//!   (context window and price).
//!
//! The legacy string-based [`crate::application::agent::registry::AgentRegistry`]
//! is intentionally left untouched so callers can migrate incrementally.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A discrete behaviour an agent provider may or may not support.
///
/// Each variant maps 1:1 onto a boolean field of [`ProviderCapabilities`],
/// which is what [`ProviderCapabilities::supports`] resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Incremental token delivery instead of a single buffered response.
    Streaming,
    /// Model can call tools/functions exposed by the harness.
    ToolCalling,
    /// Model accepts image input.
    Vision,
    /// Provider can read and write files in the workspace.
    FileEditing,
    /// Provider can execute shell commands.
    ShellExec,
    /// Provider can hold a context window of at least 100k tokens.
    LongContext,
}

impl Capability {
    /// Every capability, in declaration order.
    pub const ALL: [Capability; 6] = [
        Capability::Streaming,
        Capability::ToolCalling,
        Capability::Vision,
        Capability::FileEditing,
        Capability::ShellExec,
        Capability::LongContext,
    ];

    /// Stable, lower-case identifier suitable for logs, metrics and CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Streaming => "streaming",
            Capability::ToolCalling => "tool_calling",
            Capability::Vision => "vision",
            Capability::FileEditing => "file_editing",
            Capability::ShellExec => "shell_exec",
            Capability::LongContext => "long_context",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed descriptor of what an agent provider (a CLI or an API) can do.
///
/// Build one from a known provider constructor ([`ProviderCapabilities::claude`],
/// [`ProviderCapabilities::opencode`], [`ProviderCapabilities::agy`],
/// [`ProviderCapabilities::codex`], [`ProviderCapabilities::wrangler`]) or from
/// scratch with [`ProviderCapabilities::builder`].
///
/// # Conventions
///
/// - `long_context` is expected to be `true` exactly when
///   `max_context_tokens >= 100_000`; every bundled constructor honours this.
/// - `cost_per_mtok_in` / `cost_per_mtok_out` are USD per million tokens, `0.0`
///   for providers that are free or not LLM-backed (e.g. `wrangler`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProviderCapabilities {
    /// Delivers tokens incrementally.
    pub streaming: bool,
    /// Supports tool / function calling.
    pub tool_calling: bool,
    /// Accepts image input.
    pub vision: bool,
    /// Can read and modify workspace files.
    pub file_editing: bool,
    /// Can execute shell commands.
    pub shell_exec: bool,
    /// Can hold a context window of at least 100k tokens.
    pub long_context: bool,
    /// Maximum context window, in tokens. `0` when not applicable.
    pub max_context_tokens: u64,
    /// Model identifiers the provider can be driven with.
    pub supported_models: Vec<String>,
    /// Price in USD per million input tokens.
    pub cost_per_mtok_in: f64,
    /// Price in USD per million output tokens.
    pub cost_per_mtok_out: f64,
}

impl ProviderCapabilities {
    /// Starts a builder pre-filled with [`ProviderCapabilities::default`].
    pub fn builder() -> ProviderCapabilitiesBuilder {
        ProviderCapabilitiesBuilder::default()
    }

    /// Returns `true` when this provider offers `capability`.
    pub fn supports(&self, capability: Capability) -> bool {
        match capability {
            Capability::Streaming => self.streaming,
            Capability::ToolCalling => self.tool_calling,
            Capability::Vision => self.vision,
            Capability::FileEditing => self.file_editing,
            Capability::ShellExec => self.shell_exec,
            Capability::LongContext => self.long_context,
        }
    }

    /// Returns `true` when **every** capability in `required` is supported.
    ///
    /// An empty requirement is satisfied vacuously.
    pub fn satisfies(&self, required: &[Capability]) -> bool {
        required.iter().all(|capability| self.supports(*capability))
    }

    /// Ranking score for a requirement list, in the `[0.0, 100.0]` range.
    ///
    /// # Formula
    ///
    /// ```text
    /// score = 100.0 * matched / required.len()
    /// ```
    ///
    /// where `matched` is the number of entries in `required` that
    /// [`ProviderCapabilities::supports`] reports as available. An empty
    /// `required` scores `0.0` (nothing to match, so nothing to reward).
    ///
    /// The score is deliberately monotonic in the number of matched
    /// capabilities: for a fixed requirement list, a provider matching strictly
    /// more capabilities always scores strictly higher. Context size and price
    /// are **not** part of the formula, so equally capable providers tie
    /// exactly and [`crate::adapters::providers::ProviderRegistry::select_for`]
    /// can break the tie by price and then by name.
    pub fn score(&self, required: &[Capability]) -> f64 {
        if required.is_empty() {
            return 0.0;
        }
        let matched = required
            .iter()
            .filter(|capability| self.supports(**capability))
            .count();
        100.0 * matched as f64 / required.len() as f64
    }

    /// Defaults for the Anthropic Claude Code CLI.
    ///
    /// Fully featured (streaming, tools, vision, file editing, shell, long
    /// context), 200k context window, premium pricing.
    pub fn claude() -> Self {
        Self {
            streaming: true,
            tool_calling: true,
            vision: true,
            file_editing: true,
            shell_exec: true,
            long_context: true,
            max_context_tokens: 200_000,
            supported_models: vec![
                "claude-sonnet-4-5".to_string(),
                "claude-opus-4-1".to_string(),
                "claude-haiku-4-5".to_string(),
            ],
            cost_per_mtok_in: 3.0,
            cost_per_mtok_out: 15.0,
        }
    }

    /// Defaults for the OpenCode CLI.
    ///
    /// Fully featured like Claude, but routed to cheaper models, so the lowest
    /// input price of the fully capable providers.
    pub fn opencode() -> Self {
        Self {
            streaming: true,
            tool_calling: true,
            vision: true,
            file_editing: true,
            shell_exec: true,
            long_context: true,
            max_context_tokens: 200_000,
            supported_models: vec![
                "deepseek-v4.1-flash".to_string(),
                "qwen3-coder-plus".to_string(),
                "claude-sonnet-4-5".to_string(),
            ],
            cost_per_mtok_in: 0.5,
            cost_per_mtok_out: 2.0,
        }
    }

    /// Defaults for the Antigravity (`agy`) CLI, backed by Gemini.
    ///
    /// Fully featured with the largest context window of the bundled set.
    pub fn agy() -> Self {
        Self {
            streaming: true,
            tool_calling: true,
            vision: true,
            file_editing: true,
            shell_exec: true,
            long_context: true,
            max_context_tokens: 1_000_000,
            supported_models: vec!["gemini-3.8-flash".to_string(), "gemini-3.1-pro".to_string()],
            cost_per_mtok_in: 0.3,
            cost_per_mtok_out: 2.5,
        }
    }

    /// Defaults for the OpenAI Codex CLI.
    ///
    /// Code-oriented: no image input, but tools, file editing, shell and a
    /// 400k context window.
    pub fn codex() -> Self {
        Self {
            streaming: true,
            tool_calling: true,
            vision: false,
            file_editing: true,
            shell_exec: true,
            long_context: true,
            max_context_tokens: 400_000,
            supported_models: vec!["gpt-5-codex".to_string(), "gpt-5".to_string()],
            cost_per_mtok_in: 1.25,
            cost_per_mtok_out: 10.0,
        }
    }

    /// Defaults for the Cloudflare `wrangler` CLI.
    ///
    /// Not an LLM provider: no streaming, tools, vision or context window. It
    /// only edits files and runs shell commands (deploy, tail, kv, d1, ...),
    /// and it is free.
    pub fn wrangler() -> Self {
        Self {
            streaming: false,
            tool_calling: false,
            vision: false,
            file_editing: true,
            shell_exec: true,
            long_context: false,
            max_context_tokens: 0,
            supported_models: Vec::new(),
            cost_per_mtok_in: 0.0,
            cost_per_mtok_out: 0.0,
        }
    }
}

/// Fluent builder for [`ProviderCapabilities`].
///
/// ```
/// use gestalt_core::domain::capabilities::{Capability, ProviderCapabilities};
///
/// let caps = ProviderCapabilities::builder()
///     .streaming(true)
///     .tool_calling(true)
///     .max_context_tokens(32_000)
///     .supported_models(vec!["my-model"])
///     .cost_per_mtok_in(0.1)
///     .build();
///
/// assert!(caps.supports(Capability::Streaming));
/// assert!(!caps.supports(Capability::Vision));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilitiesBuilder {
    inner: ProviderCapabilities,
}

impl ProviderCapabilitiesBuilder {
    /// Sets [`ProviderCapabilities::streaming`].
    pub fn streaming(mut self, value: bool) -> Self {
        self.inner.streaming = value;
        self
    }

    /// Sets [`ProviderCapabilities::tool_calling`].
    pub fn tool_calling(mut self, value: bool) -> Self {
        self.inner.tool_calling = value;
        self
    }

    /// Sets [`ProviderCapabilities::vision`].
    pub fn vision(mut self, value: bool) -> Self {
        self.inner.vision = value;
        self
    }

    /// Sets [`ProviderCapabilities::file_editing`].
    pub fn file_editing(mut self, value: bool) -> Self {
        self.inner.file_editing = value;
        self
    }

    /// Sets [`ProviderCapabilities::shell_exec`].
    pub fn shell_exec(mut self, value: bool) -> Self {
        self.inner.shell_exec = value;
        self
    }

    /// Sets [`ProviderCapabilities::long_context`].
    pub fn long_context(mut self, value: bool) -> Self {
        self.inner.long_context = value;
        self
    }

    /// Sets [`ProviderCapabilities::max_context_tokens`].
    pub fn max_context_tokens(mut self, value: u64) -> Self {
        self.inner.max_context_tokens = value;
        self
    }

    /// Replaces [`ProviderCapabilities::supported_models`].
    pub fn supported_models<I, S>(mut self, models: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.supported_models = models.into_iter().map(Into::into).collect();
        self
    }

    /// Sets [`ProviderCapabilities::cost_per_mtok_in`].
    pub fn cost_per_mtok_in(mut self, value: f64) -> Self {
        self.inner.cost_per_mtok_in = value;
        self
    }

    /// Sets [`ProviderCapabilities::cost_per_mtok_out`].
    pub fn cost_per_mtok_out(mut self, value: f64) -> Self {
        self.inner.cost_per_mtok_out = value;
        self
    }

    /// Finishes the builder.
    pub fn build(self) -> ProviderCapabilities {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bundled constructor, with the flags it is documented to report.
    fn documented_defaults() -> Vec<(ProviderCapabilities, [bool; 6])> {
        vec![
            // streaming, tool_calling, vision, file_editing, shell_exec, long_context
            (
                ProviderCapabilities::claude(),
                [true, true, true, true, true, true],
            ),
            (
                ProviderCapabilities::opencode(),
                [true, true, true, true, true, true],
            ),
            (
                ProviderCapabilities::agy(),
                [true, true, true, true, true, true],
            ),
            (
                ProviderCapabilities::codex(),
                [true, true, false, true, true, true],
            ),
            (
                ProviderCapabilities::wrangler(),
                [false, false, false, true, true, false],
            ),
        ]
    }

    #[test]
    fn provider_constructors_report_documented_capabilities() {
        for (caps, expected) in documented_defaults() {
            for (capability, expected) in Capability::ALL.iter().zip(expected) {
                assert_eq!(
                    caps.supports(*capability),
                    expected,
                    "{} should report supports({}) == {}",
                    caps.supported_models
                        .first()
                        .map(String::as_str)
                        .unwrap_or("<none>"),
                    capability,
                    expected
                );
            }
        }
    }

    #[test]
    fn provider_constructors_respect_long_context_convention() {
        for (caps, _) in documented_defaults() {
            assert_eq!(
                caps.long_context,
                caps.max_context_tokens >= 100_000,
                "long_context must track max_context_tokens >= 100_000"
            );
        }
    }

    #[test]
    fn provider_constructors_have_sane_budgets() {
        let agy = ProviderCapabilities::agy();
        assert_eq!(agy.max_context_tokens, 1_000_000);
        assert!(agy.cost_per_mtok_in > 0.0);
        assert!(agy.cost_per_mtok_out >= agy.cost_per_mtok_in);

        let wrangler = ProviderCapabilities::wrangler();
        assert_eq!(wrangler.max_context_tokens, 0);
        assert!(wrangler.supported_models.is_empty());
        assert_eq!(wrangler.cost_per_mtok_in, 0.0);

        // The fully capable providers are ordered by input price as documented.
        assert!(
            ProviderCapabilities::agy().cost_per_mtok_in
                < ProviderCapabilities::opencode().cost_per_mtok_in
        );
        assert!(
            ProviderCapabilities::opencode().cost_per_mtok_in
                < ProviderCapabilities::codex().cost_per_mtok_in
        );
        assert!(
            ProviderCapabilities::codex().cost_per_mtok_in
                < ProviderCapabilities::claude().cost_per_mtok_in
        );
    }

    #[test]
    fn supports_reports_positive_and_negative_cases() {
        let codex = ProviderCapabilities::codex();
        assert!(codex.supports(Capability::Streaming));
        assert!(codex.supports(Capability::ToolCalling));
        assert!(codex.supports(Capability::FileEditing));
        assert!(codex.supports(Capability::ShellExec));
        assert!(codex.supports(Capability::LongContext));
        // Codex CLI has no image input.
        assert!(!codex.supports(Capability::Vision));

        let default = ProviderCapabilities::default();
        for capability in Capability::ALL {
            assert!(!default.supports(capability));
        }
    }

    #[test]
    fn satisfies_requires_every_capability() {
        let claude = ProviderCapabilities::claude();

        assert!(claude.satisfies(&[]));
        assert!(claude.satisfies(&[Capability::Streaming]));
        assert!(claude.satisfies(Capability::ALL.as_slice()));

        let codex = ProviderCapabilities::codex();
        assert!(codex.satisfies(&[Capability::ToolCalling, Capability::ShellExec]));
        assert!(!codex.satisfies(&[Capability::ToolCalling, Capability::Vision]));

        let wrangler = ProviderCapabilities::wrangler();
        assert!(wrangler.satisfies(&[Capability::ShellExec]));
        assert!(!wrangler.satisfies(&[Capability::Streaming]));
    }

    #[test]
    fn score_is_monotonic_in_matched_capabilities() {
        let required = [
            Capability::Streaming,
            Capability::ToolCalling,
            Capability::Vision,
            Capability::FileEditing,
        ];

        // 0, 1, 2, 3 and 4 matched capabilities respectively.
        let none = ProviderCapabilities::default();
        let one = ProviderCapabilities::builder().streaming(true).build();
        let two = ProviderCapabilities::builder()
            .streaming(true)
            .tool_calling(true)
            .build();
        let three = ProviderCapabilities::builder()
            .streaming(true)
            .tool_calling(true)
            .file_editing(true)
            .build();
        let four = ProviderCapabilities::builder()
            .streaming(true)
            .tool_calling(true)
            .vision(true)
            .file_editing(true)
            .build();

        let scores: Vec<f64> = [&none, &one, &two, &three, &four]
            .iter()
            .map(|caps| caps.score(&required))
            .collect();

        for pair in scores.windows(2) {
            assert!(
                pair[0] < pair[1],
                "score must grow with each matched capability: {scores:?}"
            );
        }
        assert_eq!(scores.first().copied(), Some(0.0));
        assert_eq!(scores.last().copied(), Some(100.0));
    }

    #[test]
    fn score_ignores_capabilities_outside_the_requirement() {
        let required = [Capability::Vision];
        let a = ProviderCapabilities::builder().vision(true).build();
        let b = ProviderCapabilities::builder()
            .vision(true)
            .streaming(true)
            .tool_calling(true)
            .shell_exec(true)
            .build();
        assert_eq!(a.score(&required), b.score(&required));
    }

    #[test]
    fn score_of_empty_requirement_is_zero() {
        assert_eq!(ProviderCapabilities::claude().score(&[]), 0.0);
    }

    #[test]
    fn builder_starts_from_default_and_sets_every_field() {
        let built = ProviderCapabilities::builder()
            .streaming(true)
            .tool_calling(true)
            .vision(true)
            .file_editing(true)
            .shell_exec(true)
            .long_context(true)
            .max_context_tokens(128_000)
            .supported_models(["m1", "m2"])
            .cost_per_mtok_in(1.5)
            .cost_per_mtok_out(6.0)
            .build();

        assert_eq!(
            built,
            ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                file_editing: true,
                shell_exec: true,
                long_context: true,
                max_context_tokens: 128_000,
                supported_models: vec!["m1".to_string(), "m2".to_string()],
                cost_per_mtok_in: 1.5,
                cost_per_mtok_out: 6.0,
            }
        );
    }

    #[test]
    fn capability_identifiers_are_stable() {
        let identifiers: Vec<&str> = Capability::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            identifiers,
            vec![
                "streaming",
                "tool_calling",
                "vision",
                "file_editing",
                "shell_exec",
                "long_context",
            ]
        );
        assert_eq!(Capability::ToolCalling.to_string(), "tool_calling");
    }

    #[test]
    fn capabilities_survive_a_json_roundtrip() {
        let caps = ProviderCapabilities::agy();
        let json = serde_json::to_string(&caps).unwrap();
        let decoded: ProviderCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, decoded);

        let capability_json = serde_json::to_string(&Capability::FileEditing).unwrap();
        assert_eq!(capability_json, "\"file_editing\"");
        let decoded: Capability = serde_json::from_str(&capability_json).unwrap();
        assert_eq!(decoded, Capability::FileEditing);
    }
}
