#!/usr/bin/env python3
"""End-to-end latency for the Rishe task packs.

    RISHE_PACK_ROOT=/path/to/compact python scripts/benchmark_modelpack.py

Without RISHE_PACK_ROOT, the packs are read from the Rishe Hub cache.
"""
import argparse
import os
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8")

from benchmark_models import load_peyma_ner, load_ud_pos
from rishe import ModelPack, hub


def pack_path(root, task):
    if root:
        return Path(root) / task
    return Path(hub.path_pack(f"{task}-compact"))


def sentences_for(task, limit):
    if task == "ner":
        return [
            [form for form, _ in tokens]
            for tokens in load_peyma_ner()[:limit]
        ]
    return [
        [form for form, _ in gold]
        for _, gold in load_ud_pos("fa_perdt-ud-test.conllu")[:limit]
    ]


def measure(pack, sentences):
    for words in sentences[:3]:
        pack.tag_words(words)
    times = []
    words = 0
    started = time.perf_counter()
    for sentence in sentences:
        call = time.perf_counter()
        pack.tag_words(sentence)
        times.append((time.perf_counter() - call) * 1000)
        words += len(sentence)
    elapsed = time.perf_counter() - started
    ordered = sorted(times)
    return {
        "median": statistics.median(times),
        "p95": ordered[int(0.95 * (len(ordered) - 1))],
        "words_per_second": words / elapsed if elapsed else 0.0,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pack-root", default=os.environ.get("RISHE_PACK_ROOT"))
    parser.add_argument("--limit", type=int, default=200)
    args = parser.parse_args()

    print("| task | sentences | words | load s | median ms | p95 ms | words/s |")
    print("|---|---|---|---|---|---|---|")
    for task in ("pos", "ner", "ezafe"):
        path = pack_path(args.pack_root, task)
        load_started = time.perf_counter()
        pack = ModelPack(str(path))
        load_seconds = time.perf_counter() - load_started
        sentences = sentences_for(task, args.limit)
        result = measure(pack, sentences)
        words = sum(len(sentence) for sentence in sentences)
        print(
            f"| {task.upper()} | {len(sentences)} | {words} | {load_seconds:.2f} "
            f"| {result['median']:.2f} | {result['p95']:.2f} "
            f"| {result['words_per_second']:.0f} |"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
