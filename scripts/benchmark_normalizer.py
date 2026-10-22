#!/usr/bin/env python3
"""
Rishe vs Shekar (and Hazm where comparable): P1 normalizer operators.

    pip install hazm shekar conllu
    python scripts/benchmark_normalizer.py

Speed is measured on the full UD-PerDT test set, single thread, with every
library constructed once. Agreement is exact string equality on the same
sentences. Shekar exposes one class per operator; Hazm exposes one composite
Normalizer, so it is compared on the composite pipeline only.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8")

from benchmark import load_ud, timeit

from hazm import Normalizer as HazmNormalizer
from rishe import Pipeline
from shekar import Normalizer as ShekarNormalizer
from shekar.preprocessing.normalizers import (
    AlphabetNormalizer,
    ArabicUnicodeNormalizer,
    DigitNormalizer,
    OrthographicSpacingNormalizer,
    PunctuationNormalizer,
    PunctuationSpacingNormalizer,
    RepeatedLetterNormalizer,
    SpacingNormalizer,
    VerbalSpacingNormalizer,
    WordSpacingNormalizer,
    YaNormalizer,
)


def pct(hit, total):
    return 100.0 * hit / total if total else 0.0


def brief(text, limit=70):
    text = text.replace("\n", " ")
    return text if len(text) <= limit else text[: limit - 1] + "…"


def diff_window(left, right, width=28):
    at = 0
    for at in range(min(len(left), len(right))):
        if left[at] != right[at]:
            break
    lo = max(0, at - width)
    return brief(left[lo : at + width]), brief(right[lo : at + width])


def main():
    pipe = Pipeline()
    sentences = [text for text, _, _ in load_ud("fa_perdt-ud-test.conllu")]

    print("# Normalizer benchmarks: Rishe vs Shekar vs Hazm")
    print(f"\n{len(sentences)} UD-PerDT sentences, single thread, libraries built once.")
    print("Agreement = exact output match with Shekar on the same sentence.")

    operators = [
        ("alphabet", AlphabetNormalizer()),
        ("arabic_unicode", ArabicUnicodeNormalizer()),
        ("ya", YaNormalizer()),
        ("digits", DigitNormalizer()),
        ("punctuation", PunctuationNormalizer()),
        ("repeated_letters", RepeatedLetterNormalizer()),
        ("orthographic_spacing", OrthographicSpacingNormalizer()),
        ("punctuation_spacing", PunctuationSpacingNormalizer()),
        ("word_spacing", WordSpacingNormalizer()),
        ("verbal_spacing", VerbalSpacingNormalizer()),
        ("spacing", SpacingNormalizer()),
    ]

    print("\n## Per-operator speed and agreement\n")
    print("| operator | Rishe µs/sentence | Shekar µs/sentence | speedup | exact agreement |")
    print("|---|---|---|---|---|")
    differences = []
    for name, shekar_op in operators:
        rishe_fn = lambda text, name=name: pipe.normalize_stages(text, [name])
        shekar_fn = lambda text, op=shekar_op: op(text)
        agree = 0
        first_diff = None
        for text in sentences:
            rishe_out = rishe_fn(text)
            shekar_out = shekar_fn(text)
            if rishe_out == shekar_out:
                agree += 1
            elif first_diff is None:
                first_diff = (text, rishe_out, shekar_out)
        rishe_us = timeit(rishe_fn, sentences, reps=3) * 1000
        shekar_us = timeit(shekar_fn, sentences, reps=3) * 1000
        print(
            f"| {name} | {rishe_us:.2f} | {shekar_us:.2f} "
            f"| {shekar_us / rishe_us:.1f}x | {pct(agree, len(sentences)):.2f}% |"
        )
        if first_diff:
            differences.append((name, *first_diff))

    if differences:
        print("\n## First difference per operator\n")
        print("| operator | input | Rishe | Shekar |")
        print("|---|---|---|---|")
        for name, text, rishe_out, shekar_out in differences:
            rishe_view, shekar_view = diff_window(rishe_out, shekar_out)
            print(f"| {name} | {brief(text)} | {rishe_view} | {shekar_view} |")

    hazm_norm = HazmNormalizer()
    shekar_norm = ShekarNormalizer()
    composites = {
        "Rishe `normalize`": pipe.normalize,
        "Rishe `spacing`": lambda t: pipe.normalize_stages(t, ["spacing"]),
        "Shekar Normalizer": shekar_norm,
        "Hazm Normalizer": hazm_norm.normalize,
    }

    print("\n## Composite pipelines\n")
    print("| pipeline | µs/sentence | exact agreement with Rishe `normalize` |")
    print("|---|---|---|")
    rishe_default = {text: pipe.normalize(text) for text in sentences}
    for name, fn in composites.items():
        agree = sum(fn(text) == rishe_default[text] for text in sentences)
        us = timeit(fn, sentences, reps=3) * 1000
        print(f"| {name} | {us:.2f} | {pct(agree, len(sentences)):.2f}% |")

    print(
        "\n`Rishe normalize` is the 0.3.0 default and does not run punctuation, "
        "repeated-letter or full spacing normalization; `Rishe spacing` opts into "
        "the four spacing operators."
    )


if __name__ == "__main__":
    main()
