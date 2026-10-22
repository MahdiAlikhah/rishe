#!/usr/bin/env python3
"""
Rishe vs Shekar: P1 maskers/removers.

    pip install shekar conllu
    python scripts/benchmark_maskers.py

Exact agreement compares full stripped outputs on the same sentence. Rishe
removes stopwords from its own (Hazm-derived) list, so that row measures list
overlap as much as behavior; the other rows use the same data.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8")

from benchmark import load_ud, timeit

from rishe import Pipeline
from shekar.preprocessing.maskers import (
    DiacriticMasker,
    DigitMasker,
    EmailMasker,
    EmojiMasker,
    HTMLTagMasker,
    HashtagMasker,
    MentionMasker,
    NonPersianLetterMasker,
    OffensiveWordMasker,
    PunctuationMasker,
    StopWordMasker,
    URLMasker,
)


def pct(hit, total):
    return 100.0 * hit / total if total else 0.0


def main():
    pipe = Pipeline()
    sentences = [text for text, _, _ in load_ud("fa_perdt-ud-test.conllu")]

    maskers = [
        ("url", URLMasker()),
        ("email", EmailMasker()),
        ("mention", MentionMasker()),
        ("hashtag", HashtagMasker()),
        ("emoji", EmojiMasker()),
        ("html", HTMLTagMasker()),
        ("non_persian", NonPersianLetterMasker()),
        ("stopwords", StopWordMasker()),
        ("offensive", OffensiveWordMasker()),
        ("digits", DigitMasker()),
        ("diacritics", DiacriticMasker()),
        ("punctuation", PunctuationMasker()),
    ]

    print("# Masker benchmarks: Rishe vs Shekar")
    print(f"\n{len(sentences)} UD-PerDT sentences, single thread, libraries built once.")
    print("Agreement = exact stripped output match with Shekar on the same sentence.\n")
    print("| masker | Rishe µs/sentence | Shekar µs/sentence | speedup | exact agreement |")
    print("|---|---|---|---|---|")
    for name, shekar in maskers:
        rishe_fn = lambda text, name=name: pipe.mask_stages(text, [name])
        shekar_fn = lambda text, op=shekar: op(text)
        agree = sum(rishe_fn(text) == shekar_fn(text) for text in sentences)
        rishe_us = timeit(rishe_fn, sentences, reps=3) * 1000
        shekar_us = timeit(shekar_fn, sentences, reps=3) * 1000
        print(
            f"| {name} | {rishe_us:.2f} | {shekar_us:.2f} "
            f"| {shekar_us / rishe_us:.1f}x | {pct(agree, len(sentences)):.2f}% |"
        )

    examples = [
        ("url", "به https://shekar.io مراجعه کنید"),
        ("email", "ایمیل info@shekar.io"),
        ("mention", "سلام @user! چطوری؟"),
        ("hashtag", "#سلام #خداحافظ"),
        ("emoji", "درود 😊🌟"),
        ("html", "<p>متن <strong>نمونه</strong></p>"),
        ("non_persian", "سلام hello 123!"),
        ("stopwords", "این کتاب است"),
        ("offensive", "این بی‌شرف است"),
        ("digits", "سال 1404"),
        ("diacritics", "کُجا نِشانِ قَدَم"),
        ("punctuation", "دریغ است ایران!"),
    ]

    print("\n## Examples\n")
    print("| masker | input | Rishe | Shekar |")
    print("|---|---|---|---|")
    for name, text in examples:
        print(
            f"| {name} | {text} | {pipe.mask_stages(text, [name])} "
            f"| {dict((n, m) for n, m in maskers)[name](text)} |"
        )


if __name__ == "__main__":
    main()
