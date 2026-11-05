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

<p align="center">
  <a href="README.fa.md">فارسی</a> ·
  <a href="README.pypi.md">Python</a> ·
  <a href="README.crates.md">Rust</a>
</p>

Rishe turns raw Persian text into clean, structured data: normalized text,
tokens, sentences, lemmas, ezafe markers, stems, verb forms, spell suggestions
and optional POS/NER tags or dependency trees. One engine, prebuilt Python
wheels and a Rust crate.

## Choose your entry point

- **Python** — `pip install rishe` (Python 3.9+, no Python dependencies).
  Read [README.pypi.md](README.pypi.md) or the [PyPI page](https://pypi.org/project/rishe/).
- **Rust** — `cargo add rishe` (Rust 1.75+; model packs need 1.91), or
  `cargo install rishe` for the CLI. Read [README.crates.md](README.crates.md)
  or the [crates.io page](https://crates.io/crates/rishe).

## Quick start

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون گفت فردا می‌ره کتابخونه و یه کتاب خوب می‌خونه.")

print(doc.tokens)
# ['آن', 'گفت', 'فردا', 'می‌رود', 'کتابخانه', 'و', 'یک', 'کتاب', 'خوب', 'می‌خواند', '.']
print(doc.lemmas)
# ['آن', 'گفت', 'فردا', 'رفت', 'کتابخانه', 'و', 'یک', 'کتاب', 'خوب', 'خواند', '.']
print(doc.formal_text)
# 'آن گفت فردا می‌رود کتابخانه و یک کتاب خوب می‌خواند.'
```

More Python usage is in [README.pypi.md](README.pypi.md); the Rust API is in
[README.crates.md](README.crates.md). Persian readers can start with
[README.fa.md](README.fa.md).

## What it does

- Normalization, word/sentence tokenization, lemmatization, stemming, verb
  conjugation and inflection.
- Colloquial → formal rewriting, clitic splitting, spell correction, stopword
  removal, ezafe detection and Jalali date parsing.
- Static fastText/word2vec embeddings, loaded and pooled in-process.
- Optional POS, NER, learned ezafe and dependency-parsing packs, downloaded
  separately and run without ONNX Runtime or a Python ML stack.

## Embeddings

```bash
rishe hub pull rishe-embed-fa-300   # 135 MB, checksum-verified
```

```python
from rishe import Embeddings, hub

vectors = Embeddings(hub.path("rishe-embed-fa-300"))
vectors.most_similar("کتاب", 5)
vectors.sentence_similarity("این کتاب خوب است", "کتاب خوبی بود")
```

`rishe-embed-fa-300` is a 300-dimensional, 38,864-word Word2Vec model trained on
Persian Wikipedia. The vectors are CC BY-SA 4.0; see [NOTICE.md](NOTICE.md).

## Architecture

- **Rust core** — normalization, tokenization, morphology, embedding math and
  the optional pack runtime.
- **Python interface** — abi3 wheels over the same core; no Python runtime
  dependencies.
- **Optional model packs** — POS, NER, ezafe and dependency parsing are
  separate downloads; the base install stays model-free.

## Performance & benchmarks

The core, feature and model-pack benchmarks live in the docs:

[Core engine](docs/BENCHMARKS.md) · [Feature comparisons](docs/FEATURE_BENCHMARKS.md) · [Model packs](docs/MODEL_BENCHMARKS.md)

## Docs

- [API](docs/api.md) · [CLI](docs/cli.md)

## License

Dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
Lexicon attributions are in [NOTICE.md](NOTICE.md).

The code license does not cover downloaded model weights: the
`rishe-embed-fa-300` vectors are released under CC BY-SA 4.0, and each optional
task pack carries its own terms. See [NOTICE.md](NOTICE.md).

Built by Mahdi Alikhah.
