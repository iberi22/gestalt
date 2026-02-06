# FASE 7: Optimization - Pending Tasks

Generated: 2026-02-06
Priority: LOW

## Tasks

---

### 7.1: Batch Processing para Embeddings

**Agent:** main
**Status:** Pending

**Objective:**
Implement batch processing to optimize embedding generation.

**Implementation:**
```python
# skills/memory_system.py (additions)

class EmbeddingBatchProcessor:
    """Process embeddings in batches for efficiency."""
    
    def __init__(self, batch_size: int = 32):
        self.batch_size = batch_size
        self.queue = []
    
    async def add(self, text: str) -> str:
        """Add text to batch queue."""
        self.queue.append(text)
        if len(self.queue) >= self.batch_size:
            await self.process_batch()
    
    async def process_batch(self) -> List[str]:
        """Process all queued texts in a single batch."""
        if not self.queue:
            return []
        
        texts = self.queue[:]
        self.queue.clear()
        
        # Call embedding API once for all texts
        embeddings = await self.embedding_client.embed_batch(texts)
        return embeddings
    
    async def flush(self):
        """Process remaining items in queue."""
        await self.process_batch()
```

**Files to Modify:**
- `skills/memory_system.py`

**Acceptance Criteria:**
- [ ] Batch processing working
- [ ] API calls reduced by >50%
- [ ] Memory usage stable

---

### 7.2: Cache de Búsqueda

**Agent:** main
**Status:** Pending

**Objective:**
Implement search result caching to reduce repeated computations.

**Implementation:**
```python
# skills/memory_system.py (additions)

from functools import lru_cache
import hashlib
import json

class SearchCache:
    """LRU cache for search results."""
    
    def __init__(self, max_size: int = 1000, ttl_seconds: int = 3600):
        self.cache = {}
        self.max_size = max_size
        self.ttl = ttl_seconds
    
    def _make_key(self, query: str, **kwargs) -> str:
        """Create cache key from query and params."""
        key_data = {"q": query, **kwargs}
        return hashlib.md5(json.dumps(key_data).encode()).hexdigest()
    
    def get(self, query: str, **kwargs) -> Optional[SearchResult]:
        """Get cached result or None."""
        key = self._make_key(query, **kwargs)
        if key in self.cache:
            result, timestamp = self.cache[key]
            if time.time() - timestamp < self.ttl:
                return result
            del self.cache[key]
        return None
    
    def set(self, query: str, result: SearchResult, **kwargs):
        """Store result in cache."""
        key = self._make_key(query, **kwargs)
        # Evict oldest if at capacity
        if len(self.cache) >= self.max_size:
            oldest_key = next(iter(self.cache))
            del self.cache[oldest_key]
        self.cache[key] = (result, time.time())
```

**Files to Modify:**
- `skills/memory_system.py`

**Acceptance Criteria:**
- [ ] Cache working
- [ ] Hit rate > 30%
- [ ] TTL configurable

---

### 7.3: Compresión de Índice

**Agent:** main
**Status:** Pending

**Objective:**
Compress search index to reduce memory usage.

**Implementation:**
```python
# skills/memory_system.py (additions)

import numpy as np
from sklearn.decomposition import PCA

class CompressedIndex:
    """Compressed vector index using PCA."""
    
    def __init__(self, original_dim: int = 1536, compressed_dim: int = 256):
        self.pca = PCA(n_components=compressed_dim)
        self.compressed_vectors = None
        self.original_dim = original_dim
        self.compressed_dim = compressed_dim
    
    def fit(self, vectors: np.ndarray):
        """Fit PCA on vectors."""
        self.compressed_vectors = self.pca.fit_transform(vectors)
        print(f"Compression: {self.original_dim} -> {self.compressed_dim} ({100*(1-self.compressed_dim/self.original_dim):.1f}% reduction)")
    
    def search(self, query_vector: np.ndarray, top_k: int = 10) -> np.ndarray:
        """Search using compressed vectors."""
        compressed_query = self.pca.transform([query_vector])[0]
        # Use compressed vectors for search
        similarities = np.dot(self.compressed_vectors, compressed_query)
        top_indices = np.argsort(similarities)[-top_k:][::-1]
        return top_indices
    
    @property
    def compression_ratio(self) -> float:
        """Return compression ratio."""
        return self.compressed_dim / self.original_dim
```

**Files to Create:**
- `skills/index_compression.py`

**Acceptance Criteria:**
- [ ] Compression working
- [ ] Memory reduction > 50%
- [ ] Search accuracy preserved

---

### 7.4: Parallel Processing

**Agent:** main
**Status:** Pending

**Objective:**
Implement parallel processing for concurrent operations.

**Implementation:**
```python
# skills/memory_system.py (additions)

import asyncio
from concurrent.futures import ThreadPoolExecutor

class ParallelProcessor:
    """Process multiple operations in parallel."""
    
    def __init__(self, max_workers: int = 4):
        self.executor = ThreadPoolExecutor(max_workers=max_workers)
        self.semaphore = asyncio.Semaphore(max_workers)
    
    async def process_in_parallel(self, tasks: List[Callable], *args) -> List[Any]:
        """Execute multiple tasks concurrently."""
        async def run_task(task, *args):
            async with self.semaphore:
                loop = asyncio.get_event_loop()
                return await loop.run_in_executor(self.executor, task, *args)
        
        results = await asyncio.gather(*[run_task(task, *args) for task in tasks])
        return results
    
    async def search_multiple_queries(self, queries: List[str]) -> List[SearchResult]:
        """Search multiple queries in parallel."""
        async def search(q):
            return await semantic_search(q)
        
        return await self.process_in_parallel([search] * len(queries))
```

**Files to Modify:**
- `skills/memory_system.py`

**Acceptance Criteria:**
- [ ] Parallel processing working
- [ ] 2x+ speedup for batch operations
- [ ] No race conditions

---

## Files to Create/Modify

```
skills/
├── memory_system.py        (MODIFY - add optimizations)
├── index_compression.py   (NEW - compression class)
└── cache.py              (NEW - caching layer)
```

---

## Performance Targets

| Metric | Target |
|--------|--------|
| Batch API calls | -50% |
| Cache hit rate | >30% |
| Memory usage | -50% |
| Parallel speedup | 2x+ |

---

## Definition of Done

- [ ] All optimizations implemented
- [ ] Performance targets met
- [ ] Benchmarks added
- [ ] Documentation updated
