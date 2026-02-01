#!/usr/bin/env python3
"""
Generate embeddings for all scriptures using BGE-small-en-v1.5 via fastembed.

Usage:
    pip install fastembed numpy
    python scripts/generate_embeddings.py

Output:
    data/scripture_embeddings.npy - NumPy array of embeddings (~65MB)
    data/scripture_metadata.json - Verse title index
"""

import json
import os
import sys
from pathlib import Path

try:
    from fastembed import TextEmbedding
    import numpy as np
except ImportError:
    print("Error: Required packages not installed.")
    print("Run: pip install fastembed numpy")
    sys.exit(1)

# Configuration
MODEL = "BAAI/bge-small-en-v1.5"
DIMENSION = 384
BATCH_SIZE = 256  # fastembed handles batching efficiently
SCRIPTURE_PATH = "lds-scriptures-2020.12.08/json/lds-scriptures-json.txt"
OUTPUT_DIR = "data"


def load_scriptures(path: str) -> list[dict]:
    """Load scriptures from JSON file."""
    with open(path, "r") as f:
        return json.load(f)


def get_verse_text(scripture: dict) -> str:
    """Get text to embed (verse title + text for better semantic matching)."""
    return f"{scripture['verse_title']}: {scripture['scripture_text']}"


def main():
    # Check scripture file exists
    if not os.path.exists(SCRIPTURE_PATH):
        print(f"Error: Scripture file not found: {SCRIPTURE_PATH}")
        print("Make sure you're running from the project root directory")
        sys.exit(1)

    print(f"Loading scriptures from {SCRIPTURE_PATH}...")
    scriptures = load_scriptures(SCRIPTURE_PATH)
    print(f"Loaded {len(scriptures)} verses")

    # Initialize model (downloads on first use to ~/.cache/fastembed/)
    print(f"\nInitializing {MODEL}...")
    print("(First run will download the model, ~33MB)")
    model = TextEmbedding(MODEL)

    # Prepare texts
    texts = [get_verse_text(s) for s in scriptures]
    total = len(texts)

    print(f"\nGenerating embeddings for {total} verses...")
    print(f"Dimensions: {DIMENSION}")
    print()

    # Generate embeddings (fastembed handles batching internally)
    embeddings_list = []
    for i, embedding in enumerate(model.embed(texts, batch_size=BATCH_SIZE)):
        embeddings_list.append(embedding)
        if (i + 1) % 1000 == 0 or i + 1 == total:
            pct = ((i + 1) / total) * 100
            print(f"\r[{'=' * int(pct // 2)}{' ' * (50 - int(pct // 2))}] {pct:.1f}% ({i + 1}/{total})", end="", flush=True)

    print()

    # Convert to numpy array
    embeddings_array = np.array(embeddings_list, dtype=np.float32)
    print(f"\nEmbeddings shape: {embeddings_array.shape}")

    # Create output directory
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # Save embeddings as .npy
    embeddings_path = os.path.join(OUTPUT_DIR, "scripture_embeddings.npy")
    np.save(embeddings_path, embeddings_array)
    embeddings_size = os.path.getsize(embeddings_path) / (1024 * 1024)
    print(f"Saved embeddings to {embeddings_path} ({embeddings_size:.1f} MB)")

    # Save metadata (verse titles for index lookup)
    metadata = [{"verse_title": s["verse_title"]} for s in scriptures]
    metadata_path = os.path.join(OUTPUT_DIR, "scripture_metadata.json")
    with open(metadata_path, "w") as f:
        json.dump(metadata, f)
    metadata_size = os.path.getsize(metadata_path) / (1024 * 1024)
    print(f"Saved metadata to {metadata_path} ({metadata_size:.1f} MB)")

    print(f"\nDone! Total size: {embeddings_size + metadata_size:.1f} MB")


if __name__ == "__main__":
    main()
