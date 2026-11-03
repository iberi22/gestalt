//! Integration tests for provider health checks wired through the Router.
//!
//! The probes are exercised against `sh` and a non-existent binary so the
//! suite stays deterministic and offline; the unit tests in `src/health.rs`
//! cover the individual layers.

use gestalt_router::agent::SubprocessRunner;
use gestalt_router::health::{
    ApiProbe, HealthCache, HealthStatus, ProviderSpec, DEFAULT_TIMEOUT, DEFAULT_TTL,
};
use gestalt_router::router::Router;
use gestalt_router::run_state::MemState;
use gestalt_router::run_state::StateDb;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn shell_spec(name: &'static str, script: &'static [&'static str]) -> ProviderSpec {
    ProviderSpec {
        name,
        binary: "sh",
        version_args: script,
        api: None,
    }
}

/// Temporary StateDb removed (with its WAL sidecars) when the test ends.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        Self {
            path: std::env::temp_dir()
                .join(format!("gestalt-health-it-{}.db", uuid::Uuid::new_v4())),
        }
    }

    fn open(&self) -> Arc<StateDb> {
        Arc::new(StateDb::open(&self.path).expect("open state db"))
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.path.display(), suffix));
        }
    }
}

fn router_with(cache: Arc<HealthCache>) -> (Router, TempDb) {
    let db = TempDb::new();
    let router = Router::new(
        None,
        Arc::new(SubprocessRunner::new(Duration::from_secs(5))),
        db.open(),
        MemState::new(),
        None,
        None,
    )
    .with_health_cache(cache);
    (router, db)
}

#[tokio::test]
async fn router_health_report_uses_injected_cache() {
    let cache = Arc::new(HealthCache::with_specs(
        vec![
            shell_spec("sh-ok", &["-c", "echo 9.9.9"]),
            ProviderSpec {
                name: "ghost",
                binary: "gestalt-not-a-real-cli-xyz",
                version_args: &["--version"],
                api: None,
            },
        ],
        DEFAULT_TTL,
        DEFAULT_TIMEOUT,
    ));
    let (router, _db) = router_with(cache);

    let report = router.health_report(false).await;
    assert_eq!(report.ttl_secs, DEFAULT_TTL.as_secs());
    assert!(!report.cached, "first sweep is not cached");
    assert_eq!(report.providers.len(), 2);
    assert_eq!(report.status, HealthStatus::Unhealthy);
    assert_eq!(
        report.provider("ghost").unwrap().status,
        HealthStatus::Unhealthy
    );

    // Within the TTL the router must serve the cached sweep.
    assert!(router.health_report(false).await.cached);
    assert!(!router.health_report(true).await.cached, "force re-probes");
}

#[tokio::test]
async fn router_provider_health_handles_known_and_unknown_names() {
    let cache = Arc::new(HealthCache::with_specs(
        vec![shell_spec("sh-single", &["-c", "echo 1.0.0"])],
        DEFAULT_TTL,
        DEFAULT_TIMEOUT,
    ));
    let (router, _db) = router_with(cache);

    assert!(router
        .provider_health("unknown-provider", false)
        .await
        .is_none());

    let check = router
        .provider_health("sh-single", false)
        .await
        .expect("known provider must be reported");
    assert_eq!(check.provider, "sh-single");
    assert_eq!(check.version.as_deref(), Some("1.0.0"));
    assert_eq!(check.status, HealthStatus::Healthy);
}

#[tokio::test]
async fn router_reports_api_degradation() {
    let cache = Arc::new(HealthCache::with_specs(
        vec![ProviderSpec {
            name: "sh-api",
            binary: "sh",
            version_args: &["-c", "echo 2.0.0"],
            // PATH is always set, so the probe really runs; port 1 is closed.
            api: Some(ApiProbe {
                env_vars: &["PATH"],
                endpoint: "127.0.0.1:1",
            }),
        }],
        DEFAULT_TTL,
        DEFAULT_TIMEOUT,
    ));
    let (router, _db) = router_with(cache);

    let check = router.provider_health("sh-api", false).await.unwrap();
    assert_eq!(check.status, HealthStatus::Degraded);
    assert_eq!(check.api_reachable, Some(false));
    assert!(check.message.unwrap().contains("unreachable"));
}
