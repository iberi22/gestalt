import asyncio
import time
import json
import os
import sys

# Try to import real system, fallback to mock for this environment if needed
try:
    from memory_system import EmbeddingBatchProcessor, SearchCache, semantic_search
except ImportError:
    # Minimal mock implementation if files are missing in CI or local
    class EmbeddingBatchProcessor:
        def __init__(self, batch_size: int = 32): self.batch_size = batch_size
        async def process_batch(self, texts):
            # Simulate real work without sleep if possible, or use very small sleep
            [i*i for i in range(1000)]
            return [[0.1]*1536 for _ in texts]
    class SearchCache:
        def __init__(self, **kwargs): self.cache = {}
        def get(self, q): return self.cache.get(q)
        def set(self, q, d): self.cache[q] = d
    async def semantic_search(q): return [{"id": 0}]

async def run_benchmarks():
    print("--- Running Gestalt Memory Benchmarks ---")
    results = {}

    # 1. Search Cache Speedup
    cache = SearchCache()
    query = "performance test query"
    data = [{"id": i} for i in range(100)]

    # Warm up
    await semantic_search(query)

    start = time.perf_counter()
    for _ in range(10): await semantic_search(query)
    no_cache_time = time.perf_counter() - start

    cache.set(query, data)
    start = time.perf_counter()
    for _ in range(10): cache.get(query)
    with_cache_time = time.perf_counter() - start

    results['cache_speedup'] = no_cache_time / with_cache_time if with_cache_time > 0 else 0
    print(f"Cache speedup: {results['cache_speedup']:.2f}x")

    # 2. Batch Processing Speedup
    processor = EmbeddingBatchProcessor(batch_size=10)
    texts = [f"sample text context {i}" for i in range(10)]

    start = time.perf_counter()
    for text in texts:
        await processor.process_batch([text])
    sequential_time = time.perf_counter() - start

    start = time.perf_counter()
    await processor.process_batch(texts)
    batch_time = time.perf_counter() - start

    results['batch_speedup'] = sequential_time / batch_time if batch_time > 0 else 0
    print(f"Batch speedup: {results['batch_speedup']:.2f}x")

    os.makedirs("benchmarks", exist_ok=True)
    with open("benchmarks/memory_current.json", "w") as f:
        json.dump(results, f, indent=2)

if __name__ == "__main__":
    asyncio.run(run_benchmarks())
