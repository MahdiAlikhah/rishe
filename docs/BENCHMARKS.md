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
  Rishe is timed and scored through `Pipeline.tokenize(text)`, which returns
  surface tokens and does not normalize, the same as Hazm and Shekar.
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
| batch `process_batch()`: Rishe (12 threads) | 0.0026 |

- Tokenization is close between Rishe and Shekar; both are faster than Hazm.
- Normalization is where Rishe is clearly faster (~80×).
- The full-pipeline row does more work than the competitors' tokenizers, so
  it is not comparable to them one-to-one. `process_batch()` is ~6× faster than
  calling the pipeline in a Python loop.

### Batch throughput

`Pipeline.process_batch()` on the full PerDT test set repeated 20× (29,100 sentences,
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

## Rust engine (criterion)

The rows above compare Rishe with Hazm and Shekar through Python. This is the
engine alone: Windows 11, release profile, single thread, rishe 0.11.4,
2026-11-11, reproduce with `cargo bench --bench tier1_bench`. Re-measured on
rishe 1.0.5 (2026-11-19): `end_to_end_process` 9.7 – 10.1 µs, so the range
below still holds.

| benchmark | median per call |
|---|---|
| `end_to_end_process` (one sentence through `Pipeline::process`) | 8.4 – 10.0 µs |

Two runs on this host spread about ±15%, so compare the range, not a single
digit, when checking a later change. This is the reference for `1.0.0` onward.

## Sentence vectors: FarsTail NLI

The shipped pack is a static word-vector model and `sentence_vector()` is the
mean of the vectors of its in-vocabulary tokens. That is **not** a trained
sentence encoder, so this section measures what the current engine does rather
than claiming a sentence-embedding quality. Reproduce with
`python scripts/benchmark_nli.py` (downloads the data on first run).

Data: [FarsTail](https://github.com/dml-qom/FarsTail), 10,367 Persian
premise/hypothesis pairs labelled entailment, contradiction or neutral, by the
Data Mining Lab at the University of Qom, **Apache-2.0**. It is used instead of
ParsiNLU because ParsiNLU is CC BY-NC-SA (evaluation only), so a number measured
on it could not be published here.

Frozen features (cosine, difference, elementwise product) with a closed-form
ridge on top; no sampling, no random seed, identical output across runs.

| model | val accuracy | test accuracy |
|---|---|---|
| Rishe sentence vectors (pair) | 0.5322 | **0.5601** |
| Rishe sentence vectors (hypothesis only) | 0.4704 | 0.4578 |

FarsTail's published baselines, for where this sits: DecompAtt (word2vec) 0.6662,
HBMP 0.6604, ESIM (fastText) 0.7116, mBERT 0.8338.

Read this honestly:

- **0.5601 is below every published baseline**, including DecompAtt at 0.6662.
  Mean pooling over static vectors loses word order and negation, which is
  exactly what entailment turns on. Per class: contradiction is the failure,
  146/510 = 28.6%, against neutral 77.9% and entailment 60.3%.
- The **hypothesis-only baseline is the bias control**: 0.4578, so the pair
  model's +10.23 points is real signal and not annotation artefact. Chance is
  0.3333.
- On FarsTail's own hard subsets, where biased models are designed to fail, the
  pair model gets 0.5050 (n=699, hypothesis-only hard) and 0.3671 (n=681,
  overlap-based hard).
- **Training-free check:** within a single premise the hypotheses differ only in
  relation, so a representation that separates relations should order them. It
  puts entailment above contradiction on 350/487 premises = **71.9%**.

The gap to DecompAtt is the argument for a trained sentence encoder (P1 in
`training/ROADMAP.md`): the pooling is the limit, not the vectors.

## Not measured

- Sentence splitting: an earlier metric compared sentence *counts*, which
  cannot distinguish systems; it has been removed rather than replaced.
- Informal → formal conversion and ezafe detection have no gold data here.
- Hazm and Shekar offer features Rishe does not (POS tagging, NER, parsing,
  classifiers). Use them for those. Embedding *quality* is measured above on
  FarsTail; there is still no head-to-head embedding comparison here.

## Known limitations

- Informal → formal conversion is lexicon- and rule-based. Common forms
  (`اون`, `خونه`, `داره`, `چیه`, `میخوام`) are converted; many suffixed or
  rarer forms (`خونه‌ام`, `کتابه`) are left unchanged.
- Lemmas follow the Universal Dependencies convention (verbs lemmatize to the
  past stem), which is why the lemma numbers above are against UD gold.
- Ezafe detection is rule-based and has not been evaluated.
- Sentence splitting has no gold-standard evaluation here.
