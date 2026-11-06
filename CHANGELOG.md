# Changelog

All notable changes to Rishe are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Model packs are versioned separately from the crate; their versions are recorded
in the pack manifest and on the model repository.

## [Unreleased]

## [0.9.1] - 2026-11-06

### Added

- `CHANGELOG.md` covering the full project history.
- `docs/RELEASING.md`, a version-independent release checklist.
- CC BY-SA 4.0 `LICENSE` files inside the embedding packs, with the manifests
  and zips regenerated so the checksums cover them.
- The model repository README now documents the embedding pack and its license.

### Changed

- The 0.9.0 model card describes the training recipe inline instead of pointing
  at a private training notebook.
- Pack `MODEL_CARD.md` files reference both `LICENSE` and `NOTICE.md`.
- Embedding pack zips are re-issued with the license file; payload `.vec` files
  and their hashes are unchanged.

### Removed

- `docs/PUBLISHING_0.9.0.md` (superseded by `docs/RELEASING.md`).

### Fixed

- Removed a documentation reference to a notebook that is not in the public
  repository.
- Embedding pack checksums no longer omit the license file.

## [0.9.0] - 2026-11-05

### Added

- First-party Persian embeddings, `rishe-embed-fa-300`: Word2Vec skip-gram,
  300 dimensions, 38,864 words, trained on 30,000 Wikipedia articles.
- `Embeddings` class in Rust and Python with word/sentence vectors,
  cosine similarity and nearest neighbours.
- Hub registration for `rishe-embed-fa-300` with a pinned SHA-256, plus
  `rishe hub path`.
- CLI commands `rishe embed`, `rishe similar` and `rishe similarity`, and
  `serve --model` support for them.
- Embedding model card, benchmark tables, `examples/embeddings.py` and
  `scripts/compare_embeddings.py`.
- Real-pack smoke tests gated by `RISHE_EMBEDDINGS_PACK` and
  `RISHE_TEST_EMBEDDINGS_PACK`.

### Changed

- Documentation leads with the first-party model; Hazm's fastText model remains
  available as an external comparison download.
- Docs, READMEs and `NOTICE.md` state the CC BY-SA 4.0 model license separately
  from the MIT/Apache-2.0 code license.

### Fixed

- The 30k embedding pack's `NOTICE.md` named the wrong Wikipedia config.
- The model card no longer claims `sentence_vector()` returns a normalized
  vector; only `sentence_similarity()` normalizes.

## [0.8.1] - 2026-11-04

### Added

- `rishe hub` with `pull`, `list`, `info`, `remove` and `verify`, backed by
  checksum-verified Hugging Face downloads.
- `rishe serve`, a dependency-free JSON-lines server with optional model packs.
- Golden-output benchmark CI (`scripts/benchmark_golden.py`).
- Optional `cuda` and `metal` features via tract's GPU runtimes.
- `--locked` CI, package-specific wheels and a single universal root manifest.

### Changed

- Release matrix no longer builds `aarch64-unknown-linux-gnu`; the
  `aarch64-unknown-linux-musl` wheel remains.
- README and CLI docs cover hub and serve.

### Fixed

- Crate publish is idempotent and checks crates.io before uploading.

## [0.8.0] - 2026-11-03

### Added

- `depparse-compact`, a compact Persian dependency parser trained on UD Persian
  Seraji and PerDT, exported to ONNX.
- `Pipeline.depparse` / `depparse_batch`, `rishe::models::Dependency`, Python
  bindings and CLI `rishe depparse`.
- Single-root Chu-Liu/Edmonds decoding and head-conditioned relation scoring.
- Parser model card, three-seed results and UD CC BY-SA 4.0 attribution.

## [0.7.1] - 2026-11-02

### Added

- Jalali dates exposed through the Python API (`JalaliDate`, `parse_date`).

## [0.7.0] - 2026-11-01

### Added

- `rishe` command-line interface: normalize, tokenize, lemmatize, mask and date,
  with `--json` output and stdin support.
- Jalali date parsing and Gregorian conversion in the Rust core.
- macOS 15 wheels.

### Changed

- Simplified the core, Python bindings and release automation.
- Dropped the docs-site configuration; documentation stays as Markdown.

## [0.6.4] - 2026-10-31

### Added

- Persian README.
- Python CI job alongside the Rust matrix.

## [0.6.3] - 2026-10-30

### Added

- CI model-pack fixture and cache verification.

## [0.6.2] - 2026-10-29

### Changed

- Model-pack developer experience: hub cache handling and clearer CLI errors.

## [0.6.1] - 2026-10-28

### Added

- Pack task validation and batch APIs.

## [0.6.0] - 2026-10-27

### Added

- `Pipeline` model-pack integration: POS, NER and ezafe packs can be attached to
  one pipeline object.

## [0.5.1] - 2026-10-26

### Added

- Package-specific readmes for PyPI (`README.pypi.md`) and crates.io
  (`README.crates.md`).

## [0.5.0] - 2026-10-25

### Added

- ONNX model-pack runtime built on tract, with no ONNX Runtime dependency.
- Compact POS, NER and ezafe packs, `ModelPack` batch APIs and model benchmarks.

### Removed

- mkdocs site configuration.

## [0.4.0] - 2026-10-23

### Added

- Static word and sentence embeddings (`Embeddings`) with cosine similarity,
  sentence pooling and nearest neighbours.
- Explicit model downloads through `rishe.hub`.
- Embedding tests and a Rishe-vs-Hazm embedding benchmark.

## [0.3.0] - 2026-10-22

### Added

- Maskers (URL, email, mention, hashtag, emoji, HTML, digits, punctuation),
  text augmentation, keyword extraction, transliteration and token utilities.
- Python CLI, Hazm-compatible API shims and corpus readers (CoNLL-U, PEYMA,
  plain text).
- API, CLI and Hazm compatibility docs, plus examples.

## [0.2.0] - 2026-10-21

### Added

- Morphology suite: verb conjugation, inflection, spell correction, stemming,
  stopwords and token splitting.
- Stopword and word-frequency data, and feature benchmarks against Hazm.

## [0.1.1] - 2026-10-20

### Added

- Batch-throughput benchmark and a documented limitations section.

### Changed

- README rewritten around benchmark results, with a banner.

### Fixed

- crates.io metadata: five valid keywords and categories.
- Crate publish is idempotent.

## [0.1.0] - 2026-10-20

### Added

- Initial release: normalization, sentence boundary detection, tokenization,
  morphology, ezafe detection, colloquial-to-formal rewriting and
  detokenization.
- Hazm-derived noun, verb and informal-word lexicons.
- Python bindings (abi3) and a Rust core.
- Benchmark suite and dual MIT/Apache-2.0 licensing.

[Unreleased]: https://github.com/MahdiAlikhah/rishe/compare/v0.9.1...HEAD
[0.9.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.4...v0.7.0
[0.6.4]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/MahdiAlikhah/rishe/releases/tag/v0.1.0
