#!/usr/bin/env python3
"""
Rishe vs Hazm/gensim: static word and sentence embeddings.

    python -m rishe.cli hub pull hazm-fasttext-300
    pip install gensim
    python scripts/benchmark_embeddings.py

Both libraries load the same .vec file, so this isolates parsing, lookup and
pooling speed. Hazm has no sentence-vector helper; the baseline here is the
mean of gensim word vectors over the same tokens (average pooling, not a
trained sentence encoder).
"""
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8")

from benchmark import load_ud, timeit
from rishe import Embeddings, hub

WORDS = [
    "ایران",
    "کتاب",
    "خانه",
    "مدرسه",
    "دوست",
    "شهر",
    "آب",
    "زبان",
    "دانش",
    "کار",
    "روز",
    "شب",
    "مرد",
    "زن",
    "کودک",
    "بزرگ",
    "خوب",
    "بد",
    "زیبا",
    "سریع",
]


def cosine(first, second):
    first = np.asarray(first, dtype=np.float32)
    second = np.asarray(second, dtype=np.float32)
    denominator = float(np.linalg.norm(first) * np.linalg.norm(second))
    if denominator == 0.0:
        return 0.0
    return float(np.dot(first, second)) / denominator


def main():
    try:
        model_path = hub.path("hazm-fasttext-300")
    except FileNotFoundError as error:
        print(error)
        return 1

    print("# Static embeddings: Rishe vs Hazm/gensim\n")
    print(f"Model: `{model_path.name}` ({model_path.stat().st_size / 1e9:.2f} GB)\n")

    started = time.perf_counter()
    embeddings = Embeddings(str(model_path))
    rishe_load = time.perf_counter() - started

    try:
        from hazm import WordEmbedding, WordTokenizer as HazmWordTokenizer

        started = time.perf_counter()
        hazm = WordEmbedding.load(model_path=str(model_path), model_type="keyedvector")
        hazm_load = time.perf_counter() - started
    except Exception as error:
        print(f"Hazm/gensim is unavailable: {error}")
        print("Install it with: pip install gensim")
        return 1

    hazm_tokenize = HazmWordTokenizer().tokenize
    hazm_vectors = hazm.model

    print("## Load\n")
    print("| library | vectors | dim | load (s) |")
    print("|---|---|---|---|")
    print(f"| Rishe | {len(embeddings)} | {embeddings.dim} | {rishe_load:.2f} |")
    print(
        f"| Hazm/gensim | {len(hazm_vectors)} | {hazm_vectors.vector_size} | {hazm_load:.2f} |"
    )

    print("\n## Word vectors (same file, same terms)\n")
    print("| metric | value |")
    print("|---|---|")
    shared = [word for word in WORDS if word in hazm_vectors and embeddings.contains(word)]
    cosines = [cosine(embeddings.vector(word), hazm_vectors[word]) for word in shared]
    differences = [
        float(np.max(np.abs(np.asarray(embeddings.vector(word)) - hazm_vectors[word])))
        for word in shared
    ]
    print(f"| terms checked | {len(shared)}/{len(WORDS)} |")
    print(f"| mean cosine vs Hazm | {np.mean(cosines):.6f} |")
    print(f"| min cosine vs Hazm | {np.min(cosines):.6f} |")
    print(f"| max absolute component difference | {np.max(differences):.2e} |")
    for pair in (("ایران", "آلمان"), ("کتاب", "دفتر"), ("خوب", "بد"), ("مرد", "زن")):
        first, second = pair
        ours = embeddings.similarity(first, second)
        theirs = (
            cosine(hazm_vectors[first], hazm_vectors[second])
            if first in hazm_vectors and second in hazm_vectors
            else None
        )
        ours_text = "n/a" if ours is None else f"{ours:.4f}"
        theirs_text = "n/a" if theirs is None else f"{theirs:.4f}"
        print(f"| similarity {first} / {second} | Rishe {ours_text}, Hazm {theirs_text} |")

    print("\n## Sentence embeddings (UD-Seraji test sentences)\n")
    sentences = [text for text, _, _ in load_ud("fa_seraji-ud-test.conllu")][:500]

    def hazm_sentence_vector(text):
        vectors = [hazm_vectors[word] for word in hazm_tokenize(text) if word in hazm_vectors]
        if not vectors:
            return np.zeros(int(hazm_vectors.vector_size), dtype=np.float32)
        return np.mean(vectors, axis=0)

    rishe_ms = timeit(embeddings.sentence_vector, sentences, reps=1)
    hazm_ms = timeit(hazm_sentence_vector, sentences, reps=1)
    agreement = [
        cosine(embeddings.sentence_vector(text), hazm_sentence_vector(text))
        for text in sentences
    ]

    print("| library | ms/sentence | sentences/s |")
    print("|---|---|---|")
    print(f"| Rishe | {rishe_ms:.4f} | {1000 / rishe_ms:.0f} |")
    print(f"| Hazm + numpy mean | {hazm_ms:.4f} | {1000 / hazm_ms:.0f} |")
    print(f"\nMean cosine between the two sentence vectors: **{np.mean(agreement):.6f}**")
    print(f"Minimum: {np.min(agreement):.6f}")

    print("\n## Lookup speed (word -> Python list of floats)\n")
    known = [word for word in WORDS if word in hazm_vectors]
    rishe_lookup = timeit(embeddings.vector, WORDS, reps=20)
    hazm_lookup = timeit(lambda word: hazm_vectors[word].tolist(), known, reps=20)
    print("| library | µs/word |")
    print("|---|---|")
    print(f"| Rishe | {rishe_lookup * 1000:.3f} |")
    print(f"| Hazm/gensim | {hazm_lookup * 1000:.3f} |")

    print("\n## Nearest neighbours (top-10 overlap with Hazm)\n")
    overlaps = []
    for word in WORDS:
        if word not in hazm_vectors:
            continue
        ours = [candidate for candidate, _ in embeddings.most_similar(word, 10)]
        theirs = [candidate for candidate, _ in hazm_vectors.most_similar(word, topn=10)]
        overlaps.append(len(set(ours) & set(theirs)) / 10)
    print("| words | mean top-10 overlap | perfect lists |")
    print("|---|---|---|")
    print(
        f"| {len(overlaps)} | {100 * np.mean(overlaps):.0f}% "
        f"| {sum(overlap == 1.0 for overlap in overlaps)}/{len(overlaps)} |"
    )

    print("\nCaveat: sentence vectors use different tokenizers, so agreement below")
    print("1.0 is mostly tokenization, not embedding arithmetic.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
