use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }

    pub fn estimate(prompt: &str, completion: &str) -> Self {
        // A simple estimation: ~4 characters per token
        let prompt_tokens = (prompt.len() / 4).max(1) as u32;
        let completion_tokens = (completion.len() / 4).max(1) as u32;
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub agent_id: String,
    pub agent_type: String,
    pub duration_ms: u64,
    pub token_usage: Option<TokenUsage>,
    pub model: String,
    pub provider: String,
    pub tools_used: usize,
    pub success: bool,
    pub cost_estimate: f64,
    pub cold_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionReport {
    pub total_agents: usize,
    pub success_rate: f64,
    pub total_tokens: u32,
    pub total_cost: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub p99_cold_latency_ms: f64,
    pub p99_warm_latency_ms: f64,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    metrics: Arc<RwLock<Vec<AgentMetrics>>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn collect(&self, metric: AgentMetrics) {
        let mut m = self.metrics.write().await;
        m.push(metric);
    }

    pub async fn get_metrics(&self) -> Vec<AgentMetrics> {
        let m = self.metrics.read().await;
        m.clone()
    }

    pub async fn clear(&self) {
        let mut m = self.metrics.write().await;
        m.clear();
    }
}

pub struct MetricsAggregator;

impl MetricsAggregator {
    pub fn calculate_cost(provider: &str, model: &str, prompt: u32, completion: u32) -> f64 {
        // Simple cost calculation helper based on model and provider rates
        let (input_rate, output_rate) = match provider.to_lowercase().as_str() {
            "gemini" => {
                if model.contains("pro") {
                    (0.00000125, 0.000005) // $1.25/1M, $5.00/1M
                } else {
                    (0.000000075, 0.0000003) // $0.075/1M, $0.30/1M
                }
            }
            "groq" => (0.00000059, 0.00000079), // $0.59/1M, $0.79/1M
            "minimax" => (0.00000015, 0.00000015), // $0.15/1M, $0.15/1M
            _ => (0.000001, 0.000002), // default fallback
        };
        (prompt as f64 * input_rate) + (completion as f64 * output_rate)
    }

    pub fn aggregate(metrics: &[AgentMetrics]) -> SessionReport {
        if metrics.is_empty() {
            return SessionReport::default();
        }

        let total_agents = metrics.len();
        let successes = metrics.iter().filter(|m| m.success).count();
        let success_rate = successes as f64 / total_agents as f64;

        let total_tokens: u32 = metrics
            .iter()
            .map(|m| m.token_usage.as_ref().map(|u| u.total_tokens).unwrap_or(0))
            .sum();

        let total_cost: f64 = metrics.iter().map(|m| m.cost_estimate).sum();

        let mut latencies: Vec<u64> = metrics.iter().map(|m| m.duration_ms).collect();
        latencies.sort_unstable();

        let avg_latency_ms = latencies.iter().sum::<u64>() as f64 / total_agents as f64;

        let percentile = |sorted: &[u64], p: f64| -> f64 {
            if sorted.is_empty() {
                return 0.0;
            }
            let idx = ((sorted.len() as f64 * p).round() as usize).min(sorted.len() - 1);
            sorted[idx] as f64
        };

        let p50_latency_ms = percentile(&latencies, 0.50);
        let p95_latency_ms = percentile(&latencies, 0.95);
        let p99_latency_ms = percentile(&latencies, 0.99);

        // Separate cold starts vs warm starts
        let mut cold_latencies: Vec<u64> = metrics
            .iter()
            .filter(|m| m.cold_start)
            .map(|m| m.duration_ms)
            .collect();
        cold_latencies.sort_unstable();
        let p99_cold_latency_ms = percentile(&cold_latencies, 0.99);

        let mut warm_latencies: Vec<u64> = metrics
            .iter()
            .filter(|m| !m.cold_start)
            .map(|m| m.duration_ms)
            .collect();
        warm_latencies.sort_unstable();
        let p99_warm_latency_ms = percentile(&warm_latencies, 0.99);

        // Generate warnings
        let mut warnings = Vec::new();
        if success_rate < 0.80 {
            warnings.push(format!(
                "Degraded performance alert: success rate is {:.1}% (under baseline of 80%)",
                success_rate * 100.0
            ));
        }
        if p95_latency_ms > 10000.0 {
            warnings.push(format!(
                "High latency alert: P95 latency is {:.1}ms",
                p95_latency_ms
            ));
        }

        // Generate actionable recommendations
        let mut recommendations = Vec::new();
        if success_rate < 0.80 {
            recommendations.push(
                "Verify agent prompt structures and provider API limits. Success rate has fallen below critical 80%."
                    .to_string(),
            );
        }
        if p95_latency_ms > 10000.0 {
            recommendations.push(
                "P95 latency exceeds 10s. Consider optimizing tools, reducing output tokens, or upgrading to a faster model/provider."
                    .to_string(),
            );
        }
        if total_cost > 5.0 {
            recommendations.push(
                "Cost exceeds threshold of $5.0. Consider using more specialized, smaller models for non-complex subtasks."
                    .to_string(),
            );
        }
        if !cold_latencies.is_empty() && p99_cold_latency_ms > p99_warm_latency_ms * 2.0 {
            recommendations.push(
                format!(
                    "Significant cold start overhead detected (P99 Cold: {:.1}ms vs P99 Warm: {:.1}ms). Increase pool pre_warm size or keep agents warm.",
                    p99_cold_latency_ms, p99_warm_latency_ms
                )
            );
        }

        SessionReport {
            total_agents,
            success_rate,
            total_tokens,
            total_cost,
            avg_latency_ms,
            p50_latency_ms,
            p95_latency_ms,
            p99_latency_ms,
            p99_cold_latency_ms,
            p99_warm_latency_ms,
            warnings,
            recommendations,
        }
    }
}

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn generate_markdown(report: &SessionReport) -> String {
        let mut md = String::new();
        md.push_str("# 🐝 Agent Session Execution Report\n\n");
        md.push_str("## 📊 Summary Metrics\n");
        md.push_str(&format!("- **Total Agents Run:** {}\n", report.total_agents));
        md.push_str(&format!("- **Success Rate:** {:.1}%\n", report.success_rate * 100.0));
        md.push_str(&format!("- **Total Tokens Consumed:** {}\n", report.total_tokens));
        md.push_str(&format!("- **Estimated Cost:** ${:.6}\n", report.total_cost));
        md.push_str(&format!("- **Average Latency:** {:.1} ms\n", report.avg_latency_ms));
        md.push_str(&format!("- **P50 Latency:** {:.1} ms\n", report.p50_latency_ms));
        md.push_str(&format!("- **P95 Latency:** {:.1} ms\n", report.p95_latency_ms));
        md.push_str(&format!("- **P99 Latency:** {:.1} ms\n", report.p99_latency_ms));
        md.push_str(&format!("- **P99 Cold-Start Latency:** {:.1} ms\n", report.p99_cold_latency_ms));
        md.push_str(&format!("- **P99 Warm-Start Latency:** {:.1} ms\n", report.p99_warm_latency_ms));
        md.push_str("\n");

        if !report.warnings.is_empty() {
            md.push_str("## ⚠️ Performance Alerts\n");
            for warning in &report.warnings {
                md.push_str(&format!("- **Alert:** {}\n", warning));
            }
            md.push_str("\n");
        }

        md.push_str("## 💡 Actionable Recommendations\n");
        if report.recommendations.is_empty() {
            md.push_str("- All metrics are within nominal limits. Keep up the good work!\n");
        } else {
            for recommendation in &report.recommendations {
                md.push_str(&format!("- {}\n", recommendation));
            }
        }

        md
    }

    pub fn generate_json(report: &SessionReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_default()
    }
}

pub struct MetricsStore;

impl MetricsStore {
    pub fn save_metrics(path: &Path, metrics: &[AgentMetrics]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(metrics)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load_metrics(path: &Path) -> anyhow::Result<Vec<AgentMetrics>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        let metrics = serde_json::from_str(&content)?;
        Ok(metrics)
    }
}
