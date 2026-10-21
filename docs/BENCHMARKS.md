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
  Rishe is timed and scored through `Pipeline.tokenize`, which does not
  normalize, the same as Hazm and Shekar; `word_tokenize()` is the
  normalize+tokenize convenience path.
- Lemma accuracy is exact match against the UD lemma (past-stem side of
  `past#present`), with each gold word lemmatized in isolation and without
  pre-normalization. The `identity` column predicts the word itself; it shows
  how much of the score is free.
- Rishe's lexicons derive from Hazm's data files, so the Rishe/Hazm lemma
  comparison is not independent.

## Tokenization (token F1)

| dataset | sentences | Rishe | Hazm | Shekar |
|---|---|---|---|---|
| UD-Seraji | 600 | **1.0000** | 0.9810 | 0.9992 |
| UD-PerDT | 1455 | **0.9989** | 0.9609 | 0.9976 |
| PEYMA | 1026 | **0.9028** | 0.8901 | 0.9018 |

Rishe is the most accurate tokenizer on both UD sets, narrowly ahead of
Shekar; on PEYMA all three are close.

## Lemmatization (accuracy)

| dataset | words | identity | Rishe | Hazm | Shekar |
|---|---|---|---|---|---|
| UD-Seraji | 16021 | 0.8354 | **0.9209** | 0.9048 | 0.8058 |
| UD-PerDT | 24131 | 0.7857 | **0.9316** | 0.9009 | 0.8170 |

Rishe is the best lemmatizer on both sets, ahead of Hazm by ~1.6 points on
Seraji and ~3.1 on PerDT. Shekar's lemmas score below the identity baseline
on Seraji.

## Speed (ms per sentence, 1000 PerDT sentences)

| task | ms/sentence |
|---|---|
| tokenize: Rishe | **0.0041** |
| tokenize: Hazm | 0.0057 |
| tokenize: Shekar | 0.0044 |
| normalize: Rishe | **0.0012** |
| normalize: Hazm | 0.0958 |
| normalize: Shekar | 0.1041 |
| full pipeline: Rishe (normalize + formalize + tokenize + lemmatize + sentences + ezafe) | 0.0163 |
| batch `pipe()`: Rishe (12 threads) | 0.0026 |

- Tokenization is close between Rishe and Shekar; both are faster than Hazm.
- Normalization is where Rishe is clearly faster (~80×).
- The full-pipeline row does more work than the competitors' tokenizers, so
  it is not comparable to them one-to-one. `pipe()` is ~6× faster than
  calling the pipeline in a Python loop.

### Batch throughput

`Pipeline.pipe()` on the full PerDT test set repeated 20× (29,100 sentences,
~484k tokens), 12-core machine:

| threads | tokens/s | sentences/s | speedup |
|---|---|---|---|
| 1 | 1.00M | 60k | 1.0× |
| 2 | 1.88M | 113k | 1.9× |
| 4 | 3.65M | 219k | 3.6× |
| 8 | 5.44M | 327k | 5.4× |
| 12 | 6.49M | 390k | 6.5× |

Median of 5 runs each. Reproduce with
`RAYON_NUM_THREADS=<n> python scripts/benchmark.py` (the last line of
output). Scaling is near-linear to 4 threads (~91% efficiency) and reaches
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
