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
