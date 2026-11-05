#!/usr/bin/env python3
"""Static Persian embeddings with Rishe.

    rishe hub pull rishe-embed-fa-300
    python examples/embeddings.py

Pass a ``.vec`` path as the first argument (or set ``RISHE_EMBEDDINGS``) to use a
model that is not in the hub cache. Loading never touches the network.
"""

import os
import sys

from rishe import Embeddings, hub

SENTENCES = ("این کتاب خوب است", "کتاب خوبی بود")
WORDS = ("کتاب", "تهران", "دانشگاه", "رفت", "خوب")


def main() -> int:
    # Persian output on Windows consoles defaults to a legacy code page.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    path = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("RISHE_EMBEDDINGS")
    if not path:
        path = hub.path("rishe-embed-fa-300")

    vectors = Embeddings(path)
    print(f"model: {path}")
    print(f"dim={vectors.dim} vocab={len(vectors)}")

    for word in WORDS:
        if not vectors.contains(word):
            print(f"{word}: out of vocabulary")
            continue
        print(f"{word}: {vectors.vector(word)[:4]} ...")

    print("کتاب ~ تهران:", vectors.similarity("کتاب", "تهران"))
    print("کتاب ~ خوب:", vectors.similarity("کتاب", "خوب"))
    print("sentence similarity:", vectors.sentence_similarity(*SENTENCES))
    print("sentences batch:", len(vectors.sentence_vectors(list(SENTENCES))))

    for word, score in vectors.most_similar("کتاب", 5):
        print(f"  {word}\t{score:.3f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
