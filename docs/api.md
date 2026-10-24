# API reference

The Python API is defined in [`python/rishe/_core.pyi`](https://github.com/MahdiAlikhah/rishe/blob/main/python/rishe/_core.pyi)
and the Rust API is documented on [docs.rs](https://docs.rs/rishe).

Main Python objects:

- `Pipeline` — normalization, tokenization, lemmatization, morphology and sentence splitting
- `ProcessedDoc` — the result of `pipe(text)`
- `Embeddings` — static word and sentence vectors
- `rishe.hub` — explicit model download and cache
- `rishe.corpora` — CoNLL-U, PEYMA and plain-text readers

```python
from rishe import Embeddings, Pipeline, hub

pipe = Pipeline()
doc = pipe("کتاب‌های خوب را به دوستانم می‌دهم.")
print(doc.tokens, doc.lemmas)

model = Embeddings(hub.path("hazm-fasttext-300"))
print(model.sentence_vector("این کتاب خوب است"))
```
