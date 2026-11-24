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
| `alignment_similarity` + sentence vectors | 0.5856 | **0.5959** |

FarsTail's published baselines, for where this sits: DecompAtt (word2vec) 0.6662,
HBMP 0.6604, ESIM (fastText) 0.7116, mBERT 0.8338.

Read this honestly:

- **0.5601 is below every published baseline**, including DecompAtt at 0.6662.
  Mean pooling over static vectors loses word order and negation, which is
  exactly what entailment turns on. Per class: contradiction is the failure,
  146/510 = 28.6%, against neutral 77.9% and entailment 60.3%. Adding
  `alignment_similarity` is the cheapest partial fix (see below).
- The **hypothesis-only baseline is the bias control**: 0.4578, so the pair
  model's +10.23 points is real signal and not annotation artefact. Chance is
  0.3333.
- On FarsTail's own hard subsets, where biased models are designed to fail, the
  pair model gets 0.5050 (n=699, hypothesis-only hard) and 0.3671 (n=681,
  overlap-based hard).
- **Training-free check:** within a single premise the hypotheses differ only in
  relation, so a representation that separates relations should order them. It
  puts entailment above contradiction on 350/487 premises = **71.9%**; blending
  `alignment_similarity` into the score raises that to 366/487 = **75.2%**.

`alignment_similarity()` matches each word to its closest word on the other side
and averages the two directions. It keeps track of *which* words correspond,
which mean pooling discards, and needs no new download: it is arithmetic over the
same static table. In the shipped `scripts/benchmark_nli.py` (the numbers above)
it moves test accuracy 0.5601 → **0.5959**, +3.58 points, closing about a sixth of
the gap to DecompAtt without a single new parameter. A wider offline feature sweep
reached 0.6074 and the Rust method alone as a single extra feature gave 0.5908, so
treat ~0.60 as the plateau for this approach, not a step on the way to 0.71: the
remaining gap is word order, and static vectors cannot represent it. Beating
0.6662 needs a contextual encoder; the shipped `rishe-vector-e5-small-fa` pack is
one, and the paragraph below measures it. No encoder is trained here — the P1
owned-backbone goal was closed as won't-do on the evidence below.

Contextual encoders were measured on the same split and protocol: e5-base
(multilingual, MIT) frozen at 0.6170 raw and **0.6419** with the documented
`query:`/`passage:` prefixes; contrastive fine-tuning on FarsTail *lowered* it
(0.6145 after 8 epochs) because a bi-encoder compresses each sentence to one
vector; the same backbone as a **cross-encoder** reaches **0.8107**, which no
one-vector-per-sentence model can match. Those numbers are for orientation only:
that backbone is a 112–266 MB int8 pack against this pack's size, so it is not
shipped.

## Semantic retrieval: MIRACL fa dev

The section above measures entailment, and a single vector cannot carry a
three-way relation. **Retrieval is the task this pack is actually for**, and it
is where the static pack fails outright rather than merely scoring lower.

Data: [MIRACL](https://project-miracl.github.io/) Persian dev split, **Apache-2.0**
and ungated: 632 queries, 1,314 relevance judgements, 2,207,172 passages. All
1,301 gold document ids were verified present before measuring, so a low score
cannot be a missing-corpus artefact. Both systems were measured through the
**installed package** (`Embeddings(hub.path(...))` and
`ModelPack(hub.path(...)).sentence_vector()`), not a reimplementation.

Reproduce with `python scripts/benchmark_retrieval.py` (fetches the queries and
qrels on first run, ~2 MiB). It needs a passage cache, because encoding 2.2M
passages costs ~36 h single-threaded on CPU at 58 ms/passage: build one once
with `--encode`, then every scoring run reads it back in seconds. Both rows are
filled in a **single** corpus pass, and the script warns when the cache is
missing judged documents, since that floors the score for a reason unrelated to
the model. `--limit 5000` is a smoke run that fits on a laptop.

| system | nDCG@10 | R@10 | R@100 | download |
|---|---|---|---|---|
| `rishe-embed-fa-300-v2` (static word vectors) | 0.0804 | 0.1227 | 0.3155 | 42 MB |
| `rishe-vector-e5-small-fa` (frozen encoder) | **0.4918** | 0.6267 | **0.8889** | 129 MB |

Downloads are whole packs, model plus tokenizer; the encoder's graph alone is
112.6 MB of that 129, the rest is an XLM-R tokenizer with a 250k vocabulary.

Two things follow, and the second matters more than the first:

- **A 6.1× gap in nDCG@10.** Mean pooling over static word vectors is not a
  sentence embedding: it has no notion of which words correspond, so a passage
  can share every topic word with a query and still be ranked wrongly.
- **Recall, not ranking, is what the static pack loses.** It finds only **31.6%**
  of relevant passages in its top 100, so it cannot be used as a first stage and
  improved later — the candidates are simply not there. The encoder reaches
  **88.89%**, which is what makes it a usable retriever.

The encoder ships as an optional second download; the base install and the
existing packs are unchanged.

### Reranking on top of the encoder

Because R@100 is 0.8889, the remaining headroom is in *ordering*, not recall.
A cross-encoder reranker over the encoder's top 50, same queries:

| reranker | license | nDCG@10 | R@10 |
|---|---|---|---|
| none | — | 0.4918 | 0.6267 |
| FarsTail NLI cross-encoder (trained here) | MIT | 0.0672 | 0.1176 |
| `BAAI/bge-reranker-base` | MIT | 0.2703 | 0.3995 |
| `cross-encoder/mmarco-mMiniLMv2-L12-H384-v1` | apache-2.0 | **0.5609** | 0.7092 |

The shipped pack is that last row, re-measured end to end on this release:
2,207,172 passages, 632 queries, 1,314 relevance judgements, scoring one pair at
a time with no padding (padding shifts a logit by −0.25). Top-50 beats top-100
(0.5609 vs 0.5526), matching the earlier sweep. Reranking permutes candidates,
so it cannot change set recall — that is why R@100 for a top-50 pass is capped at
R@50, and why a top-100 pass reproduces the encoder's 0.8889 exactly.

**+6.9 nDCG@10 for zero gradient steps**, and it beats the NLI cross-encoder
trained in this repo. The reason is the objective, not the size: mMARCO is
multilingual MS MARCO *retrieval* data, while entailment is a different question.
Reranking by P(entailment) actively *hurts* — 0.4918 → 0.0672 — and that is a
measured negative, not a tuning miss: across 20,000 (query, passage) pairs the
NLI model labelled 19,258 **neutral**, with `pearson(P(entailment), relevant) =
+0.011`. "Does this follow from that?" carries no signal for "is this what they
meant?".

Knobs measured: `max_length` 256 vs 512 is within noise; **top-50 beats top-100**
(0.5609 vs 0.5526) because rescanning more candidates gives the reranker more
chances to be fooled, at 4.3 min vs 8.8 min for the full dev set.

The reranker ships as `rishe-rerank-mmarco-minilmv2` (130 MB, Apache-2.0), pulled
separately with `hub pull` — nobody downloads it unless they ask for it. It is a
cross-encoder, so it scores a pair and cannot produce vectors; it is only useful
on top of the encoder's shortlist, bounded by the encoder's recall.

**Runtime caveat:** the figures above were measured through onnxruntime, while the
crate runs the same int8 graph through tract. On 480 real benchmark pairs from 12
queries the two order candidates closely but not identically — Kendall tau 0.92
(range 0.86–0.95), top-10 overlap 94%, mean absolute logit difference 0.22 (worst
1.03). Expect a tract-based run to land within a few nDCG points rather than
bit-identical, and compare logits only within one runtime. The reranker's value is
the ordering, not the number.

**Caveat on reproducibility:** unlike the sections above, these retrieval and
reranker numbers have **no script in this repository** — they were measured in a
notebook against a marimo kernel. Promoting that evaluation into `scripts/` is
tracked in `training/ROADMAP.md` (P2). Treat the ratios as measured but not yet
re-derivable here.


## Not measured

- Sentence splitting: an earlier metric compared sentence *counts*, which
  cannot distinguish systems; it has been removed rather than replaced.
- Informal → formal conversion and ezafe detection have no gold data here.
- Hazm and Shekar offer features Rishe does not (POS tagging, NER, parsing,
  classifiers). Use them for those. Embedding *quality* is measured above on
  FarsTail (entailment) and MIRACL fa dev (retrieval); there is still no
  head-to-head embedding comparison against another Persian embedding here, so
  the retrieval numbers show what the pack does, not how it ranks against a
  competitor.

## Known limitations

- Informal → formal conversion is lexicon- and rule-based. Common forms
  (`اون`, `خونه`, `داره`, `چیه`, `میخوام`) are converted; many suffixed or
  rarer forms (`خونه‌ام`, `کتابه`) are left unchanged.
- Lemmas follow the Universal Dependencies convention (verbs lemmatize to the
  past stem), which is why the lemma numbers above are against UD gold.
- Ezafe detection is rule-based and has not been evaluated.
- Sentence splitting has no gold-standard evaluation here.
