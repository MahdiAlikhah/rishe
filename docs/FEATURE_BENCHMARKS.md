# Feature benchmarks: Rishe vs Hazm vs Shekar

Measured on Windows 11, Python 3.13, hazm 0.12.1, shekar 1.7.0. Reproduce with
`python scripts/benchmark_features.py`,
`python scripts/benchmark_normalizer.py` and
`python scripts/benchmark_maskers.py`.

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

## Normalizer operators

1455 UD-PerDT sentences, single thread, libraries constructed once. Agreement is
**exact output equality with Shekar** on the same sentence; Shekar exposes one
class per operator, Hazm exposes one composite `Normalizer`.

| operator | Rishe µs/sentence | Shekar µs/sentence | speedup | exact agreement |
|---|---|---|---|---|
| alphabet | 1.92 | 5.86 | 3.1× | 100.00% |
| arabic_unicode | 1.79 | 5.58 | 3.1× | 100.00% |
| ya | 1.16 | 0.86 | 0.7× | 100.00% |
| digits | 1.88 | 5.76 | 3.1× | 100.00% |
| punctuation | 2.32 | 6.63 | 2.9× | 100.00% |
| repeated_letters | 1.37 | 3.63 | 2.7× | 100.00% |
| orthographic_spacing | 3.04 | 18.31 | 6.0× | 100.00% |
| punctuation_spacing | 6.90 | 10.67 | 1.5× | 100.00% |
| word_spacing | 9.43 | 33.25 | 3.5× | 80.00% |
| verbal_spacing | 3.54 | 11.31 | 3.2× | 99.04% |
| spacing | 20.02 | 77.25 | 3.9× | 79.24% |

Composite pipelines:

| pipeline | µs/sentence | exact agreement with Rishe `normalize` |
|---|---|---|
| Rishe `normalize` | 1.23 | 100.00% |
| Rishe `spacing` | 19.84 | 84.54% |
| Shekar `Normalizer` | 107.52 | 80.96% |
| Hazm `Normalizer` | 100.55 | 73.88% |

- `ya` is the one operator where Shekar's compiled regex beats Rishe (0.86 vs
  1.16 µs). Everything else is 1.5–6× faster.
- Word-spacing disagreement is almost entirely Shekar's `compound_words.csv`
  (`ابن سینا` → `ابن‌سینا`, `به طور` → `به‌طور`). Rishe validates prefix and
  suffix joins against its compiled lexicons and does not bundle that list.
- The remaining verbal-spacing disagreements are mostly Shekar over-joins
  (`میزان` → `می‌زانی`, `در بیان` → `دربیان`); Rishe stays conservative.
- Sub-2 µs operators vary about 10% run to run.

## Maskers and removers

1455 UD-PerDT sentences, single thread, libraries constructed once. Exact
agreement is exact stripped output equality with Shekar.

| masker | Rishe µs/sentence | Shekar µs/sentence | speedup | exact agreement |
|---|---|---|---|---|
| url | 0.96 | 0.53 | 0.5× | 100.00% |
| email | 0.96 | 1.31 | 1.4× | 100.00% |
| mention | 0.95 | 0.49 | 0.5× | 100.00% |
| hashtag | 0.96 | 0.49 | 0.5× | 100.00% |
| emoji | 1.21 | 2.61 | 2.2× | 100.00% |
| html | 0.96 | 0.62 | 0.6× | 100.00% |
| non_persian | 2.18 | 1.25 | 0.6× | 100.00% |
| stopwords | 2.41 | 304.75 | 126.5× | 17.53% |
| offensive | 1.72 | 115.65 | 67.3× | 100.00% |
| digits | 1.97 | 5.85 | 3.0× | 100.00% |
| diacritics | 3.17 | 5.82 | 1.8× | 100.00% |
| punctuation | 1.66 | 6.06 | 3.6× | 100.00% |

- 11 of 12 maskers match Shekar exactly. The stopword row measures list overlap:
  Rishe uses its 389-word Hazm-derived list, Shekar its 301-word list.
- Stopwords and offensive words are 67–126× faster: Rishe runs one Aho-Corasick
  pass with word-boundary checks; Shekar applies one compiled regex per word.
- The sub-1 µs no-op rows (`url`, `mention`, `hashtag`, `html`) are dominated by
  the pyo3 call floor (0.78 µs for `mask_stages` with no stages), not masker
  work. Shekar returns the original Python string when nothing matches.
- Maskers strip their output like Shekar and do not collapse the spaces left
  behind. Run `normalize_stages(..., ["spacing"])` afterwards if needed.

Reproduce: `python scripts/benchmark_maskers.py`.

## Core accuracy unchanged

The P0 additions do not regress the core benchmark: token F1 1.0000 / 0.9989 /
0.9028 (Seraji / PerDT / PEYMA) and lemma accuracy 0.9209 / 0.9316, with
tokenize at ~4.1 µs and the full pipeline at ~16.3 µs per sentence.
