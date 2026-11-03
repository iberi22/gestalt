//! Provider health checks for the agent CLIs the router can drive.
//!
//! Every provider is a CLI (`claude`, `opencode`, `agy`, `codex`, `wrangler`)
//! that the router shells out to. Before dispatching work the router needs to
//! know whether a provider can actually run, so this module probes each one in
//! three layers:
//!
//! 1. **Binary presence** — resolved through `which` (with a direct `PATH`
//!    scan as fallback for minimal containers).
//! 2. **CLI responsiveness** — a short version command run through
//!    [`tokio::process::Command`], bounded by the provider budget.
//! 3. **API connectivity** — a TCP reachability probe against the provider's
//!    API endpoint, only when the matching credentials are configured (no
//!    point in probing an API the operator cannot use).
//!
//! Results are aggregated into a [`HealthStatus`] (`healthy` / `degraded` /
//! `unhealthy`) and cached for [`DEFAULT_TTL`] seconds so a health endpoint
//! polled by a supervisor never turns into a process-spawning storm.
//!
//! All probes are concurrent and bounded by [`DEFAULT_TIMEOUT`]: a slow or
//! hung CLI degrades its own provider, never the whole report.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::process::Command;
use tracing::warn;

/// How long a cached report stays fresh before it is recomputed.
pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

/// Hard budget for a single provider probe (all layers included).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Budget for resolving the binary in `PATH`.
const LOCATE_BUDGET: Duration = Duration::from_secs(2);

/// Budget for the version command.
const VERSION_BUDGET: Duration = Duration::from_secs(3);

/// Budget for the API reachability probe.
const API_BUDGET: Duration = Duration::from_secs(2);

/// Maximum characters kept from the version output.
const VERSION_MAX_CHARS: usize = 120;

/// Structured health of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// CLI present, responsive, and (when applicable) its API is reachable.
    Healthy,
    /// CLI present but not fully functional (bad version command, API down).
    Degraded,
    /// CLI missing — the provider cannot be used at all.
    Unhealthy,
}

impl HealthStatus {
    /// Severity rank used to aggregate several providers into one status.
    fn rank(self) -> u8 {
        match self {
            Self::Healthy => 0,
            Self::Degraded => 1,
            Self::Unhealthy => 2,
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        };
        f.write_str(name)
    }
}

/// Worst status wins: any unhealthy provider makes the aggregate unhealthy.
pub fn aggregate_status(checks: &[HealthCheck]) -> HealthStatus {
    checks
        .iter()
        .map(|c| c.status)
        .max_by_key(|s| s.rank())
        .unwrap_or(HealthStatus::Healthy)
}

/// API reachability probe configuration for a provider.
#[derive(Debug, Clone, Copy)]
pub struct ApiProbe {
    /// Environment variables that, when set, mean the API is configured.
    /// The probe is skipped when none of them is non-empty.
    pub env_vars: &'static [&'static str],
    /// `host:port` to TCP-connect for the reachability check.
    pub endpoint: &'static str,
}

impl ApiProbe {
    /// Whether the operator has credentials for this API.
    pub fn is_configured(&self) -> bool {
        self.env_vars
            .iter()
            .any(|var| std::env::var(var).is_ok_and(|v| !v.is_empty()))
    }
}

/// Static description of a provider CLI.
#[derive(Debug, Clone, Copy)]
pub struct ProviderSpec {
    /// Stable provider name used in reports and lookups.
    pub name: &'static str,
    /// Executable name resolved through `PATH`.
    pub binary: &'static str,
    /// Arguments of the (non-interactive) version command.
    pub version_args: &'static [&'static str],
    /// Optional API reachability probe.
    pub api: Option<ApiProbe>,
}

/// Providers the router knows how to drive.
///
/// `opencode` is deliberately probe-free: it is provider-agnostic and talks to
/// whichever upstream the operator configured, so there is no single endpoint
/// to reach. Its CLI checks still run.
pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        name: "claude",
        binary: "claude",
        version_args: &["--version"],
        api: Some(ApiProbe {
            env_vars: &["ANTHROPIC_API_KEY"],
            endpoint: "api.anthropic.com:443",
        }),
    },
    ProviderSpec {
        name: "opencode",
        binary: "opencode",
        version_args: &["--version"],
        api: None,
    },
    ProviderSpec {
        name: "agy",
        binary: "agy",
        version_args: &["--version"],
        api: Some(ApiProbe {
            env_vars: &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
            endpoint: "generativelanguage.googleapis.com:443",
        }),
    },
    ProviderSpec {
        name: "codex",
        binary: "codex",
        version_args: &["--version"],
        api: Some(ApiProbe {
            env_vars: &["OPENAI_API_KEY"],
            endpoint: "api.openai.com:443",
        }),
    },
    ProviderSpec {
        name: "wrangler",
        binary: "wrangler",
        version_args: &["--version"],
        api: Some(ApiProbe {
            env_vars: &["CLOUDFLARE_API_TOKEN"],
            endpoint: "api.cloudflare.com:443",
        }),
    },
];

/// Names of every provider the router probes, in report order.
pub fn provider_names() -> Vec<&'static str> {
    PROVIDERS.iter().map(|spec| spec.name).collect()
}

/// Health of a single provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Provider name (`claude`, `opencode`, ...).
    pub provider: String,
    /// Aggregated status of this provider.
    pub status: HealthStatus,
    /// Executable that was probed.
    pub binary: String,
    /// Absolute path of the resolved binary, when found.
    pub path: Option<String>,
    /// Version string reported by the CLI, when it responded.
    pub version: Option<String>,
    /// `None` when not probed (no probe configured / credentials missing),
    /// `Some(true/false)` when a TCP probe was actually made.
    pub api_reachable: Option<bool>,
    /// Wall time spent probing this provider.
    pub latency_ms: u64,
    /// Human-readable reason for a degraded/unhealthy result.
    pub message: Option<String>,
}

impl HealthCheck {
    fn failure(spec: &ProviderSpec, status: HealthStatus, message: impl Into<String>) -> Self {
        Self {
            provider: spec.name.to_string(),
            status,
            binary: spec.binary.to_string(),
            path: None,
            version: None,
            api_reachable: None,
            latency_ms: 0,
            message: Some(message.into()),
        }
    }
}

/// Result of one health sweep across all known providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    /// Worst status across `providers`.
    pub status: HealthStatus,
    /// RFC3339 timestamp of the sweep.
    pub checked_at: String,
    /// Whether the report came from the cache instead of a fresh sweep.
    pub cached: bool,
    /// Cache lifetime, in seconds.
    pub ttl_secs: u64,
    /// Per-provider results, in [`PROVIDERS`] order.
    pub providers: Vec<HealthCheck>,
}

impl HealthReport {
    /// Build a report from already-collected checks.
    pub fn from_checks(checks: Vec<HealthCheck>, cached: bool, ttl: Duration) -> Self {
        Self {
            status: aggregate_status(&checks),
            checked_at: chrono::Utc::now().to_rfc3339(),
            cached,
            ttl_secs: ttl.as_secs(),
            providers: checks,
        }
    }

    /// Look up one provider in the report.
    pub fn provider(&self, name: &str) -> Option<&HealthCheck> {
        self.providers.iter().find(|c| c.provider == name)
    }
}

/// Probe a single provider: binary presence, CLI responsiveness, API reach.
///
/// The whole probe is bounded by `budget`; children are killed on timeout so a
/// hung CLI cannot leak processes.
pub async fn check_provider(spec: &ProviderSpec, budget: Duration) -> HealthCheck {
    let started = Instant::now();
    match tokio::time::timeout(budget, check_provider_inner(spec, budget)).await {
        Ok(check) => check,
        Err(_) => HealthCheck {
            provider: spec.name.to_string(),
            status: HealthStatus::Degraded,
            binary: spec.binary.to_string(),
            path: None,
            version: None,
            api_reachable: None,
            latency_ms: elapsed_ms(started),
            message: Some(format!(
                "health check timed out after {}ms",
                budget.as_millis()
            )),
        },
    }
}

async fn check_provider_inner(spec: &ProviderSpec, budget: Duration) -> HealthCheck {
    let started = Instant::now();

    let Some(path) = locate_binary(spec.binary, budget.min(LOCATE_BUDGET)).await else {
        return HealthCheck {
            provider: spec.name.to_string(),
            status: HealthStatus::Unhealthy,
            binary: spec.binary.to_string(),
            path: None,
            version: None,
            api_reachable: None,
            latency_ms: elapsed_ms(started),
            message: Some(format!("CLI `{}` not found in PATH", spec.binary)),
        };
    };

    let mut status = HealthStatus::Healthy;
    let mut message = None;

    let version = match probe_version(&path, spec, budget.min(VERSION_BUDGET)).await {
        Ok(version) => Some(version),
        Err(err) => {
            status = HealthStatus::Degraded;
            message = Some(err);
            None
        },
    };

    // Only worth probing the API when the CLI itself is usable.
    let api_reachable = if status == HealthStatus::Healthy {
        let (reachable, api_message) = probe_api(spec, budget.min(API_BUDGET)).await;
        if reachable == Some(false) {
            status = HealthStatus::Degraded;
            message = api_message;
        }
        reachable
    } else {
        None
    };

    HealthCheck {
        provider: spec.name.to_string(),
        status,
        binary: spec.binary.to_string(),
        path: Some(path.display().to_string()),
        version,
        api_reachable,
        latency_ms: elapsed_ms(started),
        message,
    }
}

/// Probe every provider concurrently, preserving `specs` order.
pub async fn check_all(specs: &[ProviderSpec], budget: Duration) -> Vec<HealthCheck> {
    let mut set = tokio::task::JoinSet::new();
    for (idx, &spec) in specs.iter().enumerate() {
        set.spawn(async move { (idx, check_provider(&spec, budget).await) });
    }

    let mut slots: Vec<Option<HealthCheck>> = vec![None; specs.len()];
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((idx, check)) => slots[idx] = Some(check),
            Err(err) => warn!("provider health task failed: {}", err),
        }
    }

    slots
        .into_iter()
        .enumerate()
        .map(|(idx, slot)| {
            slot.unwrap_or_else(|| {
                HealthCheck::failure(&specs[idx], HealthStatus::Unhealthy, "health task failed")
            })
        })
        .collect()
}

/// Resolve a binary to an absolute path.
///
/// Uses `which` (the requirement's `which`/`command -v` check) and falls back
/// to scanning `PATH` when the utility is unavailable, as in minimal images.
pub async fn locate_binary(binary: &str, budget: Duration) -> Option<PathBuf> {
    if let Ok(Ok(output)) = tokio::time::timeout(
        budget,
        Command::new("which")
            .arg(binary)
            .kill_on_drop(true)
            .output(),
    )
    .await
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().map(str::trim).find(|l| !l.is_empty()) {
                let path = PathBuf::from(line);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }

    scan_path(binary)
}

/// Direct `PATH` scan used when `which` is missing or points nowhere.
fn scan_path(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Run the CLI version command and summarize its output.
async fn probe_version(
    path: &std::path::Path,
    spec: &ProviderSpec,
    budget: Duration,
) -> Result<String, String> {
    let args = spec.version_args;
    let output = tokio::time::timeout(
        budget,
        Command::new(path).args(args).kill_on_drop(true).output(),
    )
    .await;

    match output {
        Err(_) => Err(format!(
            "`{}` version check timed out after {}ms",
            spec.binary,
            budget.as_millis()
        )),
        Ok(Err(err)) => Err(format!("failed to run `{}`: {}", spec.binary, err)),
        Ok(Ok(output)) if !output.status.success() => Err(format!(
            "`{}` exited with status {}",
            spec.binary, output.status
        )),
        Ok(Ok(output)) => {
            let summary = summarize_output(&output.stdout, &output.stderr);
            if summary.is_empty() {
                Err(format!("`{}` produced no version output", spec.binary))
            } else {
                Ok(summary)
            }
        },
    }
}

/// Probe the provider API over TCP, when credentials are configured.
///
/// Returns `(None, None)` when there is nothing to probe, `(Some(true), None)`
/// on a successful connect, and `(Some(false), Some(reason))` otherwise.
async fn probe_api(spec: &ProviderSpec, budget: Duration) -> (Option<bool>, Option<String>) {
    let Some(probe) = spec.api else {
        return (None, None);
    };
    if !probe.is_configured() {
        return (None, None);
    }

    match tokio::time::timeout(budget, TcpStream::connect(probe.endpoint)).await {
        Ok(Ok(_stream)) => (Some(true), None),
        Ok(Err(err)) => (
            Some(false),
            Some(format!(
                "API endpoint {} unreachable: {}",
                probe.endpoint, err
            )),
        ),
        Err(_) => (
            Some(false),
            Some(format!(
                "API endpoint {} timed out after {}ms",
                probe.endpoint,
                budget.as_millis()
            )),
        ),
    }
}

/// First non-empty line of stdout (or stderr), truncated for reports.
fn summarize_output(stdout: &[u8], stderr: &[u8]) -> String {
    let has_stdout = stdout.iter().any(|b| !b.is_ascii_whitespace());
    let raw = if has_stdout { stdout } else { stderr };
    let text = String::from_utf8_lossy(raw);
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");

    if line.chars().count() > VERSION_MAX_CHARS {
        let mut truncated: String = line.chars().take(VERSION_MAX_CHARS).collect();
        truncated.push_str("...");
        truncated
    } else {
        line.to_string()
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

struct CachedReport {
    checks: Vec<HealthCheck>,
    at: Instant,
}

/// TTL cache in front of the provider probes.
///
/// The mutex is held across the sweep so concurrent callers trigger a single
/// round of checks (no thundering herd of `which`/version processes).
pub struct HealthCache {
    specs: Vec<ProviderSpec>,
    ttl: Duration,
    budget: Duration,
    cached: tokio::sync::Mutex<Option<CachedReport>>,
}

impl Default for HealthCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCache {
    /// Cache over [`PROVIDERS`] with [`DEFAULT_TTL`] and [`DEFAULT_TIMEOUT`].
    pub fn new() -> Self {
        Self::with_specs(PROVIDERS.to_vec(), DEFAULT_TTL, DEFAULT_TIMEOUT)
    }

    /// Cache with an explicit provider list, TTL and probe budget.
    pub fn with_specs(specs: Vec<ProviderSpec>, ttl: Duration, budget: Duration) -> Self {
        Self {
            specs,
            ttl,
            budget,
            cached: tokio::sync::Mutex::new(None),
        }
    }

    /// Provider names covered by this cache.
    pub fn provider_names(&self) -> Vec<&'static str> {
        self.specs.iter().map(|spec| spec.name).collect()
    }

    /// Return a fresh report when the cache is still valid, otherwise sweep.
    pub async fn report(&self, force: bool) -> HealthReport {
        let mut guard = self.cached.lock().await;
        let fresh = guard
            .as_ref()
            .filter(|entry| !force && entry.at.elapsed() < self.ttl)
            .map(|entry| entry.checks.clone());

        let (checks, cached) = match fresh {
            Some(checks) => (checks, true),
            None => {
                let checks = check_all(&self.specs, self.budget).await;
                *guard = Some(CachedReport {
                    checks: checks.clone(),
                    at: Instant::now(),
                });
                (checks, false)
            },
        };
        drop(guard);

        HealthReport::from_checks(checks, cached, self.ttl)
    }

    /// Health of one provider by name (`None` for unknown providers).
    pub async fn provider(&self, name: &str, force: bool) -> Option<HealthCheck> {
        if !self.specs.iter().any(|spec| spec.name == name) {
            return None;
        }
        self.report(force)
            .await
            .providers
            .into_iter()
            .find(|check| check.provider == name)
    }

    /// Drop the cached report so the next call performs a fresh sweep.
    pub async fn invalidate(&self) {
        *self.cached.lock().await = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &'static str, binary: &'static str) -> ProviderSpec {
        ProviderSpec {
            name,
            binary,
            version_args: &["--version"],
            api: None,
        }
    }

    fn shell_spec(name: &'static str, script: &'static [&'static str]) -> ProviderSpec {
        ProviderSpec {
            name,
            binary: "sh",
            version_args: script,
            api: None,
        }
    }

    fn check(status: HealthStatus) -> HealthCheck {
        HealthCheck {
            provider: "p".to_string(),
            status,
            binary: "p".to_string(),
            path: None,
            version: None,
            api_reachable: None,
            latency_ms: 0,
            message: None,
        }
    }

    #[test]
    fn status_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&HealthStatus::Healthy).unwrap(),
            "\"healthy\""
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Degraded).unwrap(),
            "\"degraded\""
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Unhealthy).unwrap(),
            "\"unhealthy\""
        );
        let parsed: HealthStatus = serde_json::from_str("\"degraded\"").unwrap();
        assert_eq!(parsed, HealthStatus::Degraded);
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn aggregate_picks_worst_status() {
        assert_eq!(aggregate_status(&[]), HealthStatus::Healthy);
        assert_eq!(
            aggregate_status(&[check(HealthStatus::Healthy), check(HealthStatus::Healthy)]),
            HealthStatus::Healthy
        );
        assert_eq!(
            aggregate_status(&[check(HealthStatus::Healthy), check(HealthStatus::Degraded)]),
            HealthStatus::Degraded
        );
        assert_eq!(
            aggregate_status(&[
                check(HealthStatus::Degraded),
                check(HealthStatus::Unhealthy)
            ]),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn provider_specs_are_complete() {
        assert!(provider_names().contains(&"claude"));
        assert!(provider_names().contains(&"opencode"));
        assert!(provider_names().contains(&"agy"));
        assert!(provider_names().contains(&"codex"));
        assert!(provider_names().contains(&"wrangler"));
        for spec in PROVIDERS {
            assert!(!spec.name.is_empty());
            assert!(!spec.binary.is_empty());
            assert!(
                !spec.version_args.is_empty(),
                "{} has no version args",
                spec.name
            );
        }
    }

    #[test]
    fn summarize_output_takes_first_non_empty_line() {
        assert_eq!(summarize_output(b"\n  1.2.3  \n", b""), "1.2.3");
        assert_eq!(summarize_output(b"  \n", b"warn: 9.9.9\n"), "warn: 9.9.9");
        assert_eq!(summarize_output(b"", b""), "");
    }

    #[test]
    fn summarize_output_truncates_long_versions() {
        let long = "v".repeat(VERSION_MAX_CHARS + 10);
        let summary = summarize_output(long.as_bytes(), b"");
        assert_eq!(summary.chars().count(), VERSION_MAX_CHARS + 3);
        assert!(summary.ends_with("..."));
    }

    #[tokio::test]
    async fn missing_binary_is_unhealthy() {
        let spec = spec("ghost-provider", "gestalt-definitely-not-a-real-cli-xyz");
        let check = check_provider(&spec, DEFAULT_TIMEOUT).await;
        assert_eq!(check.status, HealthStatus::Unhealthy);
        assert_eq!(check.provider, "ghost-provider");
        assert!(check.path.is_none());
        assert!(check.api_reachable.is_none());
        assert!(check.message.unwrap().contains("not found in PATH"));
    }

    #[tokio::test]
    async fn responsive_cli_is_healthy() {
        let spec = shell_spec("sh-ok", &["-c", "echo 1.2.3"]);
        let check = check_provider(&spec, DEFAULT_TIMEOUT).await;
        if check.path.is_none() {
            // No `sh` in PATH: nothing portable left to assert.
            return;
        }
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.version.as_deref(), Some("1.2.3"));
        assert!(check.api_reachable.is_none(), "no api probe configured");
        assert!(check.message.is_none());
    }

    #[tokio::test]
    async fn failing_version_command_is_degraded() {
        let spec = shell_spec("sh-bad", &["-c", "exit 3"]);
        let check = check_provider(&spec, DEFAULT_TIMEOUT).await;
        if check.path.is_none() {
            return;
        }
        assert_eq!(check.status, HealthStatus::Degraded);
        assert!(check.version.is_none());
        assert!(check.message.unwrap().contains("exited with status"));
    }

    #[tokio::test]
    async fn silent_version_output_is_degraded() {
        let spec = shell_spec("sh-silent", &["-c", "true"]);
        let check = check_provider(&spec, DEFAULT_TIMEOUT).await;
        if check.path.is_none() {
            return;
        }
        assert_eq!(check.status, HealthStatus::Degraded);
        assert!(check.message.unwrap().contains("no version output"));
    }

    #[tokio::test]
    async fn unreachable_api_is_degraded() {
        let spec = ProviderSpec {
            name: "sh-api",
            binary: "sh",
            version_args: &["-c", "echo 2.0.0"],
            // PATH is always set, so the probe actually runs; port 1 is closed.
            api: Some(ApiProbe {
                env_vars: &["PATH"],
                endpoint: "127.0.0.1:1",
            }),
        };
        let check = check_provider(&spec, DEFAULT_TIMEOUT).await;
        if check.path.is_none() {
            return;
        }
        assert_eq!(check.status, HealthStatus::Degraded);
        assert_eq!(check.api_reachable, Some(false));
        assert!(check.message.unwrap().contains("unreachable"));
    }

    #[tokio::test]
    async fn unconfigured_api_probe_is_skipped() {
        let spec = ProviderSpec {
            name: "sh-nokey",
            binary: "sh",
            version_args: &["-c", "echo 3.0.0"],
            api: Some(ApiProbe {
                env_vars: &["GESTALT_HEALTH_TEST_KEY_UNSET"],
                endpoint: "127.0.0.1:1",
            }),
        };
        let check = check_provider(&spec, DEFAULT_TIMEOUT).await;
        if check.path.is_none() {
            return;
        }
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.api_reachable, None);
    }

    #[tokio::test]
    async fn cache_serves_cached_report_within_ttl() {
        let cache = HealthCache::with_specs(
            vec![shell_spec("sh-cache", &["-c", "echo 4.0.0"])],
            Duration::from_secs(60),
            DEFAULT_TIMEOUT,
        );

        let first = cache.report(false).await;
        assert!(!first.cached);
        assert_eq!(first.status, HealthStatus::Healthy);
        assert_eq!(first.ttl_secs, 60);

        let second = cache.report(false).await;
        assert!(second.cached, "second call must reuse the cached report");
        assert_eq!(second.providers, first.providers);
    }

    #[tokio::test]
    async fn cache_refreshes_after_ttl_or_force() {
        let cache = HealthCache::with_specs(
            vec![shell_spec("sh-refresh", &["-c", "echo 5.0.0"])],
            Duration::ZERO,
            DEFAULT_TIMEOUT,
        );

        assert!(!cache.report(false).await.cached);
        assert!(
            !cache.report(false).await.cached,
            "zero TTL is always stale"
        );

        let cache = HealthCache::with_specs(
            vec![shell_spec("sh-force", &["-c", "echo 6.0.0"])],
            Duration::from_secs(60),
            DEFAULT_TIMEOUT,
        );
        assert!(!cache.report(false).await.cached);
        assert!(
            !cache.report(true).await.cached,
            "force bypasses a fresh cache"
        );
    }

    #[tokio::test]
    async fn cache_invalidate_forces_refresh() {
        let cache = HealthCache::with_specs(
            vec![shell_spec("sh-inv", &["-c", "echo 7.0.0"])],
            Duration::from_secs(60),
            DEFAULT_TIMEOUT,
        );
        assert!(!cache.report(false).await.cached);
        assert!(cache.report(false).await.cached);
        cache.invalidate().await;
        assert!(!cache.report(false).await.cached);
    }

    #[tokio::test]
    async fn provider_lookup_handles_unknown_names() {
        let cache = HealthCache::with_specs(
            vec![shell_spec("sh-lookup", &["-c", "echo 8.0.0"])],
            Duration::from_secs(60),
            DEFAULT_TIMEOUT,
        );
        assert!(cache.provider("nope", false).await.is_none());
        let found = cache.provider("sh-lookup", false).await.unwrap();
        assert_eq!(found.provider, "sh-lookup");
    }

    #[tokio::test]
    async fn report_orders_providers_as_configured() {
        let cache = HealthCache::with_specs(
            vec![
                shell_spec("first", &["-c", "echo 1"]),
                spec("second", "gestalt-not-a-real-cli-xyz"),
            ],
            Duration::from_secs(60),
            DEFAULT_TIMEOUT,
        );
        let report = cache.report(false).await;
        let names: Vec<&str> = report
            .providers
            .iter()
            .map(|c| c.provider.as_str())
            .collect();
        assert_eq!(names, vec!["first", "second"]);
        // A missing CLI must degrade the aggregate report.
        assert_eq!(report.status, HealthStatus::Unhealthy);
        assert!(report.provider("second").is_some());
    }
}
