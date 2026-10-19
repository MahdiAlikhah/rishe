#!/usr/bin/env python3
"""
Rishe vs Hazm vs Shekar: tokenization, lemmatization and speed.

    pip install rishe hazm shekar conllu
    python scripts/benchmark.py

Fairness rules:
  - every library is built once and reused (hazm.word_tokenize() rebuilds
    WordTokenizer on every call, ~130 ms, which is not tokenization cost);
  - full test sets, no subsampling;
  - tokens are scored as per-sentence multisets against *surface* tokens
    (UD multiword tokens kept whole), micro-averaged over the corpus;
  - lemmas are scored on gold word tokens in isolation, next to an
    "identity" baseline (predict the token itself).
"""
import statistics
import sys
import time
import urllib.request
from collections import Counter
from pathlib import Path

from conllu import parse
from hazm import Lemmatizer as HazmLemmatizer, Normalizer as HazmNormalizer, WordTokenizer as HazmWT
from shekar import Lemmatizer as ShekarLemmatizer, Normalizer as ShekarNormalizer, WordTokenizer as ShekarWT
from rishe import Pipeline

sys.stdout.reconfigure(encoding="utf-8")
CORPORA = Path(__file__).resolve().parents[1] / "data" / "corpora"
UD = "https://raw.githubusercontent.com/UniversalDependencies/UD_Persian-{0}/master/fa_{1}-ud-test.conllu"
URLS = {
    "fa_seraji-ud-test.conllu": UD.format("Seraji", "seraji"),
    "fa_perdt-ud-test.conllu": UD.format("PerDT", "perdt"),
    "peyma_test.txt": "https://huggingface.co/datasets/ParsiAI/PEYMA/resolve/main/data/test.txt",
}


def read(name):
    path = CORPORA / name
    if not path.exists():
        CORPORA.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(URLS[name], path)
    return path.read_text(encoding="utf-8")


def load_ud(name):
    """-> [(raw text, surface tokens, [(word, lemma)])]"""
    out = []
    for sent in parse(read(name)):
        text = sent.metadata.get("text")
        if not text:
            continue
        surface, words, mwt_end = [], [], 0
        for t in sent:
            if isinstance(t["id"], tuple):  # multiword token a-b
                surface.append(t["form"])
                mwt_end = t["id"][2]
                continue
            if t["id"] > mwt_end:
                surface.append(t["form"])
            if t["lemma"] not in (None, "_", ""):
                words.append((t["form"], t["lemma"]))
        out.append((text.strip(), surface, words))
    return out


def load_peyma():
    out, cur = [], []
    for line in read("peyma_test.txt").splitlines() + [""]:
        if line.strip():
            cur.append(line.split("|")[0])
        elif cur:
            out.append((" ".join(cur), cur, []))
            cur = []
    return out


def micro_f1(pairs):
    tp = fp = fn = 0
    for gold, pred in pairs:
        g, p = Counter(gold), Counter(pred)
        hit = sum((g & p).values())
        tp, fp, fn = tp + hit, fp + sum(p.values()) - hit, fn + sum(g.values()) - hit
    return 2 * tp / (2 * tp + fp + fn)


def canon_lemma(s):
    s = s.replace("‌", "").split("#")[0]  # UD/Hazm "past#present"
    return s.split("/")[0] if len(s) > 1 else s  # Shekar "past/present"


def timeit(fn, xs, reps=3):
    for x in xs[:3]:
        fn(x)
    t0 = time.perf_counter()
    for _ in range(reps):
        for x in xs:
            fn(x)
    return (time.perf_counter() - t0) / (reps * len(xs)) * 1000


def main():
    pipe, hazm_wt, shekar_wt = Pipeline(), HazmWT(), ShekarWT()
    tokenizers = {
        "Rishe": pipe.word_tokenize,
        "Hazm": hazm_wt.tokenize,
        "Shekar": lambda t: list(shekar_wt.tokenize(t)),
    }
    datasets = {
        "UD-Seraji": load_ud("fa_seraji-ud-test.conllu"),
        "UD-PerDT": load_ud("fa_perdt-ud-test.conllu"),
        "PEYMA": load_peyma(),
    }

    print("## Token F1 (micro, multiset, ZWNJ and punctuation kept)\n")
    print("| dataset | sentences | " + " | ".join(tokenizers) + " |\n|---|---|" + "---|" * len(tokenizers))
    for name, data in datasets.items():
        scores = [micro_f1((d[1], tok(d[0])) for d in data) for tok in tokenizers.values()]
        print(f"| {name} | {len(data)} | " + " | ".join(f"{s:.4f}" for s in scores) + " |")

    hazm_lemma, shekar_lemma = HazmLemmatizer(), ShekarLemmatizer()
    lemmatizers = {
        "identity": lambda w: w,
        "Rishe": pipe.lemmatize,
        "Hazm": hazm_lemma.lemmatize,
        "Shekar": lambda w: next(iter(shekar_lemma.transform([w])), w),
    }
    print("\n## Lemma accuracy (gold word tokens in isolation)\n")
    print("| dataset | words | " + " | ".join(lemmatizers) + " |\n|---|---|" + "---|" * len(lemmatizers))
    for name in ("UD-Seraji", "UD-PerDT"):
        words = [w for d in datasets[name] for w in d[2]]
        accs = [
            statistics.fmean(canon_lemma(fn(w)) == canon_lemma(g) for w, g in words)
            for fn in lemmatizers.values()
        ]
        print(f"| {name} | {len(words)} | " + " | ".join(f"{a:.4f}" for a in accs) + " |")

    sents = [d[0] for d in datasets["UD-PerDT"]][:1000]
    hazm_norm, shekar_norm = HazmNormalizer(), ShekarNormalizer()
    speed = {
        "tokenize: Rishe": pipe.word_tokenize,
        "tokenize: Hazm": hazm_wt.tokenize,
        "tokenize: Shekar": tokenizers["Shekar"],
        "normalize: Rishe": pipe.normalize,
        "normalize: Hazm": hazm_norm.normalize,
        "normalize: Shekar": shekar_norm,
        "full pipeline: Rishe": pipe,
    }
    print(f"\n## Speed (ms per sentence, {len(sents)} PerDT sentences, single thread)\n")
    print("| task | ms/sentence |\n|---|---|")
    for name, fn in speed.items():
        print(f"| {name} | {timeit(fn, sents):.5f} |")
    batch = sents * 10
    t0 = time.perf_counter()
    pipe.pipe(batch)
    print(f"| batch `pipe()`: Rishe | {(time.perf_counter() - t0) / len(batch) * 1000:.5f} |")


if __name__ == "__main__":
    main()
