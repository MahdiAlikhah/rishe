# Model cards

Cards for every published Rishe pack. The comparisons between packs, and against
other libraries, are in [MODEL_BENCHMARKS.md](MODEL_BENCHMARKS.md).

| pack | release | task | download | license |
|---|---:|---|---:|---|
| `rishe-embed-fa-300-v2` (default) | 0.10.0 | static 300-d word vectors | 42 MB `.rvec` | CC BY-SA 4.0 |
| `rishe-embed-fa-300` | 0.9.0 | static 300-d word vectors | 135 MB `.vec` | CC BY-SA 4.0 |
| `rishe-depparse-compact` | 0.8.0 | dependency parsing (UAS/LAS) | 48.1 MB ONNX | CC BY-SA 4.0 |

Packs are versioned separately from the crate and are pulled from the Rishe Hub
(`rishe hub pull <name>`). Model weights are not covered by the code license.

## rishe-embed-fa-300-v2

Static 300-dimensional Persian word vectors for [Rishe](../README.md). This is
the second embedding release and the default pack from Rishe 0.10.0: more corpus,
the same frequency cutoff, better coverage and better separation.

### Model

| | |
|---|---|
| Task | Static word vectors; mean sentence/document vectors |
| Algorithm | Word2Vec skip-gram with negative sampling |
| Dimension | 300 |
| Vocabulary | 68,144 |
| Corpus | Persian Wikipedia (`wikimedia/wikipedia`, config `20231101.fa`), streamed |
| Articles | 150,000 |
| Sentences | 3,830,930 |
| Tokens | 57,661,561 |
| Window | 5 |
| Minimum count | 20 |
| Max final vocabulary | 200,000 |
| Epochs | 15 |
| Workers | 16 |
| Seed | 42 |
| Text file (`f32`) | `rishe-embed-fa-300-v2.vec` — 236,068,629 bytes |
| Binary file (`f16`) | `rishe-embed-fa-300-v2.rvec` — 41,849,368 bytes |
| Release | Rishe 0.10.0 |

Both files hold the same 68,144 × 300 matrix. The `.vec` file is plain word2vec
text and works with any tool. The `.rvec` file is Rishe's compact binary format
(version 1): an `RVEC` header records the version, storage width, dimension and
vocabulary size, then a length-prefixed vocabulary and the payload. It stores
each value as IEEE 754 binary16 and decodes to `f32` on load, so the only
difference is storage: a decoded value moves by at most 4.9e-4 relative (max
absolute deviation 8.9e-4 over 20.4M values), row cosines stay above 0.9999997,
and `Embeddings` accepts either file without extra options.

### Evaluation

Eight hand-picked pairs with an own-vocabulary random baseline (100 pairs, seed
42), and ParsiNLU entailment with mean sentence vectors plus logistic regression
on the test split (1,673 pairs; majority accuracy 0.3646).

Coverage is the share of ParsiNLU tokens that fall inside the model's
vocabulary; it is the main improvement in this release.

| model | vocab | positive | random | gap | accuracy | macro F1 | coverage |
|---|---:|---:|---:|---:|---:|---:|---:|
| `rishe-embed-fa-300` (0.9.0) | 38,864 | 0.4808 | 0.1558 | 0.3250 | 0.3796 | 0.2438 | 82.56% |
| **`rishe-embed-fa-300-v2`** | **68,144** | **0.4740** | **0.1393** | **0.3347** | **0.4071** | **0.2985** | **97.23%** |

The 150k-article configuration was chosen over 500k, a lower `min_count` and a
subword (fastText) model: 500k adds +0.009 gap and +0.8 points of coverage with
no macro-F1 gain, `min_count=10` trades 0.031 gap for 0.7 points of coverage,
and fastText needs 15 epochs before its intrinsic gap stops trailing (0.3138 at
5 epochs, 0.3554 at 15) while its downstream macro F1 stays flat at 0.2867.

ParsiNLU is CC BY-NC-SA 4.0 and is used for evaluation only; no dataset text is
included in this pack.

### Usage

```bash
rishe hub pull rishe-embed-fa-300-v2
```

```python
from rishe import Embeddings, hub

vectors = Embeddings(hub.path("rishe-embed-fa-300-v2"))
print(vectors.dim, len(vectors), vectors.dtype)   # 300 68144 f16
print(vectors.similarity("کتاب", "خوب"))
print(vectors.most_similar("کتاب", 5))
print(vectors.sentence_similarity("این کتاب خوب است", "کتاب خوبی بود"))
```

`rishe similar` and `rishe similarity` expose the same lookups from the command
line. `rishe convert` rewrites any word2vec file in the compact format:

```bash
rishe convert my-vectors.vec my-vectors.rvec            # f16
rishe convert my-vectors.vec my-vectors.rvec --dtype f32
```

### Limitations

- Static vectors: one vector per word type, no context-dependent meaning.
- No subword/fastText fallback. Out-of-vocabulary words are skipped by
  `sentence_vector`, and `vector()` returns `None`.
- Wikipedia domain: colloquial, social-media and specialized technical text is
  under-represented.
- The sentence-level number comes from mean pooling, a weak encoder. Treat it as
  a comparison between packs, not a headline quality claim; contextual encoders
  score far higher on the same task (see [MODEL_BENCHMARKS.md](MODEL_BENCHMARKS.md)).
- `sentence_vector()` returns an unnormalized mean; `sentence_similarity()`
  normalizes before the cosine.

### License and attribution

The training corpus is Wikipedia, distributed under CC BY-SA and the GFDL. The
derived vectors are released under **CC BY-SA 4.0**; the Rishe code is
MIT/Apache-2.0 and the weights are licensed separately. The pack-local `LICENSE`
holds the full terms and `NOTICE.md` the attribution. Keep the Wikipedia
attribution when redistributing the vectors.

## rishe-embed-fa-300

Static 300-dimensional Persian word embeddings for Rishe 0.9.0. Sentence and
document vectors are the mean of in-vocabulary token vectors, computed by the
runtime; there is no separate sentence encoder.

Superseded by [`rishe-embed-fa-300-v2`](#rishe-embed-fa-300-v2) as the default
pack in Rishe 0.10.0; this 30,000-article pack stays published for anyone pinning
it.

This is a compact, qualitatively evaluated model, not a state-of-the-art
contextual embedding. It exists so the zero-dependency Rishe install has a
first-party Persian embedding option that is 17× smaller than the 2.33 GB
fastText comparison file and loads in well under a second.

### Model

| | |
|---|---|
| Task | Static word vectors; mean sentence/document vectors |
| Algorithm | Word2Vec skip-gram with negative sampling |
| Dimension | 300 |
| Vocabulary | 38,864 |
| Corpus | Persian Wikipedia (`wikimedia/wikipedia`, `20231101.fa`), streamed |
| Articles | 30,000 |
| Sentences | 1,109,472 |
| Tokens | 20,731,195 |
| Window | 5 |
| Minimum count | 20 |
| Max final vocab | 100,000 |
| Epochs | 10 |
| Workers | 4 |
| Seed | 42 |
| Vector file | 135,072,816 bytes |
| Zip | 56,287,225 bytes |
| Vector SHA-256 | `afed6aa266e07745ce7b6b1e2f6600f6f0373dc91e197902d7305e19d2d35bf5` |
| Zip SHA-256 | `2ce8f375f30aad313189300ec071fadc15835185be38d51ba4412f9f814c68d7` |
| License | CC BY-SA 4.0 (`LICENSE`) |
| Release | Rishe 0.9.0 |

The vocabulary is an FST mapping word → row index; the vectors are one flat
row-major `f32` buffer. Lookups are O(word length) and the model is shared
read-only across threads.

### Training

The corpus is streamed and tokenized with Rishe's own pipeline, so the training
tokens match the runtime tokenizer. The recipe is the released profile: 30,000
articles, `min_count=20`, `max_final_vocab=100_000`, 10 epochs, 4 workers,
seed 42. Training ran on a CPU host in 395.5 s.

### Evaluation

Hand-picked related pairs against random vocabulary pairs (cosine similarity).
This is a sanity benchmark, not WordSim-353/SimLex for Persian.

| measure | related pairs | random pairs |
|---|---:|---:|
| mean | 0.4808 | 0.1558 |
| median | 0.5395 | 0.1542 |

Runtime through `rishe.Embeddings`, loaded once:

| measure | value |
|---|---:|
| load | 0.34 s |
| `most_similar` | 5.5 ms/call |
| `sentence_vector` | 5.77 µs/call |
| identical-sentence similarity | 1.0 |

Comparisons use the same eight pairs; the random baseline samples each model's
own vocabulary (100 pairs, seed 42):

| model | size | vocab | positive mean | random mean | gap | `most_similar` |
|---|---:|---:|---:|---:|---:|---:|
| **Rishe `rishe-embed-fa-300`** | **135 MB** | **38,864** | **0.4808** | **0.1558** | **0.3250** | **5.5 ms** |
| Rishe `rishe-embed-fa-300-135k` | 475 MB | 135,513 | 0.5055 | 0.2553 | 0.2503 | 23.5 ms |
| Hazm `hazm-fasttext-300` | 2.33 GB | 884,790 | 0.5565 | 0.2057 | 0.3509 | 152 ms |

Hazm has the strongest raw pair similarities and the best own-vocabulary gap,
but it is ~17× larger and its linear `most_similar` scan is ~28× slower. The
135k Rishe variant raises the related-pair mean but raises the random baseline
more, so its gap is 0.075 lower than the 30k model. It is a coverage variant,
not the recommended release.

On a shared 2,955-word baseline the gaps are 0.3334 / 0.3202 / 0.3145. Eight
hand-picked pairs cannot separate models at that resolution, so treat the
ranking as indicative. Reproduce with `python scripts/compare_embeddings.py`.

### Usage

```bash
rishe hub pull rishe-embed-fa-300
```

```python
from rishe import Embeddings, hub

vectors = Embeddings(hub.path("rishe-embed-fa-300"))
print(vectors.dim, len(vectors))          # 300 38864
print(vectors.similarity("کتاب", "خوب"))
print(vectors.most_similar("کتاب", 5))
print(vectors.sentence_similarity("این کتاب خوب است", "کتاب خوبی بود"))
```

`rishe similar` exposes the same lookup from the command line.

### Limitations

- Static vectors: one vector per word type, no context-dependent meaning.
- No subword/fastText fallback. Out-of-vocabulary words are skipped by
  `sentence_vector`, and `vector()` returns `None`.
- Wikipedia domain. Colloquial, social-media and specialized technical text is
  under-represented.
- The evaluation is an intrinsic sanity check on hand-picked pairs. There is no
  gold Persian similarity/analogy score in this release, so do not quote one.
- `sentence_vector()` returns an unnormalized mean; `sentence_similarity()`
  normalizes before the cosine. Sentence similarity below 1.0 against another
  library is usually tokenization, not embedding arithmetic.

### License and attribution

The training corpus is Wikipedia, distributed under CC BY-SA and the GFDL. The
derived vectors are released under **CC BY-SA 4.0**; the Rishe code is
MIT/Apache-2.0 and the weights are licensed separately. The pack-local
`LICENSE` holds the full terms and `NOTICE.md` the attribution. Keep the
Wikipedia attribution when redistributing the vectors.

## rishe-depparse-compact

A compact Persian dependency parser for Rishe 0.8.0. It predicts a single-root
dependency tree and Universal Dependencies relations over whitespace-tokenized
Persian text.

This is a small, fast pack, not a state-of-the-art parser. It is meant to make
syntax available in the zero-dependency Rishe install once the optional pack is
downloaded.

### Model

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
| Release | Rishe 0.8.0 |

The ONNX graph takes `input_ids`, `attention_mask`, `word_positions`,
`word_mask`, and `head_indices`. It returns `arc_scores` for every
word/head pair and `relation_logits` for the selected head. The runtime runs
the arc head, decodes one tree, then runs the relation head with the decoded
heads.

### Training

I trained the pack on the Universal Dependencies Persian treebanks:

- UD Persian Seraji
- UD Persian PerDT

The recipe is 10 epochs, batch size 16, AdamW with linear warmup and decay,
seed 42, and the best epoch by development LAS. The released checkpoint was
selected at epoch 8.

### Evaluation

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

### Usage

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

### Limitations

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

### License and attribution

The training data is Universal Dependencies Persian, distributed under
**CC BY-SA 4.0**. The derived weights are released under **CC BY-SA 4.0**.
The ALBERT backbone is Apache-2.0. The Rishe runtime code is MIT/Apache-2.0.
The pack-local `LICENSE` and `NOTICE.md` files carry the full terms. Keep the
Universal Dependencies attribution and share-alike terms when redistributing
the pack or derivative weights.

Please cite the UD Persian treebanks and the base model when using this pack.

## Author

All packs are trained, evaluated and published by Mahdi Alikhah.
