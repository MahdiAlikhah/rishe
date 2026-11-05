# API reference

The Python API is defined in [`python/rishe/_core.pyi`](https://github.com/MahdiAlikhah/rishe/blob/main/python/rishe/_core.pyi)
and the Rust API is documented on [docs.rs](https://docs.rs/rishe).

Main Python objects:

- `Pipeline` — normalization, tokenization, lemmatization, morphology and sentence splitting
- `ProcessedDoc` — the result of `pipe(text)`
- `JalaliDate` / `parse_date` — Jalali parsing, formatting and Gregorian conversion
- `Embeddings` — static word and sentence vectors
- `Dependency` — one dependency arc from `Pipeline.depparse`
- `rishe.hub` — explicit model download and cache
- `rishe.corpora` — CoNLL-U, PEYMA and plain-text readers

```python
from rishe import Embeddings, Pipeline, hub

pipe = Pipeline()
doc = pipe("کتاب‌های خوب را به دوستانم می‌دهم.")
print(doc.tokens, doc.lemmas)

model = Embeddings(hub.path("rishe-embed-fa-300"))
print(model.dim, len(model))
print(model.similarity("کتاب", "خوب"))
print(model.sentence_similarity("این کتاب خوب است", "کتاب خوبی بود"))
print(model.most_similar("کتاب", 5))
```

The embedding model is a separate download (CC BY-SA 4.0). The base install does
not bundle weights and importing `rishe` never touches the network.
