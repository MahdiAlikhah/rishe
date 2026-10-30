<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="Rishe: fast Persian NLP and morphology" width="100%">
</p>

# Rishe (ریشه)

**Fast Persian NLP and morphology, built in Rust.**

<p align="center">
  <a href="https://pepy.tech/projects/rishe">
    <img src="https://static.pepy.tech/personalized-badge/rishe?period=total&units=INTERNATIONAL_SYSTEM&left_color=BLACK&right_color=GREEN&left_text=downloads" alt="PyPI Downloads">
  </a>
</p>

Rishe turns raw Persian text into clean, structured data: normalized text,
tokens, sentences, lemmas, ezafe markers, stems, verb forms, spell suggestions
and optional POS/NER tags. One engine, prebuilt Python wheels and a Rust crate.

## Choose your entry point

- **Python** — `pip install rishe` (Python 3.9+, no Python dependencies).
  Read [README.pypi.md](README.pypi.md) or the [PyPI page](https://pypi.org/project/rishe/).
- **Rust** — `cargo add rishe` (Rust 1.75+; model packs need 1.91).
  Read [README.crates.md](README.crates.md) or the [crates.io page](https://crates.io/crates/rishe).

## Quick start

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")

print(doc.tokens)
print(doc.lemmas)
print(doc.formal_text)
```

More Python usage is in [README.pypi.md](README.pypi.md); the Rust API is in
[README.crates.md](README.crates.md).

## What it does

- Normalization, word/sentence tokenization, lemmatization, stemming, verb
  conjugation and inflection.
- Colloquial → formal rewriting, clitic splitting, spell correction, stopword
  removal and ezafe detection.
- Static fastText/word2vec embeddings, loaded and pooled in-process.
- Optional POS, NER and learned ezafe packs, downloaded separately and run
  without ONNX Runtime or a Python ML stack.

## Architecture

- **Rust core** — normalization, tokenization, morphology, embedding math and
  the optional pack runtime.
- **Python interface** — abi3 wheels over the same core; no Python runtime
  dependencies.
- **Optional model packs** — POS, NER and ezafe are separate downloads; the base
  install stays model-free.

## Performance & benchmarks

The core, feature and model-pack benchmarks live in the docs:

[Core engine](docs/BENCHMARKS.md) · [Feature comparisons](docs/FEATURE_BENCHMARKS.md) · [Model packs](docs/MODEL_BENCHMARKS.md)

## Docs

- [API](docs/api.md) · [CLI](docs/cli.md)

## License

Dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
Lexicon attributions are in [NOTICE.md](NOTICE.md).

Built by Mahdi Alikhah.
