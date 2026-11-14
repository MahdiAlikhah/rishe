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

model = Embeddings(hub.path("rishe-embed-fa-300-v2"))
print(model.dim, len(model), model.dtype)
print(model.similarity("کتاب", "خوب"))
print(model.sentence_similarity("این کتاب خوب است", "کتاب خوبی بود"))
print(model.most_similar("کتاب", 5))
```

### Embedding file formats

`Embeddings(path)` accepts a word2vec text `.vec` file, a fastText `.vec` file
or Rishe's compact binary `.rvec` file; both stored widths (`f32`, `f16`) are
detected from the file header and decoded into the same row-major `f32` buffer,
so the storage width affects only the file on disk. `dtype` reports which width
the file used. On the Rust side the same information comes from
`Embeddings::dtype()`, and `Embeddings::save(path, DType::F16)` writes the
compact format.

### Module-level functions

The common operations do not need a `Pipeline` object: each module-level
function uses one shared default pipeline, created on first use.

```python
import rishe

rishe.normalize("مي كنم")                    # 'می‌کنم'
rishe.tokenize("سلام دنیا")                   # ['سلام', 'دنیا']
rishe.tokenize_spans("سلام دنیا")             # character offsets: [('سلام', 0, 4), ...]
rishe.sent_tokenize(text)                     # sentence list
rishe.lemmatize("می‌روم")                     # 'رفت'
rishe.mask(text, maskers=["url", "email"])    # mask sensitive spans
rishe.keywords(text, n=5)                     # ['کلمه', ...] (RAKE)
rishe.process(text)                           # a ProcessedDoc
rishe.pipeline()                              # the shared default Pipeline
```

`tokenize` does not normalize: it returns surface tokens, exactly as they appear
in the text. Normalization is its own call, and the pipeline runs the sequence:

```python
rishe.tokenize("مي كنم")                  # ['مي', 'كنم']
rishe.tokenize(rishe.normalize("مي كنم"))  # ['می\u200cکنم']
rishe.process("مي كنم").tokens             # ['می\u200cکنم']
```

Tokenization options are keyword arguments (`join_verb_parts=`,
`replace_links=`, `replace_emails=`, …), not separate methods.

The classic class API, and its defaults (such as `word_tokenize` joining verb
parts), lives in `rishe.compat`; see [compat.md](compat.md). The `rishe`-level
names follow the conventions in [`API_CONVENTIONS.md`](API_CONVENTIONS.md).

### Hub

`rishe.hub` uses one verb set for single-file models and task packs:

```python
from rishe import hub

hub.pull("rishe-embed-fa-300-v2")     # 42 MB embedding download
hub.pull("pos-compact")               # task pack download
hub.path("pos-compact")               # cached directory
hub.cached("rishe-embed-fa-300-v2")   # cached file, or None
hub.verify("pos-compact")             # checksum check
```

The embedding model is a separate download (CC BY-SA 4.0). The base install does
not bundle weights and importing `rishe` never touches the network.
