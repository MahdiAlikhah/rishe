#!/usr/bin/env python3
"""Evaluate Rishe's sentence vectors on MIRACL fa dev, a license-clean retrieval set.

    python scripts/benchmark_retrieval.py                 # needs a vector cache
    python scripts/benchmark_retrieval.py --encode        # build the cache (hours)
    python scripts/benchmark_retrieval.py --limit 5000    # laptop smoke run

Data: MIRACL Persian dev split, **Apache-2.0** and ungated: 632 queries, 1,314
relevance judgements, 2,207,172 passages. This is the task the sentence-encoder
pack exists for. The NLI number in `benchmark_nli.py` measures a different
question and cannot justify a retriever.

Both systems are scored through the **installed package**, not a
reimplementation: `ModelPack(hub.path(...)).sentence_vectors()` for the encoder
and `Embeddings(hub.path(...)).sentence_vectors()` for the static pack.

**Why there is a cache.** Encoding 2.2M passages costs ~36 h single-threaded on
CPU (58 ms/passage measured on the release build), so a script that re-encodes
on every run would never be run twice. `--encode` writes an `.npy` memmap once;
every scoring run reads it back in seconds. Only the *passages* are cached, and
only for the configured `--limit`: a cache is therefore tied to that limit, and
the script refuses a mismatched one rather than silently scoring a short corpus.

`shortcut: one flat .npy and a linear scan, so memory is 2.2M * dim * 4 bytes
(~3.4 GB for 384-d fp32) and the scan is exact but not indexed. Upgrade to
faiss/ann if the corpus grows or the box cannot hold the memmap.`
"""
import argparse
import csv
import gzip
import io
import json
import os
import sys
import urllib.request
from pathlib import Path

import numpy as np

sys.stdout.reconfigure(encoding="utf-8")
ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "corpora" / "miracl"
TOPICS = "https://huggingface.co/datasets/miracl/miracl/resolve/main/miracl-v1.0-fa/topics/topics.miracl-v1.0-fa-dev.tsv"
QRELS = "https://huggingface.co/datasets/miracl/miracl/resolve/main/miracl-v1.0-fa/qrels/qrels.miracl-v1.0-fa-dev.tsv"
CORPUS = "https://huggingface.co/datasets/miracl/miracl-corpus/resolve/main/miracl-corpus-v1.0-fa/docs-{shard}.jsonl.gz"
SHARDS = 5
PASSAGES = 2_207_172
# e5 was trained with these prefixes and its model card documents them; the gap
# is not cosmetic (0.4485 -> 0.4918 nDCG@10 on this set).
QUERY_PREFIX = "query: "
PASSAGE_PREFIX = "passage: "


def fetch(url, name):
    path = DATA / name
    if not path.exists():
        DATA.mkdir(parents=True, exist_ok=True)
        req = urllib.request.Request(url, headers={"User-Agent": "rishe-benchmark"})
        with urllib.request.urlopen(req, timeout=300) as response:
            path.write_bytes(response.read())
    return path


def topics():
    text = fetch(TOPICS, "topics.miracl-v1.0-fa-dev.tsv").read_text(encoding="utf-8")
    return [(r[0], r[1]) for r in csv.reader(io.StringIO(text), delimiter="\t") if r]


def qrels():
    """`{qid: {docid: grade}}`, grade 1 means relevant."""
    text = fetch(QRELS, "qrels.miracl-v1.0-fa-dev.tsv").read_text(encoding="utf-8")
    judged = {}
    for row in csv.reader(io.StringIO(text), delimiter="\t"):
        if len(row) >= 4:
            judged.setdefault(row[0], {})[row[2]] = int(row[3])
    return judged


def passages(limit):
    """Yield `(docid, text)` in corpus order, stopping at `limit`."""
    seen = 0
    for shard in range(SHARDS):
        path = fetch(CORPUS.format(shard=shard), f"docs-{shard}.jsonl.gz")
        with gzip.open(path, "rt", encoding="utf-8") as handle:
            for line in handle:
                if limit and seen >= limit:
                    return
                record = json.loads(line)
                title = (record.get("title") or "").strip()
                body = (record.get("text") or "").strip()
                yield record["docid"], f"{title} {body}".strip()
                seen += 1


def encode(encoder, static, limit, cache, static_cache):
    """Write one row per passage, in corpus order, for each system given.

    Both matrices are filled in a **single** pass over the corpus: reading the
    257 MiB of gzipped shards twice would double the wall time of the only
    expensive step here.
    """
    rows = limit or PASSAGES
    # `ModelPack` deliberately exposes only `task` and `max_len`, so the vector
    # size comes from one probe rather than a `dim` property it does not have.
    encoder_matrix = np.lib.format.open_memmap(
        cache, mode="w+", dtype=np.float32,
        shape=(rows, len(encoder.sentence_vector(PASSAGE_PREFIX + "a"))))
    static_matrix = None
    if static is not None:
        static_matrix = np.lib.format.open_memmap(
            static_cache, mode="w+", dtype=np.float32,
            shape=(rows, static.dim))
    docids = []
    for index, (docid, text) in enumerate(passages(limit)):
        encoder_matrix[index] = encoder.sentence_vector(PASSAGE_PREFIX + text)
        if static_matrix is not None:
            static_matrix[index] = static.sentence_vector(PASSAGE_PREFIX + text)
        docids.append(docid)
        if index and index % 2000 == 0:
            print(f"  encoded {index}/{rows}", flush=True)
    if len(docids) != rows:
        raise SystemExit(
            f"the corpus yielded {len(docids)} passages but {rows} were expected; "
            f"the citation says {PASSAGES}. Refusing to write a short cache."
        )
    for matrix, path in ((encoder_matrix, cache),
                         (static_matrix, static_cache)):
        if matrix is not None:
            matrix.flush()
            path.with_suffix(".docids.json").write_text(
                json.dumps(docids), encoding="utf-8")


def load_cache(model, limit, cache):
    rows = limit or PASSAGES
    if not cache.exists() or not cache.with_suffix(".docids.json").exists():
        raise SystemExit(
            f"no vector cache at {cache}\n\n"
            f"Encoding {rows} passages takes hours; build it once with:\n"
            f"  python scripts/benchmark_retrieval.py --encode"
            + (f" --limit {limit}" if limit else "")
        )
    matrix = np.load(cache, mmap_mode="r")
    docids = json.loads(cache.with_suffix(".docids.json").read_text(encoding="utf-8"))
    if matrix.shape[0] != rows or len(docids) != rows:
        raise SystemExit(
            f"cache {cache} holds {matrix.shape[0]} rows, expected {rows}. "
            f"It was built for a different --limit; rebuild it."
        )
    return np.asarray(matrix), docids


def ndcg(ranked, judged, k):
    """nDCG@k over graded relevance, binary gain (MIRACL's grades are 0/1)."""
    gains = [judged.get(docid, 0) for docid in ranked[:k]]
    ideal = sorted(judged.values(), reverse=True)[:k]
    discount = 1.0 / np.log2(np.arange(2, len(gains) + 2))
    dcg = float(np.sum(np.asarray(gains) * discount))
    idcg = float(np.sum(np.asarray(ideal) * discount[:len(ideal)]))
    return 0.0 if idcg == 0.0 else dcg / idcg


def recall(ranked, judged, k):
    relevant = {d for d, grade in judged.items() if grade > 0}
    if not relevant:
        return None
    return len(relevant & set(ranked[:k])) / len(relevant)


def selftest():
    """Hand-computed values, because a metric that only returns 0.0 looks fine."""
    # Perfect ranking of one relevant document: nDCG is 1.0.
    assert ndcg(["a"], {"a": 1}, 10) == 1.0
    # Relevant document at rank 2: DCG = 1/log2(3), IDCG = 1/log2(2) = 1.
    assert abs(ndcg(["x", "a"], {"a": 1}, 10) - 1 / np.log2(3)) < 1e-12
    # Two relevant, both found, best order: DCG == IDCG so nDCG == 1.
    assert abs(ndcg(["a", "b"], {"a": 1, "b": 1}, 10) - 1.0) < 1e-12
    # An irrelevant hit between two relevant ones costs: DCG = 1 + 1/log2(4)
    # against the ideal 1 + 1/log2(3), so 1.5 / 1.6309... = 0.9197...
    assert abs(ndcg(["a", "z", "b"], {"a": 1, "b": 1}, 10) - 0.9197207891481876) < 1e-12
    # No relevant documents judged at all: defined as 0 rather than a crash.
    assert ndcg(["a"], {}, 10) == 0.0
    # Recall: 1 of 2 relevant in the top 10, and k is respected.
    assert recall(["a", "x"], {"a": 1, "b": 1}, 10) == 0.5
    assert recall(["x", "a"], {"a": 1, "b": 1}, 1) == 0.0
    # A query with no judgements has no recall to speak of (None, not 0.0),
    # so it does not drag the mean towards zero.
    assert recall(["a"], {}, 10) is None
    # Grades are thresholded at >0: a 0 score is not relevant.
    assert recall(["a"], {"a": 0, "b": 1}, 10) == 0.0
    print("benchmark_retrieval: selftest ok")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pack", default="rishe-vector-e5-small-fa")
    parser.add_argument("--static", default="rishe-embed-fa-300-v2",
                        help="static pack for the comparison row; 'none' to skip")
    parser.add_argument("--encode", action="store_true",
                        help="build the passage cache instead of scoring")
    parser.add_argument("--limit", type=int, default=0,
                        help="passages to encode; 0 means the whole corpus")
    parser.add_argument("--cache", default=str(DATA / "passages-e5-small.npy"))
    parser.add_argument("--threads", type=int, default=0)
    parser.add_argument("--selftest", action="store_true",
                        help="check the metrics against hand-computed values")
    args = parser.parse_args()

    if args.selftest:
        selftest()
        return 0

    from rishe import Embeddings, ModelPack, hub

    if args.threads:
        os.environ["RAYON_NUM_THREADS"] = str(args.threads)

    try:
        # A directory is used as given so a locally built pack needs no Hub
        # entry; anything else is a Hub name.
        source = Path(args.pack)
        encoder = ModelPack(source if source.is_dir() else hub.path(args.pack))
    except (FileNotFoundError, RuntimeError, ValueError) as error:
        print(f"{error}\n\nPull it first: python -m rishe.cli hub pull {args.pack}")
        return 1

    static = None
    if args.static.lower() != "none":
        try:
            static_source = Path(args.static)
            static = Embeddings(static_source if static_source.exists()
                                else hub.path(args.static))
        except (FileNotFoundError, RuntimeError, ValueError) as error:
            print(f"(skipping the static row: {error})")

    cache = Path(args.cache)
    static_cache = cache.with_name(cache.stem + "-static.npy")
    if args.encode:
        print(f"encoder: {args.pack}")
        if static is not None:
            print(f"static:  {args.static} (same pass, its own cache)")
        print(f"encoding {(args.limit or PASSAGES):,} passages -> {cache}")
        encode(encoder, static, args.limit, cache, static_cache)
        print("done")
        return 0

    matrix, docids = load_cache(encoder, args.limit, cache)
    print(f"pack: {args.pack}  dim={matrix.shape[1]}")
    print(f"cache: {cache}  passages={len(docids):,}"
          + (f"  (limited to {args.limit:,})" if args.limit else ""))
    print("data: MIRACL fa dev (Apache-2.0) https://project-miracl.github.io/\n")

    queries = topics()
    judged = qrels()
    print(f"queries: {len(queries)}  judgements: "
          f"{sum(len(v) for v in judged.values()):,}  "
          f"relevant: {sum(1 for v in judged.values() for g in v.values() if g):,}")

    # A gold document that is not in the cache can never be retrieved, and IDCG
    # still counts it, so a partial corpus scores low for a reason that has
    # nothing to do with the model. The published numbers were taken with every
    # gold id present; check that rather than trusting it.
    present = set(docids)
    missing = {docid for rels in judged.values() for docid in rels
               if docid not in present}
    if missing:
        print(f"\nWARNING: {len(missing)} of "
              f"{len({d for r in judged.values() for d in r})} judged documents "
              f"are not in this cache, so scores below are floor-limited.\n"
              f"Re-run without --limit for a number comparable to the docs.\n")

    # Queries are encoded once and reused by every system in the table.
    query_matrix = np.stack(
        [encoder.sentence_vector(QUERY_PREFIX + title) for _, title in queries]
    ).astype(np.float32)
    scores = query_matrix @ matrix.T

    systems = [("encoder", args.pack, scores)]
    if static is not None:
        try:
            static_matrix, static_docids = load_cache(
                static, args.limit, static_cache)
            if static_docids != docids:
                raise SystemExit(
                    f"{static_cache} was built for a different corpus; re-encode.")
            static_queries = np.stack(
                [static.sentence_vector(title) for _, title in queries]
            ).astype(np.float32)
            systems.append(("static", args.static,
                            static_queries @ static_matrix.T))
        except (FileNotFoundError, RuntimeError) as error:
            print(f"(skipping the static row: {error})")

    print()
    print("| system | nDCG@10 | R@10 | R@100 |")
    print("|---|---|---|---|")
    for name, label, system_scores in systems:
        ndcgs, r10, r100 = [], [], []
        for row, (qid, _) in zip(system_scores, queries):
            order = np.argsort(-row)[:100]
            ranked = [docids[i] for i in order]
            relevant = judged.get(qid, {})
            ndcgs.append(ndcg(ranked, relevant, 10))
            r10.append(recall(ranked, relevant, 10))
            r100.append(recall(ranked, relevant, 100))
        print(f"| `{label}` | {np.mean(ndcgs):.4f} | "
              f"{np.mean([v for v in r10 if v is not None] or [0]):.4f} | "
              f"{np.mean([v for v in r100 if v is not None] or [0]):.4f} |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
