# Feature benchmarks: Rishe vs Hazm vs Shekar

Measured on Windows 11, Python 3.13, hazm 0.12.1, shekar 1.7.0. Reproduce with
`python scripts/benchmark_features.py`.

## Method and caveats

- There is **no gold data** for most of these features (stemming, conjugation,
  inflection, token splitting, spelling). The tables report the closest
  defensible proxy: agreement with UD gold lemmas for stemming, hand-written
  spot-checks for conjugation, and cross-library agreement for the rest.
- Hazm has no spell checker or inflector; Shekar has no token splitter. Those
  cells are marked N/A rather than scored with a different task.
- Speed is single-thread and includes only the operation, not model or lexicon
  construction. Hazm's `TokenSplitter` and Shekar's `SpellChecker` are slow by
  design; their times are real, not setup costs.
- Rishe's stopword list is derived from Hazm's `stopwords.dat`, so the
  Rishe/Hazm stopword comparison is identical by construction.

## Stemming

Proxy: stem equals the UD gold lemma (exact), and stem is a prefix of the gold
lemma. Alphabetic word tokens only.

| dataset | words | library | exact lemma | lemma prefix | ms/word |
|---|---|---|---|---|---|
| UD-Seraji | 14471 | identity | 81.78% | 81.94% | — |
| UD-Seraji | 14471 | **Rishe** | **89.18%** | 90.65% | **0.0004** |
| UD-Seraji | 14471 | Hazm | 66.37% | **90.78%** | 0.0008 |
| UD-Seraji | 14471 | Shekar | 76.07% | 88.18% | 0.0030 |
| UD-PerDT | 21851 | identity | 76.34% | 76.82% | — |
| UD-PerDT | 21851 | **Rishe** | **89.49%** | **90.59%** | **0.0004** |
| UD-PerDT | 21851 | Hazm | 64.35% | 86.56% | 0.0007 |
| UD-PerDT | 21851 | Shekar | 72.00% | 82.99% | 0.0028 |

Rishe has the highest exact-lemma score and is 2–7× faster. Hazm's prefix score
is slightly higher on Seraji; its exact score is 23 points lower.

## Conjugation

Spot-check: 29 hand-written forms across six verbs (past, present, subjunctive,
future, perfect). Coverage: fraction of Rishe's generated forms that the other
library also generates.

| library | forms (6 verbs) | spot-check | Rishe forms covered | ms/verb |
|---|---|---|---|---|
| **Rishe** | 288 | **29/29** | 100.0% | **0.010** |
| Hazm | 3210 | 28/29 | 95.8% | 0.152 |
| Shekar | 2352 | **29/29** | 100.0% | 0.080 |

Rishe generates a compact 8-category set (48 forms/verb) instead of every
tense/mood/voice, covers the same spot-check, and is 8–15× faster.

## Inflection

Rishe vs Shekar. Hazm has no inflector.

| form | agreement | Rishe ms/op | Shekar ms/op |
|---|---|---|---|
| comparative | **100.0%** | 0.0003 | 0.0001 |
| superlative | **100.0%** | 0.0003 | 0.0001 |
| plural | **100.0%** | 0.0003 | 0.0001 |
| possessive (6 forms) | 55.0% | 0.0019 | 0.0005 |

Possessive disagreement is orthographic: Shekar writes a uniform ZWNJ before
singular suffixes (`کتاب‌م`, `خانه‌م`); Rishe follows standard orthography
(`کتابم`, `خانه‌ام`). Comparative, superlative, and plural match exactly.

## Token splitting

Rishe returns one split; Hazm returns all candidates. "Among" means Rishe's
split is one of Hazm's candidates.

| tokens | Rishe split | Hazm split | Rishe split among Hazm candidates | same first candidate | Rishe ms/token | Hazm ms/token |
|---|---|---|---|---|---|---|
| 15 | 11 | 13 | 11/15 | 8/15 | **0.0006** | 12.0499 |

Examples:

| token | Rishe | Hazm candidates |
|---|---|---|
| کتاب‌ها | `['کتاب', 'ها']` | `[('کتاب','ها'), ('کت','اب‌ها'), ('کتاب‌ها',)]` |
| کتابهایم | `['کتاب', 'هایم']` | `[..., ('کتاب','هایم'), ...]` |
| خانه‌ام | `['خانه', 'ام']` | `[('خانه','ام'), ('خانه‌ام',)]` |
| بهش | `['به', 'او']` | `[('بهش',)]` |
| می‌روم | `['می', 'روم']` | `[('می','روم')]` |
| داستان‌سرا | `['داستان', 'سرا']` | `[('داستان','سرا'), ('داستان‌سرا',)]` |

Hazm is ~20,000× slower per token because it lemmatizes every candidate split.
Rishe also handles fused clitics (`بهش`, `ازش`) that Hazm leaves whole.

## Spell correction

Synthetic 1-edit typos: 500 frequency-ranked words (freq ≥ 1000, length ≥ 4),
one random delete/transpose/replace/insert per word, typos that are valid words
excluded. Hazm has no spell checker.

| library | top-1 recovers original | correct in top-5 | ms/word |
|---|---|---|---|
| **Rishe** | **80.8%** | **96.2%** | **0.159** |
| Shekar | 60.8% | 77.6% | 94.372 |

Rishe is ~600× faster and 20 points better at top-1 on this set. Shekar's
lexicon differs from the frequency list used to generate typos, so part of its
gap is vocabulary coverage; the speed gap is structural.

## Stopword lists

| library | count | shared with Rishe | % of Rishe | Jaccard |
|---|---|---|---|---|
| Rishe | 389 | 389 | 100.0% | 100.0% |
| Hazm | 389 | 389 | 100.0% | 100.0% |
| Shekar | 301 | 234 | 60.2% | 51.3% |

Rishe and Hazm are identical by construction (same source list). Shekar's list
is smaller and overlaps on 60% of Rishe's words.

## Dual-stem lemma agreement

Present stems are **not** in UD gold; this measures agreement, not accuracy.
UD-Seraji unique words: 3740.

| library | words with a present stem |
|---|---|
| Rishe | 359 (9.6%) |
| Hazm | 287 (7.7%) |
| Shekar | 456 (12.2%) |

On the 257 words where all three return a present stem:

| pair | agreement |
|---|---|
| Rishe vs Hazm | 84.8% |
| Rishe vs Shekar | 87.2% |
| Hazm vs Shekar | 87.9% |

Rishe exposes 25% more present stems than Hazm and agrees with both libraries on
~85–87% of the shared cases.

## Token spans

Rishe only. Python character offsets; checked by slicing the original text.

| sentences | tokens | correct slices |
|---|---|---|
| 1000 | 15869 | 100.00% |

## Core accuracy unchanged

The P0 additions do not regress the core benchmark: token F1 1.0000 / 0.9989 /
0.9028 (Seraji / PerDT / PEYMA) and lemma accuracy 0.9209 / 0.9316, with
tokenize at ~4.1 µs and the full pipeline at ~16.3 µs per sentence.
