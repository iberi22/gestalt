import numpy as np

class CompressedIndex:
    def __init__(self, original_dim: int = 1536, compressed_dim: int = 256):
        self.original_dim = original_dim
        self.compressed_dim = compressed_dim
        self.vectors = None

    def compress(self, vectors: np.ndarray):
        # Mock compression (taking first N dimensions)
        return vectors[:, :self.compressed_dim]
