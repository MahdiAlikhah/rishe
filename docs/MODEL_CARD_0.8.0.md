# rishe-depparse-compact

A compact Persian dependency parser for Rishe 0.8.0. It predicts a single-root
dependency tree and Universal Dependencies relations over whitespace-tokenized
Persian text.

This is a small, fast pack, not a state-of-the-art parser. It is meant to make
syntax available in the zero-dependency Rishe install once the optional pack is
downloaded.

## Model

| | |
|---|---|
| Task | Dependency parsing (UAS/LAS) |
| Backbone | [`HooshvareLab/albert-fa-zwnj-base-v2`](https://huggingface.co/HooshvareLab/albert-fa-zwnj-base-v2) |
| Parameters | 12.55M |
| ONNX size | 48.1 MB |
| Relations | 45 Universal Dependencies labels |
| Sequence length | Fixed 192 subword positions; runtime pads |
| Word axis | Dynamic |
| Decoding | Single-root maximum spanning arborescence (Chu–Liu/Edmonds) |
| Relation scoring | Head-conditioned biaffine relation classifier |
| Version | 0.8.0 |

The ONNX graph takes `input_ids`, `attention_mask`, `word_positions`,
`word_mask`, and `head_indices`. It returns `arc_scores` for every
word/head pair and `relation_logits` for the selected head. The runtime runs
the arc head, decodes one tree, then runs the relation head with the decoded
heads.

## Training

I trained the pack on the Universal Dependencies Persian treebanks:

- UD Persian Seraji
- UD Persian PerDT

The recipe is 10 epochs, batch size 16, AdamW with linear warmup and decay,
seed 42, and the best epoch by development LAS. The released checkpoint was
selected at epoch 8.

## Evaluation

Single-root MST, test split, punctuation included:

| Test treebank | UAS | LAS | LAS (no punct) | Cyclic trees |
|---|---:|---:|---:|---:|
| UD-Seraji | 85.38 | 77.99 | 78.18 | 0 / 600 |
| UD-PerDT | 91.84 | 87.31 | 87.33 | 0 / 1455 |

The released checkpoint is seed 42. Two additional seeds give the following
spread (same recipe, test split, MST):

| Seed | Seraji UAS | Seraji LAS | PerDT UAS | PerDT LAS |
|---|---:|---:|---:|---:|
| 42 (released) | 85.38 | 77.99 | 91.84 | 87.31 |
| 43 | 84.56 | 76.44 | 91.92 | 87.67 |
| 44 | 84.32 | 75.68 | 92.28 | 88.17 |
| **Mean ± std** | **84.75 ± 0.56** | **76.70 ± 1.18** | **92.01 ± 0.24** | **87.71 ± 0.43** |

Seed 42 has the best mean LAS across the two treebanks and is the released
pack. The other seeds are a robustness check, not additional released models.

The parser is compact and beats spaCy's compact `md`/`lg` Persian models on
UD-PerDT in this setup, but it remains below BERT-scale Hazm and spaCy `trf`.
Use it when install size and CPU speed matter more than the last few LAS
points.

Indicative PerDT test comparison (not an identical harness; tokenization and
punctuation handling differ):

| Model | Size | UAS | LAS |
|---|---:|---:|---:|
| spaCy `sm` | 13.5 MB | 89.69 | 85.15 |
| **Rishe depparse** | **48.1 MB** | **91.84** | **87.31** |
| spaCy `md` | 62.6 MB | 90.52 | 86.34 |
| spaCy `lg` | 235.2 MB | 90.96 | 86.60 |
| Hazm BERT parser | large | 92.46 | 89.34 |
| spaCy `trf` | 608.2 MB | 93.87 | 90.79 |

## Usage

```python
from rishe import Pipeline

pipe = Pipeline(depparse_pack="path/to/depparse")
for item in pipe.depparse("کتاب‌های خوب را به دوستانم می‌دهم."):
    print(item.text, item.head, item.relation)
```

`head` is a zero-based word index, or `None` for the root. The CLI prints a
tab-separated `word`, `head`, `relation` table:

```bash
rishe depparse --pack path/to/depparse "کتاب‌های خوب را به دوستانم می‌دهم."
```

The Rust API is `Pipeline::depparse` with the `models` feature enabled.

## Limitations

- This is a compact model. It is not a replacement for a BERT-scale parser on
  long or difficult sentences.
- It is trained on UD Persian treebanks and will reflect their annotation
  conventions and domain.
- Input is truncated to 192 subword positions. Longer text should be split
  into sentences.
- Only basic dependencies are predicted; enhanced dependencies, morphology,
  and word senses are out of scope for this pack.
- Tokenization follows Rishe's word tokenizer. Treebank tokens and runtime
  tokens can differ on clitics, punctuation, and spacing.

## License and attribution

The training data is Universal Dependencies Persian, distributed under
**CC BY-SA 4.0**. The derived weights are released under **CC BY-SA 4.0**.
The ALBERT backbone is Apache-2.0. The Rishe runtime code is MIT/Apache-2.0.
The pack-local `LICENSE` and `NOTICE.md` files carry the full terms. Keep the
Universal Dependencies attribution and share-alike terms when redistributing
the pack or derivative weights.

Please cite the UD Persian treebanks and the base model when using this pack.

## Author

Mahdi Alikhah
