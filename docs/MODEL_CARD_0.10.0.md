# `rishe-embed-fa-300-v2`

Static 300-dimensional Persian word vectors for [Rishe](../README.md). This is the
second embedding release and the default pack from Rishe 0.10.0: more corpus,
the same frequency cutoff, better coverage and better separation.

## Model

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
absolute deviation 8.9e-4 over 20.4M values), row cosines stay above
0.9999997, and `Embeddings` accepts either file without extra options.

## Evaluation

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

## Usage

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

## Limitations

- Static vectors: one vector per word type, no context-dependent meaning.
- No subword/fastText fallback. Out-of-vocabulary words are skipped by
  `sentence_vector`, and `vector()` returns `None`.
- Wikipedia domain: colloquial, social-media and specialized technical text is
  under-represented.
- The sentence-level number comes from mean pooling, a weak encoder. Treat it as
  a comparison between packs, not a headline quality claim; contextual encoders
  score far higher on the same task (see
  [`MODEL_BENCHMARKS.md`](MODEL_BENCHMARKS.md)).
- `sentence_vector()` returns an unnormalized mean; `sentence_similarity()`
  normalizes before the cosine.

## License and attribution

The training corpus is Wikipedia, distributed under CC BY-SA and the GFDL. The
derived vectors are released under **CC BY-SA 4.0**; the Rishe code is
MIT/Apache-2.0 and the weights are licensed separately. The pack-local `LICENSE`
holds the full terms and `NOTICE.md` the attribution. Keep the Wikipedia
attribution when redistributing the vectors.

## Author

Mahdi Alikhah
