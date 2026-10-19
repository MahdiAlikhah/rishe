# Benchmarks: Rishe vs Hazm vs Shekar

Measured on Windows 11, Python 3.13, single thread, hazm 0.12.1, shekar 1.7.0.
Reproduce with `python scripts/benchmark.py` (downloads the corpora on first run).

## Method

- Each library is constructed **once** and reused. `hazm.word_tokenize()`
  rebuilds `WordTokenizer()` on every call (~130 ms of data loading), so
  timing that helper measures initialization, not tokenization. Reused, Hazm
  tokenizes in ~0.006 ms.
- Full test sets: UD Persian-Seraji (600 sentences), UD Persian-PerDT (1455),
  PEYMA (1026). No subsampling.
- Token F1 is micro-averaged over per-sentence multisets of **surface**
  tokens (UD multiword tokens kept whole). ZWNJ and punctuation count.
- Lemma accuracy is exact match against the UD lemma (past-stem side of
  `past#present`), with each gold word lemmatized in isolation. The
  `identity` column predicts the word itself; it shows how much of the score
  is free.
- Rishe's lexicons derive from Hazm's data files, so the Rishe/Hazm lemma
  comparison is not independent.

## Tokenization (token F1)

| dataset | sentences | Rishe | Hazm | Shekar |
|---|---|---|---|---|
| UD-Seraji | 600 | 0.9963 | 0.9810 | **0.9992** |
| UD-PerDT | 1455 | 0.9906 | 0.9609 | **0.9976** |
| PEYMA | 1026 | **0.9057** | 0.8901 | 0.9018 |

Shekar is the most accurate tokenizer on the UD sets; Rishe is second and
clearly ahead of Hazm. On PEYMA all three are close.

## Lemmatization (accuracy)

| dataset | words | identity | Rishe | Hazm | Shekar |
|---|---|---|---|---|---|
| UD-Seraji | 16021 | 0.8354 | 0.9005 | **0.9048** | 0.8056 |
| UD-PerDT | 24131 | 0.7857 | 0.8832 | **0.9009** | 0.8182 |

Hazm is the best lemmatizer, narrowly on Seraji and by ~2 points on PerDT.
Shekar's lemmas score below the identity baseline on Seraji.

## Speed (ms per sentence, 1000 PerDT sentences)

| task | ms/sentence |
|---|---|
| tokenize: Rishe | 0.0048 |
| tokenize: Hazm | 0.0057 |
| tokenize: Shekar | 0.0043 |
| normalize: Rishe | **0.0014** |
| normalize: Hazm | 0.0947 |
| normalize: Shekar | 0.1024 |
| full pipeline: Rishe (normalize + formalize + tokenize + lemmatize + sentences + ezafe) | 0.0169 |
| batch `pipe()`: Rishe (12 threads) | 0.0025 |

- Tokenization speed is a tie; all three are in the microseconds.
- Normalization is where Rishe is clearly faster (~70×).
- The full-pipeline row does more work than the competitors' tokenizers, so
  it is not comparable to them one-to-one. `pipe()` is ~6× faster than
  calling the pipeline in a Python loop.

### Batch throughput

`Pipeline.pipe()` on the full PerDT test set repeated 20× (29,100 sentences,
~484k tokens), 12-core machine:

| threads | tokens/s | sentences/s | speedup |
|---|---|---|---|
| 1 | 1.03M | 62k | 1.0× |
| 2 | 1.98M | 119k | 1.9× |
| 4 | 3.65M | 220k | 3.5× |
| 8 | 5.84M | 351k | 5.7× |
| 12 | 6.66M | 400k | 6.5× |

Median of 5 runs each. Reproduce with
`RAYON_NUM_THREADS=<n> python scripts/benchmark.py` (the last line of
output). Scaling is near-linear to 4 threads (~88% efficiency) and reaches
6.5× on 12.

## Not measured

- Sentence splitting: an earlier metric compared sentence *counts*, which
  cannot distinguish systems; it has been removed rather than replaced.
- Informal → formal conversion and ezafe detection have no gold data here.
- Hazm and Shekar offer features Rishe does not (POS tagging, NER, parsing,
  embeddings, spell checking, classifiers). Use them for those.

## Known limitations

- Informal → formal conversion is lexicon- and rule-based. Common forms
  (`اون`, `خونه`, `داره`, `چیه`, `میخوام`) are converted; many suffixed or
  rarer forms (`خونه‌ام`, `کتابه`) are left unchanged.
- Lemmas follow the Universal Dependencies convention (verbs lemmatize to the
  past stem), which is why the lemma numbers above are against UD gold.
- Ezafe detection is rule-based and has not been evaluated.
- Sentence splitting has no gold-standard evaluation here.
