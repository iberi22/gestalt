pub mod artifact_ingest;
pub mod inject;
pub mod orca_bridge;
pub mod proc_monitor;

use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveryResults {
    pub path_agents: Vec<PathBuf>,
    pub config_dirs: Vec<PathBuf>,
    pub orca_hooks: Vec<PathBuf>,
}

pub fn discover_agents() -> Result<DiscoveryResults, String> {
    // Walk PATH directories
    let target_binaries = [
        "agy", "kimi", "opencode", "hermes", "gestalt", "claude", "codex", "orca", "jules", "agent",
    ];

    let mut path_agents = Vec::new();
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for binary_name in &target_binaries {
                let binary_path = dir.join(binary_name);
                if binary_path.is_file() {
                    // Check if it is already in our list (avoid duplicates from different PATH entries)
                    if !path_agents.contains(&binary_path) {
                        path_agents.push(binary_path);
                    }
                }
            }
        }
    }

    // Walk known config dirs
    let known_config_paths = [
        ".config/opencode",
        ".codex",
        ".claude",
        ".kimi",
        ".hermes",
        ".config/orca",
        ".config/agy",
        ".gestalt",
        ".config/gestalt",
    ];

    let mut config_dirs = Vec::new();
    if let Some(home) = home::home_dir() {
        for rel_path in &known_config_paths {
            let full_path = home.join(rel_path);
            if full_path.is_dir() {
                config_dirs.push(full_path);
            }
        }
    }

    // Walk Orca hooks
    let mut orca_hooks = Vec::new();
    if let Some(home) = home::home_dir() {
        let hooks_dir = home.join(".orca/agent-hooks");
        if hooks_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(hooks_dir) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        orca_hooks.push(entry.path());
                    }
                }
            }
        }
    }

    Ok(DiscoveryResults {
        path_agents,
        config_dirs,
        orca_hooks,
    })
}

pub async fn run_daemon_loop() -> Result<(), String> {
    println!("[Daemon] Starting observation daemon loop (ticking every 5s)...");
    loop {
        println!("[Daemon] Ticking process monitor...");
        if let Err(e) = proc_monitor::monitor_processes() {
            // It is expected to return "not implemented" for now
            tracing::debug!("Process monitor tick warning: {}", e);
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        orig_home: Option<std::ffi::OsString>,
        orig_path: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn lock() -> Self {
            let lock = ENV_MUTEX.lock().unwrap();
            let orig_home = std::env::var_os("HOME");
            let orig_path = std::env::var_os("PATH");
            Self {
                _lock: lock,
                orig_home,
                orig_path,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref h) = self.orig_home {
                std::env::set_var("HOME", h);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(ref p) = self.orig_path {
                std::env::set_var("PATH", p);
            } else {
                std::env::remove_var("PATH");
            }
        }
    }

    #[test]
    fn test_discover_agents_empty() {
        let _guard = EnvGuard::lock();

        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = temp_dir.path();

        std::env::set_var("HOME", home_path);
        // Safely clear PATH completely for the duration of this serialized test
        std::env::set_var("PATH", home_path.join("nonexistent"));

        let results = discover_agents().unwrap();

        assert_eq!(results.path_agents.len(), 0);
        assert_eq!(results.config_dirs.len(), 0);
        assert_eq!(results.orca_hooks.len(), 0);
    }

    #[test]
    fn test_discover_agents_populated() {
        let _guard = EnvGuard::lock();

        let temp_dir = tempfile::tempdir().unwrap();
        let home_path = temp_dir.path();

        // Create mock config dirs
        let opencode_dir = home_path.join(".config/opencode");
        fs::create_dir_all(&opencode_dir).unwrap();

        let hermes_dir = home_path.join(".hermes");
        fs::create_dir_all(&hermes_dir).unwrap();

        // Create mock PATH binary dir
        let bin_dir = home_path.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let hermes_bin = bin_dir.join("hermes");
        fs::write(&hermes_bin, b"").unwrap();
        let orca_bin = bin_dir.join("orca");
        fs::write(&orca_bin, b"").unwrap();

        // Create mock Orca hook dir
        let hooks_dir = home_path.join(".orca/agent-hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook1 = hooks_dir.join("hook1");
        fs::write(&hook1, b"").unwrap();

        std::env::set_var("HOME", home_path);

        // Prepend our bin_dir to original PATH
        let old_path = std::env::var_os("PATH").unwrap_or_default();
        let mut path_dirs = vec![bin_dir];
        for p in std::env::split_paths(&old_path) {
            path_dirs.push(p);
        }
        let new_path = std::env::join_paths(path_dirs).unwrap();
        std::env::set_var("PATH", new_path);

        let results = discover_agents().unwrap();

        assert_eq!(results.path_agents.len(), 2);
        assert!(results.path_agents.iter().any(|p| p.ends_with("hermes")));
        assert!(results.path_agents.iter().any(|p| p.ends_with("orca")));

        assert_eq!(results.config_dirs.len(), 2);
        assert!(results
            .config_dirs
            .iter()
            .any(|p| p.ends_with(".config/opencode")));
        assert!(results.config_dirs.iter().any(|p| p.ends_with(".hermes")));

        assert_eq!(results.orca_hooks.len(), 1);
        assert!(results.orca_hooks[0].ends_with("hook1"));
    }
}
