#!/usr/bin/env python3
"""Compare Rishe embedding packs with each other and with Hazm.

This is the harness behind the tables in ``docs/MODEL_CARD_0.9.0.md`` and
``docs/MODEL_BENCHMARKS.md``. It uses the 0.9.0 evaluation set: eight
hand-picked Persian pairs plus two random baselines.

- **own vocabulary** (the primary baseline, also stored in ``metrics.json``):
  100 random pairs sampled with seed 42 from the model's full vocabulary.
- **shared vocabulary**: 100 random pairs sampled with seed 42 from the words
  that appear in every model's first 5,000 most frequent entries, so all models
  are scored on the same pairs.

    rishe hub pull rishe-embed-fa-300
    python scripts/compare_embeddings.py

    # explicit files, no Hazm
    python scripts/compare_embeddings.py \
        --model "rishe-30k=path/to/rishe-embed-fa-300.vec" \
        --model "rishe-135k=path/to/rishe-embed-fa-300.vec" --no-hazm

These are intrinsic sanity checks, not a gold Persian WordSim/SimLex score.
"""

from __future__ import annotations

import argparse
import gc
import random
import statistics
import time
from pathlib import Path
from typing import Dict, List, Tuple

from rishe import Embeddings, hub

PAIRS: List[Tuple[str, str, str]] = [
    ("book_notebook", "کتاب", "دفتر"),
    ("book_read", "کتاب", "خواندن"),
    ("uni_student", "دانشگاه", "دانشجو"),
    ("tehran_iran", "تهران", "ایران"),
    ("water_drink", "آب", "نوشیدنی"),
    ("good_bad", "خوب", "بد"),
    ("went_came", "رفت", "آمد"),
    ("woman_man", "زن", "مرد"),
]

SEED = 42
RANDOM_SAMPLES = 100
COMMON_WORDS = 5_000
REPO = Path(__file__).resolve().parents[1]
DEFAULT_PACKS = [
    ("rishe-30k", REPO / "training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/rishe-embed-fa-300.vec"),
    ("rishe-135k", REPO / "training/result/rishe-0.9.0/artifacts-135k/rishe-embed-fa-300/rishe-embed-fa-300.vec"),
]


def read_words(path: Path) -> List[str]:
    """Vocabulary in file order (frequency order for gensim exports)."""
    words: List[str] = []
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        handle.readline()
        for line in handle:
            word = line.split(" ", 1)[0]
            if word:
                words.append(word)
    return words


def sample_scores(pack, words: List[str]) -> List[float]:
    random.seed(SEED)
    scores: List[float] = []
    for _ in range(RANDOM_SAMPLES):
        left, right = random.sample(words, 2)
        if left == right:
            continue
        if pack.contains(left) and pack.contains(right):
            scores.append(pack.similarity(left, right))
    return scores


def measure(name: str, path: Path, shared_words: List[str]) -> dict:
    started = time.perf_counter()
    pack = Embeddings(path)
    load_seconds = time.perf_counter() - started

    positive = [
        pack.similarity(left, right)
        for _label, left, right in PAIRS
        if pack.contains(left) and pack.contains(right)
    ]
    own_random = sample_scores(pack, read_words(path))
    shared_random = sample_scores(pack, shared_words)

    started = time.perf_counter()
    for _ in range(100):
        pack.most_similar("کتاب", 5)
    most_similar_ms = (time.perf_counter() - started) * 1000 / 100

    sentence = "این کتاب خوب است"
    started = time.perf_counter()
    for _ in range(1000):
        pack.sentence_vector(sentence)
    sentence_vector_us = (time.perf_counter() - started) * 1_000_000 / 1000

    positive_mean = statistics.mean(positive)
    row = {
        "name": name,
        "path": path,
        "size": path.stat().st_size,
        "vocab": len(pack),
        "load": load_seconds,
        "positive_mean": positive_mean,
        "positive_median": statistics.median(positive),
        "own_random": statistics.mean(own_random),
        "shared_random": statistics.mean(shared_random),
        "own_gap": positive_mean - statistics.mean(own_random),
        "shared_gap": positive_mean - statistics.mean(shared_random),
        "most_similar_ms": most_similar_ms,
        "sentence_vector_us": sentence_vector_us,
        "pairs": {
            label: (
                pack.similarity(left, right)
                if pack.contains(left) and pack.contains(right)
                else None
            )
            for label, left, right in PAIRS
        },
    }
    del pack
    gc.collect()
    return row


def megabytes(size: int) -> str:
    if size >= 1_000_000_000:
        return f"{size / 1e9:.2f} GB"
    return f"{size / 1e6:.0f} MB"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--model",
        action="append",
        metavar="LABEL=PATH",
        help="embedding file to compare; repeatable",
    )
    parser.add_argument(
        "--hazm",
        default=None,
        help="path to hazm-fasttext-300.vec (default: hub cache)",
    )
    parser.add_argument("--no-hazm", action="store_true", help="skip the Hazm row")
    args = parser.parse_args()

    models: List[Tuple[str, Path]] = []
    if args.model:
        for item in args.model:
            label, _, raw = item.partition("=")
            if not raw:
                parser.error(f"--model expects LABEL=PATH, got {item!r}")
            models.append((label, Path(raw)))
    else:
        models = [(label, path) for label, path in DEFAULT_PACKS if path.is_file()]

    if not args.no_hazm:
        hazm_path = Path(args.hazm) if args.hazm else hub.cached("hazm-fasttext-300")
        if hazm_path is None or not Path(hazm_path).is_file():
            print("Hazm file is not cached; run `rishe hub pull hazm-fasttext-300`")
        else:
            models.append(("hazm-fasttext-300", Path(hazm_path)))

    models = [(label, path) for label, path in models if path.is_file()]
    if not models:
        parser.error("no embedding files found; pass --model LABEL=PATH")

    # Shared baseline: words present in every model's first 5,000 entries.
    heads: Dict[str, List[str]] = {
        label: read_words(path)[:COMMON_WORDS] for label, path in models
    }
    head_sets = {label: set(words) for label, words in heads.items()}
    shared_words = [
        word
        for word in heads[models[0][0]]
        if all(word in head_sets[label] for label, _ in models)
    ]
    print(f"shared baseline vocabulary: {len(shared_words)} words", flush=True)

    rows = []
    for label, path in models:
        print(f"measuring {label} ({path})", flush=True)
        rows.append(measure(label, path, shared_words))

    print("\n| model | size | vocab | load s | positive mean | random mean | gap | most_similar |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|")
    for row in rows:
        print(
            f"| {row['name']} | {megabytes(row['size'])} | {row['vocab']:,} "
            f"| {row['load']:.2f} | {row['positive_mean']:.4f} "
            f"| {row['own_random']:.4f} | {row['own_gap']:.4f} "
            f"| {row['most_similar_ms']:.1f} ms |"
        )

    print("\n| pair | " + " | ".join(row["name"] for row in rows) + " |")
    print("|---|" + "---:|" * len(rows))
    for label, _left, _right in PAIRS:
        cells = []
        for row in rows:
            score = row["pairs"][label]
            cells.append("n/a" if score is None else f"{score:.4f}")
        print(f"| {label} | " + " | ".join(cells) + " |")

    print(f"\nShared-vocabulary baseline ({len(shared_words)} words):")
    print("\n| model | shared random mean | shared gap |")
    print("|---|---:|---:|")
    for row in rows:
        print(
            f"| {row['name']} | {row['shared_random']:.4f} | {row['shared_gap']:.4f} |"
        )

    print(
        "\nsentence_vector: "
        + ", ".join(
            f"{row['name']} {row['sentence_vector_us']:.2f} µs" for row in rows
        )
    )
    print("Random baselines use seed 42 and 100 pairs; the primary table samples")
    print("each model's own vocabulary, the shared table the common word list.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
