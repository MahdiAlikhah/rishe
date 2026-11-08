#!/usr/bin/env python3
"""Small deterministic benchmark gate for CI.

The golden file contains exact outputs plus generous latency ceilings. The
ceilings are intentionally loose: this catches large regressions, not normal
CI noise.
"""
import json
import statistics
import sys
import time
from pathlib import Path

from rishe import Pipeline, parse_date

ROOT = Path(__file__).with_name("benchmark_golden.json")


def measure(fn, iterations):
    for _ in range(50):
        fn()
    runs = []
    for _ in range(3):
        start = time.perf_counter()
        for _ in range(iterations):
            fn()
        runs.append(time.perf_counter() - start)
    return statistics.median(runs)


def main():
    pipe = Pipeline()
    cases = {
        "normalize": (lambda: pipe.normalize("مي كنم"), "می‌کنم"),
        "tokenize": (lambda: pipe.tokenize("سلام دنیا"), ["سلام", "دنیا"]),
        "lemmatize": (lambda: pipe.lemmatize("می‌روم"), "رفت"),
        "mask": (
            lambda: pipe.mask("به https://a.b برو", ["url"]),
            "به <URL> برو",
        ),
        "date": (lambda: parse_date("۱۲ مرداد ۱۴۰۲").iso, "2023-08-03"),
    }
    golden = json.loads(ROOT.read_text(encoding="utf-8"))
    failed = False
    for name, (fn, expected) in cases.items():
        actual = fn()
        if actual != expected:
            print(f"{name}: output changed: {actual!r} != {expected!r}", file=sys.stderr)
            failed = True
        limit = golden[name]
        elapsed = measure(fn, limit["iterations"])
        print(
            f"{name}: {elapsed:.3f}s / {limit['max_seconds']:.1f}s "
            f"({limit['iterations']} iterations)"
        )
        if elapsed > limit["max_seconds"]:
            print(f"{name}: too slow", file=sys.stderr)
            failed = True
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
