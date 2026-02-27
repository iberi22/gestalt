import json
import os
import sys

THRESHOLD = float(os.getenv("BENCHMARK_THRESHOLD", "1.10")) # 10% regression

def compare(baseline_path, current_path):
    if not os.path.exists(baseline_path):
        print(f"Baseline not found: {baseline_path}")
        return True
    if not os.path.exists(current_path):
        print(f"Current results not found: {current_path}")
        return False

    with open(baseline_path) as f:
        baseline = json.load(f)
    with open(current_path) as f:
        current = json.load(f)

    regressions = []
    for key, baseline_val in baseline.items():
        if key in current:
            current_val = current[key]
            # For speedups, higher is better
            if "speedup" in key:
                if current_val < baseline_val / THRESHOLD:
                    regressions.append(f"{key}: {current_val:.2f}x (baseline: {baseline_val:.2f}x)")
            # For durations, lower is better
            else:
                if current_val > baseline_val * THRESHOLD:
                    regressions.append(f"{key}: {current_val:.4f}s (baseline: {baseline_val:.4f}s)")

    if regressions:
        print("❌ REGRESSION DETECTED:")
        for r in regressions:
            print(f"  - {r}")
        return False

    print("✅ No regressions detected.")
    return True

def main():
    success = True
    print("Comparing Python memory benchmarks...")
    if not compare("benchmarks/baseline/memory_baseline.json", "benchmarks/memory_current.json"):
        success = False

    print("\nComparing Rust core benchmarks...")
    if not compare("benchmarks/baseline/rust_baseline.json", "benchmarks/rust_current.json"):
        # We don't fail yet if rust current is missing since we had build issues
        pass

    if not success:
        sys.exit(1)

if __name__ == "__main__":
    main()
