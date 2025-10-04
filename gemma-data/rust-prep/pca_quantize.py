#!/usr/bin/env python3
"""
Simple PCA + 8-bit quantization script.
Reads embeddings.npy, outputs embeddings_64d_q8.bin
"""
import numpy as np
from sklearn.decomposition import PCA
import struct
import sys

# Config
INPUT = "../../gemma-data/embeddings.npy"
OUTPUT = "../../gemma-data/embeddings_64d_q8.bin"
TARGET_DIM = 64

print("Loading embeddings...")
emb = np.load(INPUT, mmap_mode="r")
vocab_size, orig_dim = emb.shape
print(f"  Shape: {emb.shape}")

# PCA
print(f"Running PCA to {TARGET_DIM} dimensions...")
pca = PCA(n_components=TARGET_DIM)
emb_reduced = pca.fit_transform(emb)
explained_var = pca.explained_variance_ratio_.sum()
print(f"  Explained variance: {explained_var * 100:.2f}%")

# Quantize per-token (symmetric)
print("Quantizing to 8-bit...")
scales = []
quantized = []

for i in range(vocab_size):
    row = emb_reduced[i]
    abs_max = np.abs(row).max()
    scale = abs_max / 127.0 if abs_max > 0 else 1.0
    scales.append(scale)
    
    # Quantize to i8 then store as u8
    q_row = np.clip(np.round(row / scale), -127, 127).astype(np.int8)
    quantized.append(q_row.view(np.uint8))

quantized = np.vstack(quantized)
scales = np.array(scales, dtype=np.float32)

# Write binary
print(f"Writing {OUTPUT}...")
with open(OUTPUT, "wb") as f:
    # Header
    f.write(struct.pack("III", vocab_size, TARGET_DIM, 1))  # version=1
    # Scales
    f.write(scales.tobytes())
    # Quantized embeddings (flattened)
    f.write(quantized.tobytes())

size_mb = (quantized.nbytes + scales.nbytes + 12) / 1_000_000
print(f"✓ Done. Size: {size_mb:.2f} MB")
print(f"  Vocab: {vocab_size}, Dim: {TARGET_DIM}")
