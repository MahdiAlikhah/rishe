#!/usr/bin/env python3
"""Evaluate Rishe's sentence vectors on FarsTail, a license-clean Persian NLI set.

    python scripts/benchmark_nli.py

Data: FarsTail by the Data Mining Lab, University of Qom, **Apache-2.0**
(https://github.com/dml-qom/FarsTail), 10,367 premise/hypothesis pairs labelled
entailment / contradiction / neutral. Apache-2.0 is why this dataset is used
instead of ParsiNLU (CC BY-NC-SA: evaluation only, so it cannot back a published
number).

What this measures, and what it does not. The shipped embedding pack is a static
word-vector model and `sentence_vector()` is a mean of those vectors. That is not
a trained sentence encoder, so the number below is a **floor for the current
engine**, not a claim about sentence embeddings in general. The comparison to
FarsTail's published baselines (DecompAtt 0.6662, HBMP 0.6604, ESIM 0.7116,
mBERT 0.8338) exists to show where the mean-pooled representation sits.

The hypothesis-only baseline is the bias control. On NLI a model that reads only
the hypothesis can score well by learning annotation artefacts; a pair model is
only informative if it clearly beats that. Both numbers are printed.

Everything is deterministic: frozen features, closed-form ridge, no sampling, no
random seed.
"""
import csv
import os
import sys
import urllib.request
from pathlib import Path

import numpy as np

sys.stdout.reconfigure(encoding="utf-8")
ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "corpora" / "farstail"
BASE = "https://raw.githubusercontent.com/dml-qom/FarsTail/master/data"
SPLITS = ("Train-word.csv", "Val-word.csv", "Test-word.csv")
LABELS = {"e": 0, "c": 1, "n": 2}
NAMES = ("entailment", "contradiction", "neutral")


def fetch(name):
    path = DATA / name
    if not path.exists():
        DATA.mkdir(parents=True, exist_ok=True)
        req = urllib.request.Request(f"{BASE}/{name}",
                                     headers={"User-Agent": "rishe-benchmark"})
        with urllib.request.urlopen(req, timeout=120) as response:
            path.write_bytes(response.read())
    return path


def load(split):
    text = fetch(f"{split}-word.csv").read_text(encoding="utf-8")
    return list(csv.DictReader(text.splitlines(), delimiter="\t"))


def vector(model, text):
    return np.asarray(model.sentence_vector(text), dtype=np.float32)


def cosine(first, second):
    denominator = float(np.linalg.norm(first) * np.linalg.norm(second))
    return 0.0 if denominator == 0.0 else float(np.dot(first, second)) / denominator


def pair_features(model, premise, hypothesis):
    p, h = vector(model, premise), vector(model, hypothesis)
    difference = p - h
    return np.concatenate([
        [cosine(p, h), float(np.linalg.norm(difference)),
         float(np.linalg.norm(p)), float(np.linalg.norm(h))],
        np.abs(difference), p * h,
    ]).astype(np.float32)


def hypothesis_features(model, premise, hypothesis):
    """Ignores the premise on purpose: the biased baseline."""
    h = vector(model, hypothesis)
    return np.concatenate([[float(np.linalg.norm(h))], h]).astype(np.float32)


def ridge(X, y, penalty=1.0):
    mean = X.mean(axis=0)
    std = X.std(axis=0) + 1e-6
    Z = np.hstack([(X - mean) / std, np.ones((len(X), 1), dtype=np.float32)])
    targets = np.eye(3, dtype=np.float32)[y]
    weights = np.linalg.solve(
        Z.T @ Z + penalty * np.eye(Z.shape[1], dtype=np.float32), Z.T @ targets)
    return weights, mean, std


def predict(X, params):
    weights, mean, std = params
    Z = np.hstack([(X - mean) / std, np.ones((len(X), 1), dtype=np.float32)])
    return (Z @ weights).argmax(axis=1)


def accuracy(rows, model, featurizer, params):
    X = np.stack([featurizer(model, r["premise"], r["hypothesis"]) for r in rows])
    y = np.array([LABELS[r["label"]] for r in rows])
    return predict(X, params), y


def ranked(model, rows):
    """Share of premises where entailment scores above contradiction.

    Uses no training at all: within one premise the hypotheses differ only in
    relation, so a representation that separates relations should order them.
    """
    groups = {}
    for r in rows:
        groups.setdefault(r["premise"], []).append(r)
    considered, correct = 0, 0
    for premise, group in groups.items():
        pv = vector(model, premise)
        if not pv.any():
            continue
        scored = [(cosine(pv, vector(model, r["hypothesis"])), r["label"])
                  for r in group]
        entail = [s for s, label in scored if label == "e"]
        contradict = [s for s, label in scored if label == "c"]
        if entail and contradict:
            considered += 1
            correct += max(entail) > max(contradict)
    return correct, considered


def main():
    from rishe import Embeddings, hub

    try:
        model = Embeddings(hub.path("rishe-embed-fa-300-v2"))
    except (FileNotFoundError, RuntimeError) as error:
        print(f"{error}\n\nPull it first: python -m rishe.cli hub pull "
              "rishe-embed-fa-300-v2")
        return 1

    print(f"pack: rishe-embed-fa-300-v2  dim={model.dim}  dtype={model.dtype}")
    print("data: FarsTail (Apache-2.0) https://github.com/dml-qom/FarsTail\n")

    train, val, test = load("Train"), load("Val"), load("Test")
    print(f"rows: train {len(train)}  val {len(val)}  test {len(test)}")

    pair = ridge(np.stack([pair_features(model, r["premise"], r["hypothesis"])
                          for r in train]),
                 np.array([LABELS[r["label"]] for r in train]))
    only = ridge(np.stack([hypothesis_features(model, r["premise"], r["hypothesis"])
                           for r in train]),
                 np.array([LABELS[r["label"]] for r in train]))

    pair_pred, test_y = accuracy(test, model, pair_features, pair)
    only_pred, _ = accuracy(test, model, hypothesis_features, only)
    val_pair, val_y = accuracy(val, model, pair_features, pair)
    val_only, _ = accuracy(val, model, hypothesis_features, only)

    pair_acc = float((pair_pred == test_y).mean())
    only_acc = float((only_pred == test_y).mean())
    val_pair_acc = float((val_pair == val_y).mean())
    val_only_acc = float((val_only == val_y).mean())

    print()
    print("| model | val accuracy | test accuracy |")
    print("|---|---|---|")
    print(f"| Rishe sentence vectors (pair) | {val_pair_acc:.4f} | {pair_acc:.4f} |")
    print(f"| Rishe sentence vectors (hypothesis only) | "
          f"{val_only_acc:.4f} | {only_acc:.4f} |")
    print(f"\nPair model beats the biased baseline by "
          f"{(pair_acc - only_acc) * 100:+.2f} points on test.")

    print("\nFarsTail published baselines (test accuracy, from the dataset paper):")
    print("  0.6662 DecompAtt (word2vec) | 0.6604 HBMP (word2vec)")
    print("  0.7116 ESIM (fastText)      | 0.8338 mBERT")

    correct, considered = ranked(model, test)
    if considered:
        print(f"\nTraining-free ranking: entailment above contradiction on "
              f"{correct}/{considered} premises = {correct / considered:.1%}")

    print("\nError breakdown on test (pair model):")
    for index, name in enumerate(NAMES):
        total = int((test_y == index).sum())
        hit = int(((pair_pred == index) & (test_y == index)).sum())
        if total:
            print(f"  {name:14} {hit}/{total} = {hit / total:.1%}")

    for flag, label in (("hard(hypothesis)", "hypothesis-only hard"),
                        ("hard(overlap)", "overlap-based hard")):
        rows = [i for i, r in enumerate(test) if r.get(flag, "").strip() == "1"]
        if rows:
            acc = float((pair_pred[rows] == test_y[rows]).mean())
            print(f"  {label:22} n={len(rows):4} acc={acc:.4f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
