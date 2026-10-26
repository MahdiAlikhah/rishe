<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="Rishe: fast Persian NLP and morphology" width="100%">
</p>

# Rishe (ریشه)

**Fast Persian NLP and morphology, built in Rust.**

Rishe turns raw Persian text into clean, structured data: normalized text,
tokens, sentences, lemmas, ezafe markers, stems, verb forms, spell suggestions
and optional POS/NER tags. One engine, prebuilt Python wheels and a Rust crate.

## Choose your entry point

- **Python** — `pip install rishe` (Python 3.9+, no Python dependencies).
  Read [README.pypi.md](README.pypi.md) or the [PyPI page](https://pypi.org/project/rishe/).
- **Rust** — `cargo add rishe` (Rust 1.75+; model packs need 1.91).
  Read [README.crates.md](README.crates.md) or the [crates.io page](https://crates.io/crates/rishe).

## What it does

- Normalization, word/sentence tokenization, lemmatization, stemming, verb
  conjugation and inflection.
- Colloquial → formal rewriting, clitic splitting, spell correction, stopword
  removal and ezafe detection.
- Static fastText/word2vec embeddings, loaded and pooled in-process.
- Optional POS, NER and learned ezafe packs, downloaded separately and run
  without ONNX Runtime or a Python ML stack.

## Docs

- [API](docs/api.md) · [CLI](docs/cli.md)
- [Core benchmarks](docs/BENCHMARKS.md) · [Feature benchmarks](docs/FEATURE_BENCHMARKS.md) · [Model-pack benchmarks](docs/MODEL_BENCHMARKS.md)

## License

Dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
Lexicon attributions are in [NOTICE.md](NOTICE.md).

Built by Mahdi Alikhah.
