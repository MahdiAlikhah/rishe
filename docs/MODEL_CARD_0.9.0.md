# rishe-embed-fa-300

Static 300-dimensional Persian word embeddings for Rishe 0.9.0. Sentence and
document vectors are the mean of in-vocabulary token vectors, computed by the
runtime; there is no separate sentence encoder.

Superseded by [`rishe-embed-fa-300-v2`](MODEL_CARD_0.10.0.md) as the default pack
in Rishe 0.10.0; this 30,000-article pack stays published for anyone pinning it.

This is a compact, qualitatively evaluated model, not a state-of-the-art
contextual embedding. It exists so the zero-dependency Rishe install has a
first-party Persian embedding option that is 17× smaller than the usual
Hazm/fastText comparison file and loads in well under a second.

## Model

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
| Version | 0.9.0 |

The vocabulary is an FST mapping word → row index; the vectors are one flat
row-major `f32` buffer. Lookups are O(word length) and the model is shared
read-only across threads.

## Training

The corpus is streamed and tokenized with Rishe's own pipeline
(`Pipeline.sent_tokenize` then `Pipeline.word_tokenize`), so the training tokens
match the runtime tokenizer. The recipe is the released profile: 30,000
articles, `min_count=20`, `max_final_vocab=100_000`, 10 epochs, 4 workers,
seed 42. Training ran on a CPU host in 395.5 s.

## Evaluation

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

## Usage

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

## Limitations

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

## License and attribution

The training corpus is Wikipedia, distributed under CC BY-SA and the GFDL. The
derived vectors are released under **CC BY-SA 4.0**; the Rishe code is
MIT/Apache-2.0 and the weights are licensed separately. The pack-local
`LICENSE` holds the full terms and `NOTICE.md` the attribution. Keep the
Wikipedia attribution when redistributing the vectors.

## Author

Mahdi Alikhah
