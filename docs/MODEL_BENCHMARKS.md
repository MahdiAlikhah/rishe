# Model benchmarks

Phase 0 baselines and the 0.5.0 compact pack results. The baseline numbers are
what the packs had to beat; the Rishe rows are measured with the published packs.

## Method

- POS: UD Persian-Seraji and UD Persian-PerDT test sets. Hazm is fed gold
  tokens; Shekar runs its own tokenizer, so predictions are aligned to gold
  tokens by surface form and coverage is reported.
- NER: PEYMA test, entity-level exact match after stripping whitespace and
  ZWNJ. Hazm is N/A because its NER wrapper requires an external spaCy model.
- Latency: single thread, models constructed once, first three calls warmed up,
  measured during one full accuracy pass.
- Libraries: hazm 0.12.1 (CRF POS), shekar 1.7.0 (q8 ONNX ALBERT POS/NER).
  Reproduce with `python scripts/benchmark_models.py`.

## POS

| dataset | sentences | library | coverage | token acc | micro F1 | macro F1 | ms/sentence |
|---|---|---|---|---|---|---|---|
| UD-Seraji | 600 | Hazm | 1.0000 | 0.9066 | 0.9066 | 0.8838 | 0.19 |
| UD-Seraji | 600 | Shekar | 0.9867 | 0.8250 | 0.8250 | 0.8381 | 40.63 |
| UD-PerDT | 1455 | Hazm | 1.0000 | 0.8490 | 0.8490 | 0.8032 | 0.12 |
| UD-PerDT | 1455 | Shekar | 0.9735 | 0.9739 | 0.9739 | 0.9581 | 35.15 |

The POS story is split: Shekar owns PerDT, Hazm owns Seraji. Training on both
splits is the opening — neither baseline is strong on both.

## NER

| dataset | sentences | library | gold entities | precision | recall | F1 | ms/sentence |
|---|---|---|---|---|---|---|---|
| PEYMA | 1026 | Shekar | 1982 | 0.5738 | 0.5691 | 0.5714 | 270.63 |

Exact-span matching is strict (no partial credit, punctuation included), so
this is a lower bound. Even allowing for that, Shekar's NER is the weakest
baseline and 4–8× slower than its POS model. This is where a compact ALBERT
fine-tuned on Arman/Peyma/Naab should win clearly.

## Runtime spike: tract vs onnxruntime

Same machine, single thread, 11–13 tokens, steady state:

| model | file | tract | onnxruntime | parity |
|---|---|---|---|---|
| ALBERT-fa fp32 | 44.8 MB | 22.7 ms | 6.35 ms | max abs Δ 1.1e-5 |
| Shekar POS, dynamic int8 | 40.2 MB | 48.8 ms | 7.74 ms | argmax 100%, max abs Δ 4.2e-3 |
| Shekar POS, weight-only int8 | 11.7 MB | fails on symbolic shapes; static 128-token pad 305 ms | — | — |

Decisions:

- **tract is correct but its int8 path is not competitive.** We ship the
  compact pack as fp32 in 0.5.0: ~45 MB, ~23 ms/sentence, still faster than
  Shekar's 35–41 ms POS end-to-end.
- **Do not ship tract int8.** If we need an 11 MB pack, the fallback is `ort`
  (or a distilled fp32 student), not dynamic int8 under tract.
- **The size lever is distillation, not quantization.** A 6-layer student
  (~6.4M params, ~23 MB fp32) is the next compact-pack target; it halves the
  parameter count and should land near 11 ms/sentence while keeping the compact
  tier's accuracy.

## Targets for the compact pack

- POS: beat Shekar on PerDT (97.4%) and Hazm on Seraji (90.7%) with one model.
- NER: ≥ 85 F1 on PEYMA test, trained on the PEYMA/Arman train splits, at
  ≤ 30 ms/sentence.
- Ezafe: beat the rule-based baseline by 5 F1 points.
- Pack: ≤ 50 MB fp32, load ≤ 1 s.

## Rishe compact pack candidate (0.5.0)

Trained from `HooshvareLab/albert-fa-zwnj-base-v2` on one GPU (Python 3.13,
torch 2.11.0+cu130, transformers 5.20.2). The model files are published at
[`MahdiAlikhah/rishe-models`](https://huggingface.co/MahdiAlikhah/rishe-models).
The numbers below come from the first training run.

| task | dataset | Rishe | Shekar | Hazm |
|---|---|---|---|---|
| POS micro F1 | UD-Seraji | **94.89%** | 82.50% | 90.66% |
| POS micro F1 | UD-PerDT | 96.80% | **97.39%** | 84.90% |
| NER exact-span F1 | PEYMA test | **94.05%** | 57.14% | N/A |
| NER entity F1 | PEYMA+Arman mixed | **92.48%** | N/A | N/A |
| Ezafe F1 | HomoRich held-out | **94.86%** | N/A | N/A |

- One POS model now leads on Seraji by 12.4 points and is 0.59 behind Shekar on
  PerDT; Hazm is behind on both. The combined model is the better generalist.
- NER is the decisive win: 94.05 exact-span F1 on PEYMA against Shekar's 57.14,
  at a fraction of its latency.
- Ezafe reaches 94.86 F1 / 98.55% accuracy on a held-out HomoRich split.

Full-model runtime, CPU, single thread, after tract prepare (~0.24 s), raw
tensor input:

| task | tokens | tract | onnxruntime | parity |
|---|---|---|---|---|
| POS | 11 | 22.1 ms | 14.9 ms | argmax 100%, max Δ 7.2e-6 |
| NER | 25 | 49.1 ms | 35.0 ms | argmax 100%, max Δ 1.1e-5 |
| Ezafe | 13 | 27.6 ms | 19.1 ms | argmax 100%, max Δ 5.7e-6 |

### End-to-end `ModelPack` latency (0.5.0)

Python API, pre-tokenized words, single thread, pack loaded once, first three
calls warmed up. This includes pack tokenization, ONNX inference and argmax,
but not Rishe's word tokenizer (use `tag(text)` for that path). Medians are
over 200 sentences:

| task | dataset | sentences | words | load s | median ms | p95 ms | words/s |
|---|---|---|---|---|---|---|---|
| POS | UD-PerDT | 200 | 3097 | 0.26 | 41.88 | 83.53 | 336 |
| NER | PEYMA test | 200 | 5884 | 0.29 | 78.88 | 155.03 | 368 |
| Ezafe | UD-PerDT (latency only) | 200 | 3097 | 0.30 | 41.71 | 81.04 | 340 |

The larger NER numbers track sentence length: the PEYMA sample averages 29.4
words per sentence against 15.5 for the PerDT sample. Reproduce with
`python scripts/benchmark_modelpack.py`; set `RISHE_PACK_ROOT` to a local
`compact` directory or use the packs already in the Rishe Hub cache.

All three ONNX files load in tract unchanged (opset 14, batch 1, dynamic
sequence). Manifest checksums match, and the Rust tokenizer produces the same
IDs as the Python tokenizer on the packed `tokenizer.json`.

**Packaging: three optional per-task packs.** Each model is 44.8 MB, so a user
who needs POS pulls 44.8 MB, not the 134.5 MB sum. The 134.5 MB is only paid by
someone who pulls all three. This matches the Hub design: `pos-compact`,
`ner-compact` and `ezafe-compact` are separate registry entries with their own
manifests and checksums.

A shared-backbone pack would save ~90 MB for the all-three user, but it needs
another training run and risks the measured per-task accuracy. It is deferred
until there is actual demand for an all-in-one download. The per-task 44.8 MB
is the number to compare with Shekar's ~11 MB int8 models; the trade is
precision (fp32) and accuracy, not install size.

## Dependency parsing (0.8.0)

The compact parser is an ALBERT backbone with a biaffine arc scorer and a
head-conditioned relation classifier, decoded with single-root MST. It is
48.1 MB and runs through the same tract pack runtime. Full card:
[`rishe-depparse-compact`](MODEL_CARDS.md#rishe-depparse-compact).

| test treebank | UAS | LAS | LAS no punct |
|---|---:|---:|---:|
| UD-Seraji | 85.38 | 77.99 | 78.18 |
| UD-PerDT | 91.84 | 87.31 | 87.33 |

Across seeds 42, 43 and 44, mean ± sample std LAS is 76.70 ± 1.18 on Seraji and
87.71 ± 0.43 on PerDT. The released pack is seed 42. These are compact-model
numbers: below BERT-scale Hazm/spaCy `trf`, above spaCy's compact `sm` on PerDT.

## Static embeddings (0.10.0)

The first-party embedding pack is Word2Vec skip-gram over Persian Wikipedia.
`rishe-embed-fa-300-v2` is the current default: 68,144 words from 150,000
articles, published both as a word2vec text `.vec` file and as a compact `f16`
binary `.rvec` file. It loads directly into `rishe.Embeddings`; there is no ONNX
or Python ML stack involved. Full cards:
[`rishe-embed-fa-300-v2`](MODEL_CARDS.md#rishe-embed-fa-300-v2) and
[`rishe-embed-fa-300`](MODEL_CARDS.md#rishe-embed-fa-300) for the previous pack.

Same eight hand-picked pairs; the random baseline samples each model's own
vocabulary (100 pairs, seed 42).

| model | size | vocab | positive mean | random mean | gap | `most_similar` |
|---|---:|---:|---:|---:|---:|---:|
| **Rishe `rishe-embed-fa-300-v2`** | **42 MB** | **68,144** | **0.4740** | **0.1393** | **0.3347** | **11.7 ms** |
| Rishe `rishe-embed-fa-300` (0.9.x) | 135 MB | 38,864 | 0.4808 | 0.1558 | 0.3250 | 5.5 ms |
| Rishe `rishe-embed-fa-300-135k` | 475 MB | 135,513 | 0.5055 | 0.2553 | 0.2503 | 23.5 ms |
| Hazm `hazm-fasttext-300` | 2.33 GB | 884,790 | 0.5565 | 0.2057 | 0.3509 | 152 ms |

v2 doubles the vocabulary of the 0.9.x pack and lifts ParsiNLU-entailment token
coverage from 82.6% to 97.2%, with macro F1 from 0.2438 to 0.2985 (see its model
card). Hazm keeps the best own-vocabulary gap and 1.3 points more coverage, at
55× the download and ~13× the query time; on the downstream task v2 scores
higher (0.2985 vs 0.2602 macro F1). The 135k variant is a coverage experiment,
not the default: it raises the related-pair mean but raises the random baseline
more. Scored on one shared word list, v2 and the 0.9.x pack separate the same
way (0.3526 vs 0.3434 over 4,403 words), and the earlier three-way comparison on
2,955 shared words gave 0.3334 / 0.3202 / 0.3145 for v1 / 135k / Hazm. These are
intrinsic sanity numbers, not a gold Persian WordSim/SimLex score.

Sizes are the downloaded file. v2 is published as a 236 MB word2vec text file
and as a 42 MB `f16` binary holding the same matrix; the hub pulls the binary,
which also loads about 2.4× faster (236 ms versus 575 ms for 68,144 × 300 on the
same host). The `most_similar` column times one query with the model already
loaded, single thread, on the host that produced each row: 11.7 ms is on the
0.10.0 host, where the 0.9.x pack answers the same query in 6.7 ms. Reproduce
with `python scripts/compare_embeddings.py` after
`rishe hub pull rishe-embed-fa-300-v2`.

## Contextual encoders (zero-shot, 0.10.0)

The same ParsiNLU-entailment protocol scores contextual encoders far above any
static model, at two to three orders of magnitude more memory. They are listed
here as the reference point for future sentence-embedding packs; Rishe does not
bundle them, and each needs its own runtime.

| model | parameters | dim | weights | pooling | macro F1 |
|---|---:|---:|---:|---|---:|
| Rishe `rishe-embed-fa-300-v2` (static, for scale) | 20.4M values | 300 | 42 MB | mean | 0.2985 |
| E5-small | 118M | 384 | 471 MB | `query: ` prefix | 0.3492 |
| EmbeddingGemma 2 | 740M total (270M text) | 768 | 1,489 MB | `SentenceSimilarity` / STS | 0.3721 |
| EmbeddingGemma 2 | 740M total (270M text) | 768 | 1,489 MB | `Classification` | 0.3601 |
| BGE-M3 | 568M | 1,024 | 2,271 MB | `query: ` prefix | 0.3853 |

EmbeddingGemma 2's weights include vision and audio towers; a text-only pack
would be substantially smaller than 1,489 MB. Prompt choice matters: the
`Classification` model card prompt loses ~0.012 macro F1 against the
`SentenceSimilarity` one on this task.
