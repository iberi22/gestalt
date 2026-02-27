use std::time::Instant;
use serde_json::json;

fn main() {
    println!("--- Running Gestalt Core Rust Benchmarks ---");
    let mut results = std::collections::HashMap::new();

    // Benchmark 1: JSON serialization overhead
    let start = Instant::now();
    for _ in 0..10000 {
        let val = json!({
            "id": "bench-id",
            "type": "CommandExecuted",
            "payload": { "cmd": "ls", "args": ["-la", "/tmp"] }
        });
        let _s = serde_json::to_string(&val).unwrap();
    }
    let duration = start.elapsed();
    results.insert("json_serialization_10k_ops", duration.as_secs_f64());

    // Benchmark 2: Mock logic throughput
    let start = Instant::now();
    let mut _sum: u64 = 0;
    for i in 0..10_000_000 {
        _sum = _sum.wrapping_add(i ^ (i >> 1));
    }
    let duration = start.elapsed();
    results.insert("logic_throughput_10m_ops", duration.as_secs_f64());

    let json_results = serde_json::to_string_pretty(&results).unwrap();
    let _ = std::fs::create_dir_all("benchmarks");
    let _ = std::fs::write("benchmarks/rust_current.json", json_results);
}
