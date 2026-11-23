# API reference

The Python API is defined in [`python/rishe/_core.pyi`](https://github.com/MahdiAlikhah/rishe/blob/main/python/rishe/_core.pyi)
and the Rust API is documented on [docs.rs](https://docs.rs/rishe).

Main Python objects:

- `Pipeline` — normalization, tokenization, lemmatization, morphology and sentence splitting
- `ProcessedDoc` — the result of `pipe(text)`
- `JalaliDate` / `parse_date` — Jalali parsing, formatting and Gregorian conversion
- `Embeddings` — static word and sentence vectors
- `ModelPack` — an ONNX task or sentence pack from `rishe.hub`
- `Dependency` — one dependency arc from `Pipeline.depparse`
- `rishe.__version__` — the crate version, baked in at build time
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

It does take keyword options, and they change the tokens rather than the text:
`replace_links=True` collapses a URL to one `LINK` token, `replace_numbers=True`
collapses a number to `NUM3`, and `join_verb_parts=True` joins a verb and its
auxiliary into one token (`رفته است` → `رفته_است`). All of them are off by
default, so the un-argumented call above is the plain surface split.

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
hub.pull("rishe-vector-e5-small-fa")  # 129 MB sentence encoder
hub.pull("rishe-rerank-mmarco-minilmv2")  # 130 MB cross-encoder reranker
hub.path("pos-compact")               # cached directory
hub.cached("rishe-embed-fa-300-v2")   # cached file, or None
hub.verify("pos-compact")             # checksum check
```

The embedding model is a separate download (CC BY-SA 4.0). The base install does
not bundle weights and importing `rishe` never touches the network.

`rishe-vector-e5-small-fa` is a sentence encoder, not a tagger. `SentenceEmbedder`
wraps it and refuses a pack that is not a sentence encoder, so a POS pack cannot
be pooled into a vector that looks like an embedding but means nothing:

```python
from rishe import SentenceEmbedder, hub

encoder = SentenceEmbedder(hub.path("rishe-vector-e5-small-fa"))
encoder.embed("این کتاب بسیار خوب است")        # 384 floats, L2-normalized
encoder.embed_batch(["متن یک", "متن دو"])      # one vector per text, in order
encoder.similarity("کتاب خوب", "کتاب بد")      # cosine, a plain dot product
encoder.rank("کتاب", candidates)               # [(score, text)] best first
```

`ModelPack` still works directly when you already have one; the wrapper adds the
task check and the cosine helpers.

`rishe-rerank-mmarco-minilmv2` is a cross-encoder reranker (Apache 2.0). It
scores a `(query, passage)` pair with `score_pairs(query, passage)` and returns
one logit — higher is more relevant. `score_pairs_batch(pairs)` does many at once
and keeps input order. It cannot produce vectors, so use it to reorder candidates
the encoder already fetched:

```python
from rishe import ModelPack, hub

encoder = SentenceEmbedder(hub.path("rishe-vector-e5-small-fa"))
reranker = ModelPack(hub.path("rishe-rerank-mmarco-minilmv2"))

vectors = encoder.embed_batch(candidates)
# ... rank by dot product, keep the top N ...
shortlist = candidates[:50]
order = sorted(shortlist, key=lambda doc: -reranker.score_pairs(query, doc))
```

Both downloads are optional; nothing above is needed for tagging.
