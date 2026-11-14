# Changelog

All notable changes to Rishe are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Model packs are versioned separately from the crate; their versions are recorded
in the pack manifest and on the model repository.

## [Unreleased]

## [1.0.0] - 2026-11-14

The API freeze. The surface listed in [`docs/STABILITY.md`](docs/STABILITY.md)
is the promise: from here, removing or renaming a public name, changing a
return type, a default or a documented behaviour, or dropping a platform, a
Python version or the MSRV needs `2.0.0`. A new name arrives as a minor.

### Removed

Every alias that `0.11.0` deprecated and promised to drop here:

- Rust: `Pipeline::normalize_stages`, `mask_stages`, `lemma_full`,
  `word_tokenize`, `word_tokenize_with`; `ModelPack::tag_text`,
  `tag_text_items`, `tag_text_batch`, `tag_words`.
- Python: `Pipeline.pipe`, `process_stages`, `normalize_stages`, `mask_stages`,
  `word_tokenize`, `word_tokenize_with`, `lemma_full`; `ModelPack.tag`,
  `tag_tokens`, `tag_batch`, `tag_words`; `rishe.word_tokenize`;
  `hub.pull_pack`, `hub.cached_pack`, `hub.path_pack`.
- CLI: `--stages` is gone, `--maskers` is the only spelling. The `serve`
  request field `stages` is still accepted for 0.x clients, because the
  protocol is versioned separately.

The replacements: `tokenize` / `process` for tokens, `normalize(text, stages=...)`,
`mask(text, maskers=...)`, `lemmatize_verb`, `process_batch`,
`tags` / `tags_batch` / `tags_from_words`, and `hub.pull` / `cached` / `path`.

`TaggedToken.tag` and `Dependency.relation` stay. They are the historical field
names, equal to `label`; breaking the most-used attribute of a result class for
cosmetics was not worth it at the freeze.

### Changed

- The `DeprecationWarning` / `#[deprecated]` machinery is gone with the aliases
  it served.
- `docs/api-surface.txt` is regenerated and diffed in CI: the frozen Rust and
  Python surface is checked rather than reviewed.
- `docs/STABILITY.md` states the freeze and is linked from the README, the
  documentation index and both CLI helps.
- Building the Python extension from source needs the `extension-module`
  Cargo feature; maturin enables it through `pyproject.toml`, and the CI
  feature job runs the PyO3 tests.
- The `ProcessedDoc` stub says `lemmas` equals the tokens and
  `ezafe_markers` is empty when that stage is off, instead of promising
  aligned lemma and ezafe data for every stage list.

### Fixed

- `most_similar` caps the requested count at the vocabulary size, so an
  oversized `n` returns the whole vocabulary instead of panicking; this also
  keeps `rishe serve` and the Python server alive after such a request.
- A `.vec` file whose header declares an absurd vocabulary size now fails with
  an error instead of aborting the process.
- `JalaliDate.from_gregorian` returns `None` for years outside the supported
  calendar range instead of overflowing.
- A model pack whose `labels.json` has a `max_len` outside `3..=4096` is
  rejected when the pack is loaded.
- `Embeddings.save` writes a temporary file and renames it, so an interrupted
  `rishe convert` no longer leaves a truncated `.rvec`.
- `hub pull` in Rust resumes a partial download and reports progress on a
  terminal; the Python `pull` checks the checksum of an already cached file
  instead of trusting it.
- `hub remove` also deletes the `.part` file of an interrupted pull, and
  `hub list`/`hub info` report a partial model or half-filled pack directory
  as `partial` with its size instead of claiming it is absent.
- A checksum mismatch fails on the first attempt instead of downloading the
  whole file three times.
- `cargo clippy --features python,models` now runs in CI, as the release
  checklist says.
- `spell_suggest("")` returns an empty list, and `number_to_words`
  reports an int too large for `i64` as `OverflowError` instead of a
  `TypeError`.
- A `.vec` file whose first line declares a different number of vectors
  than the file contains is an error instead of a silently partial model.
- `tokenize(..., join_verb_parts=True)` no longer panics when the text ends
  with a verb prefix such as `خواهد`.
- `spell_suggest` and `spell_correct` skip words longer than 64 characters
  instead of building millions of one-edit candidates.
- The `html` masker no longer removes text between `<` and `>` that is not
  a tag.
- An embedding file with a non-finite value is rejected at load instead of
  returning NaN similarities.
- `possessive(noun, -1)` returns `None` instead of raising `OverflowError`.
- `possessive` inserts the linking `ی` after nouns ending in `ا`/`آ`/`و`, so
  `دنیا` gives `دنیایم` instead of `دنیام`.
- `JalaliDate.from_gregorian` returns `None` for days in 560 before
  560-03-20, matching the documented window instead of returning a date that
  cannot be converted back.
- The compat `Normalizer` honors `persian_numbers=False` and rejects
  `unicodes_replacement=False` and `seperate_mi=False` with
  `NotImplementedError` instead of silently ignoring them.
- `tokenize_spans` counts characters once per gap instead of once per token,
  so a long text costs linear time instead of quadratic.
- `spell_correct` remembers each word's correction within a call, so a text
  that repeats a misspelling is searched once per distinct word.
- `textrank` sums each word's link weights once instead of once per pass per
  link, so a wide `window` costs linear work per pass: 800 words went from
  46 s to 2.6 s at window 120.
- The sentence splitter looks back at most six characters to decide whether a
  dot ends an abbreviation, so a long run with dots and no spaces costs linear
  time: 84,000 characters of space-free Persian went from 1.35 s to under
  1 ms, and a one-line 354 KB list of URLs from 2.7 s to under 1 ms.
- `spell_correct` skips a word that contains no Persian letter instead of
  building one-edit candidates for it: 259 KB of JSON keys went from 3.3 s to
  12 ms.
- `persianize_suggest` counts character offsets incrementally instead of from
  the start of the text for every match: 780 KB with 16,000 loanwords went
  from 225 ms to 10 ms.
- `most_similar` skips the query word by text, so a `.vec` that lists a word
  twice no longer returns the query itself among its neighbours.
- The maskers no longer trim a text they did not mask, so masking leaves the
  surrounding whitespace alone when there is nothing to replace.
- The `mention` and `hashtag` maskers require a word boundary before the
  sigil, so an `@` or `#` inside a word (`info@example.com`) is no longer
  masked as a social token.
- `textrank`'s graph builder copies each word once per edge instead of twice.
- `plural`, `comparative`, `superlative`, `possessive` and `split_token`
  return an empty result for empty input instead of a bare suffix or `[""]`.
- The sentence splitter has unit tests for its punctuation, abbreviation,
  ellipsis and no-whitespace branches, which is where the quadratic scan above
  had been hiding.
- `hub pull <url>` no longer writes outside the cache directory. The incoming
  file name is validated as one plain path component, so a URL whose last
  segment carries a separator, a drive prefix or `..` is refused instead of
  joining to a path outside `$RISHE_HOME/models`. A query string or fragment is
  dropped from the inferred name instead of becoming part of it, which also
  fixes URLs like `model.vec?download=true` that previously failed with a raw
  filesystem error on Windows.
- `serve` ends cleanly when its reader closes the pipe. `rishe serve | head -1`
  used to panic on the write and exit 101; both the Rust and Python servers now
  treat a closed stdout as end of input and exit 0.
- A resumed download whose local `.part` prefix does not match the remote object
  restarts from zero instead of failing permanently. Only a mismatch on a fresh
  download, where the bytes came from the remote as-is, still fails on the first
  attempt.
- `rishe hub pull <pack> --sha256 <digest>` is refused instead of silently
  ignoring the flag. Task packs are verified against the registry manifest's own
  digests, and the Python CLI already rejected this.
- `hub.info()` always includes the `partial` key, `False` for a cached entry, so
  a caller can read it without `.get` or a `KeyError`.
- `hub remove <pack>` also clears a `rishe-models-manifest.json.part` left by an
  interrupted manifest download.
- `Normalizer(correct_spacing=False)` is honoured. The compat wrapper used to run
  the full default normalizer first, so the spacing step had already been applied
  before the flag was consulted and the switch did nothing.
- The copula split no longer mangles ordinary words that end in `ست`. `است`,
  `دست`, `هست`, `نیست`, `چیست`, `درست` and `مست` were being split into a stem plus
  `است` (`دست` became `ده است`), because the lexicon holds one-off entries such as
  `ده` and `مه` that made almost any stem look like a word. A token the lexicon
  already knows is now left alone, and a genuine contraction splits to its stem as
  written, so `کتابست` gives `کتاب است` and `عالیست` gives `عالی است` rather than
  the invented `کتابه است` / `عالیه است`.
- `split_token` no longer splits an ordinary word on a prefix. `برنامه`, `درخت`,
  `بازار`, `واژه`, `دریا` and `فروشگاه` came back as `بر`+`نامه`, `در`+`خت` and so
  on, because the prefix loop ran before the whole-word lexicon check.
- Lexicalized nouns ending in `ه` no longer lemmatize through a lookalike verb stem.
  `دانه`, `هسته`, `آیه`, `گوشه`, `سایه`, `پوشه`, `گیره`, `کنده` and `مانده` were
  reduced to `دانست`, `بود`, `آمد`, `گوشید`, `سایید`, `پوشید` and `گرفت`.
- `توت` is no longer read as `در` + `تو`; it is mulberry.
- `split_token` no longer strips a noun suffix off a fused clitic, so `بهمین`
  ("to this") stays one word instead of becoming `بهم` + `ین`. The same guard
  keeps `بهتان`, `تومان`, `بهشت` and `توتم` whole; a clitic form is not a noun
  stem.
- `split_token` keeps a ZWNJ morpheme boundary. `می‌فروش` and `پیش‌بینی` are
  lexicon entries, but the whole-word check no longer swallows them, so they
  still split to `می`+`فروش` and `پیش`+`بینی`.
- `convert` refuses to write f16 values outside ±65504 instead of writing a file
  that `load` then rejects as non-finite, and it leaves no partial file behind.
- An embedding row with a finite but very large magnitude no longer makes
  `similarity` return `NaN`. Norms and dot products accumulate in f64.
- A model whose output has the wrong number of sequence rows is an error instead
  of an index panic.
- A model pack whose `labels.json` disagrees with the model's own output width is
  an error naming both counts, instead of a panic or a mislabelled result.
- A downloaded model is flushed to disk before being moved into place, in the Rust
  and Python downloaders, so a crash during the move cannot leave a short file
  where a good one used to be.

## [0.11.4] - 2026-11-12

No functional changes: the public API, the output and the file formats are the
same as 0.11.3. This is the freeze candidate — it records the numbers `1.0.0`
will be compared against and re-verifies the published artifacts, leaving the
removal of the deprecated aliases as the only step to `1.0.0`.

Every published hub name was pulled from scratch and checksum-verified against
its manifest: `rishe-embed-fa-300-v2`, `rishe-embed-fa-300`, `pos-compact`,
`ner-compact`, `ezafe-compact`, `depparse-compact` and `hazm-fasttext-300`.

### Added

- `docs/BENCHMARKS.md` records the Rust engine baseline: median time per
  `Pipeline::process` call on this host, with the exact command and the
  run-to-run spread, so a later change can be compared with a number instead of
  a feeling.

## [0.11.3] - 2026-11-11

### Added

- `tests/guarantees.rs`: `Pipeline`, `Embeddings` and `ModelPack` are asserted to
  be `Send + Sync`, eight threads give the same result as one, and the output is
  identical across runs and across instances.
- `tests/hostile.rs`: a corrupt model pack returns an error, and a 10 MB single
  token finishes instead of hanging.
- `docs/STABILITY.md` states the concurrency and determinism guarantees, and says
  which tests hold the "error, never a panic" claim for each input format.
- `README.md` shows one badge row: the supported Python versions, total PyPI
  downloads and the license.

### Changed

- `README.md` no longer repeats the measured version table: the badge carries the
  Python versions, the install bullets keep the Rust floors, and the full matrix
  stays in `README.pypi.md`, `README.crates.md`, `README.fa.md` and
  [`docs/STABILITY.md`](docs/STABILITY.md).
- `docs/API_CONVENTIONS.md` no longer lists `ProcessedDoc.token_spans` as a field:
  it was never implemented, and character offsets are available from
  `tokenize_spans`. Adding it is additive for a later minor.

## [0.11.2] - 2026-11-10

### Added

- Doc comments on every public Rust item, enforced by `#![deny(missing_docs)]`,
  with doctests on `Pipeline`, `ProcessOptions`, `dates` and `hub`.
- Docstrings for every class, method and property in `python/rishe/_core.pyi`, so
  an editor shows the contract instead of a bare signature.
- `ProcessOptions::from_stages_checked`: the stage list of `from_stages`, except
  an unknown name is an error (additive Rust API).
- `rishe._core.DEFAULT_MASKERS`; `python/rishe/api.py` derives its tuple from it,
  so the Rust binary, the Python CLI and the package share one masker list.
- `docs/api-surface.txt` and `scripts/api_surface.py`, checked in CI, so a public
  item cannot disappear or be renamed unnoticed.
- `scripts/check_docs_examples.py`, which executes the Quick start blocks of all
  three READMEs in CI.
- `README.md` states the supported versions with measured numbers instead of a
  guess: CPython 3.9 – 3.14 for the wheel, Rust 1.88 for the default and
  `python` features, Rust 1.91 for `models` and for a source install of the
  Python package, each with the exact message an older tool refuses with. The
  PyPI, crates.io and Persian READMEs say the same, and `docs/STABILITY.md`
  carries the matrix.
- `Programming Language :: Python :: 3.14` and `Programming Language :: Python
  :: Implementation :: CPython` classifiers.

### Changed

- `process(stages=...)` rejects an unknown stage name with `ValueError` instead of
  ignoring it, so a typo cannot silently change the output; an empty list still
  means every stage, as documented. `normalize(stages=...)` already behaved this
  way.
- `serve` treats a request field of the wrong JSON type as absent in both
  implementations (`maskers`, `mask_token`, `word`, `n`), the documented rule for
  `text`. Previously the console script raised while the Rust binary used the
  default, so the same request got different answers.
- CI builds and tests the Python package on every supported CPython version
  (3.9, 3.10, 3.11, 3.12, 3.13, 3.14), and the MSRV job also compiles the
  `python` feature on Rust 1.88.

## [0.11.1] - 2026-11-09

### Added

- `serve` protocol 1: a `{"command":"version"}` handshake returning
  `{"name","protocol","version"}`, and a `date` command in both the Rust binary
  and the Python console script. The whole protocol is documented in
  `docs/serve-schema.md`.
- `docs/STABILITY.md`: which surfaces a version number covers, what a minor and
  a major release may change, the deprecation rule, and what is explicitly not
  covered (model quality, benchmark ranking, defaults, future schedules).
- `docs/errors.md`: the error contract for both CLIs (exit codes, stdout/stderr
  split, `serve` error envelope) and the exception types the Python API raises
  per failure mode.
- `.rvec` version 1 reader-forever fixture (`tests/fixtures/rvec-v1-f16.rvec`)
  with tests that load it and reject truncated or corrupt copies, in Rust and
  Python.
- Hub manifest schema versioning: the manifest reader accepts `"schema": 1`,
  treats an absent field as version 1, and refuses an unknown major version
  (Rust and Python readers agree).
- CI gates: MSRV 1.88 job, Python 3.9/3.12/3.13 matrix, offline `examples` smoke
  job, rustdoc with `-D warnings`, and a feature-combination matrix; the publish
  jobs now depend on them.

### Changed

- CLI error contract, in both the Rust binary and the Python console script:
  `0` success, `2` command line rejected by the parser, `1` failure after the
  command line parsed (missing file, bad data, out-of-vocabulary word, network,
  pack or model error). `-V`/`--version` prints `rishe <version>`.
- The Python console script's `serve` now speaks the same protocol as the Rust
  binary (version handshake, `date`, same error envelope), and `depparse`
  without `--pack` is a runtime failure (exit 1) in both.
- MSRV is now Rust 1.88. The locked dependency graph already requires it
  (`ureq` → `url` → `idna` → `icu` declare `rust-version = 1.88`), so the
  previously documented 1.75 was not buildable; `Cargo.toml`, both READMEs and
  `STABILITY.md` now say 1.88, and CI checks it.
- Clippy suggestions that became active with the MSRV bump are fixed in
  `embeddings`, `maskers`, `normalizer`, `sbd` and `utilities` with no behaviour
  change.
- Documentation: `docs/hazm.md` is now `docs/compat.md` and presents the
  `rishe.compat` classes as a feature ("drop-in preprocessing classes") rather
  than as a migration guide. The three per-release model cards are merged into
  one `docs/MODEL_CARDS.md`, linked from the README, `docs/index.md` and
  `docs/MODEL_BENCHMARKS.md`.
- Documentation: the quick-start examples in both READMEs and `docs/index.md`
  now show direct module-level calls (`rishe.normalize`, `rishe.tokenize`,
  `rishe.lemmatize`, `rishe.process(text).formal_text`) instead of constructing
  a `Pipeline`. `Pipeline` remains for configuration and for the methods that
  have no module-level function.

### Removed

- `docs/ROADMAP.md`. The project does not publish a roadmap and makes no
  promises about future releases; what is published describes what already
  shipped.

### Fixed

- `rishe.word_tokenize` (the deprecated 0.10 alias) keeps its old behaviour:
  it normalizes and joins verb parts again (`"رفته است"` → `["رفته_است"]`)
  instead of returning surface tokens, as the 0.11.0 entry promised.
- A corrupt `.rvec` header that declares an absurd vocabulary size is refused
  with an error instead of aborting the process while reserving memory; the
  reader-forever test now covers it.
- `serve` no longer dies on a request line that is not valid UTF-8: both
  implementations answer with an error envelope and keep serving, as the
  protocol page says. The Python console script also no longer prints a
  traceback for it.
- The Python console script with no arguments prints usage and exits 0, matching
  the Rust binary and `docs/errors.md`.
- `import rishe` works on Python 3.9 again: `rishe.corpora` used
  `dataclass(slots=True)`, which needs Python 3.10, so every 0.x wheel failed to
  import on the oldest interpreter it claimed to support. The new Python 3.9 CI
  job caught it.
- `hub verify` prints the same word (`invalid`) in both CLIs, as documented.
- `rishe::hub::run` keeps returning `Ok(())` for a failed `verify`, so Rust
  callers of the compatibility wrapper see no change in behaviour.

## [0.11.0] - 2026-11-08

### Added

- Module-level front door: `rishe.normalize`, `rishe.tokenize`,
  `rishe.tokenize_spans`, `rishe.sent_tokenize`, `rishe.process`,
  `rishe.process_batch`, `rishe.lemmatize`, `rishe.stem`, `rishe.mask`,
  `rishe.keywords` and `rishe.pipeline()`, all backed by one shared default
  pipeline created on first use.
- Canonical pipeline names: `Pipeline.process(text, stages=None)`,
  `process_batch(texts)`, `normalize(text, stages=None)`,
  `mask(text, maskers=..., mask_token=...)` and
  `tokenize(text, normalize=True, **options)`, plus
  `Pipeline.lemmatize_verb(word)`.
- `ModelPack.tags(text)`, `tags_batch(texts)` and `tags_from_words(words)`.
- `label` on every result span: `TaggedToken.label` and `Dependency.label`
  join the existing `Entity.label`, so one loop reads POS, NER and dependency
  output the same way.
- `rishe.hub.pull`, `cached` and `path` now accept task packs as well as
  single-file models; `hub.is_pack` reports which one a name is.
- `docs/API_CONVENTIONS.md`, the naming rules the public surface follows up to
  `1.0.0`, and `docs/ROADMAP.md`, the release-ordered backlog to it.
- `rishe._deprecation` helper so renamed names warn once and disappear at
  `1.0.0`.

### Changed

- Rust surface follows the same conventions as Python: `Pipeline::tokenize`
  takes a `word_tokenizer::WordTokenizerOptions` and returns surface tokens,
  `Pipeline::mask` replaces `mask_stages`,
  `Pipeline::normalize_with` replaces `normalize_stages`,
  `Pipeline::lemmatize_verb` replaces `lemma_full`, and `ModelPack::tags`,
  `tags_batch`, `tags_from_words` replace the `tag_text*` / `tag_words` names.
  `TaggedToken::label()` and `Dependency::label()` join `Entity.label`.
- `rishe mask` takes `--maskers`; `--stages` still works as a deprecated alias
  (same for `serve`). The masker list is defined once, in `rishe.api`.
- `tokenize` is now pure tokenization: it returns surface tokens and does not
  normalize. `normalize` normalizes, and `process` (or `pipe(text)`) runs the
  sequence; `word_tokenize*` keep their old normalize-then-tokenize behaviour as
  deprecated aliases. This applies to `Pipeline`, the module-level `rishe.tokenize`
  and `rishe tokenize` on the command line.
- Hazm-style defaults stay available from `rishe.compat` only; the compat
  façade now calls the canonical methods.
- Python test suites gained a naming ratchet (`python/tests/test_surface.py`)
  that pins the remaining deprecated aliases and may only shrink.

### Deprecated

Removed in `1.0.0`; each emits `DeprecationWarning` today.

- `Pipeline.word_tokenize` and `Pipeline.word_tokenize_with` → `tokenize`
- `Pipeline.process_stages` and `Pipeline.pipe` → `process` / `process_batch`
- `Pipeline.normalize_stages` → `normalize(stages=...)`
- `Pipeline.mask_stages` → `mask(maskers=...)`
- `Pipeline.lemma_full` → `lemmatize_verb`
- `ModelPack.tag`, `tag_tokens`, `tag_batch`, `tag_words` → `tags`,
  `tags_batch`, `tags_from_words`
- Rust: `ModelPack::tag_text`, `tag_text_items`, `tag_text_batch`, `tag_words`
  → `tags`, `tags_batch`, `tags_from_words`
- Rust: `Pipeline::word_tokenize`, `word_tokenize_with` → `tokenize`
- Rust: `Pipeline::mask_stages` → `mask`; `normalize_stages` →
  `normalize_with`; `lemma_full` → `lemmatize_verb`
- `hub.pull_pack`, `hub.cached_pack`, `hub.path_pack` → `hub.pull`, `cached`,
  `path`
- `rishe.word_tokenize` → `rishe.tokenize`

## [0.10.0] - 2026-11-07

### Added

- `rishe-embed-fa-300-v2`, a second first-party embedding pack: Word2Vec
  skip-gram, 300 dimensions, 68,144 words, trained on 150,000 Persian Wikipedia
  articles (3,830,930 sentences, 57,661,561 tokens; minimum count 20, 15 epochs).
  On ParsiNLU entailment it lifts token coverage from 82.6% to 97.2%, macro F1
  from 0.2438 to 0.2985 and the random-baseline separation gap from 0.3250 to
  0.3347.
- Compact binary embedding format `.rvec` (version 1): an `RVEC` header records
  the version, storage width, dimension and vocabulary size, then a
  length-prefixed vocabulary and the vector payload. `Embeddings::load` detects
  the format by its magic bytes and decodes both `f32` and `f16` files into the
  same in-memory layout. The v2 pack ships an `f16` file of 41,849,368 bytes
  next to the 236,068,629-byte text file.
- `rishe convert <input> <output> [--dtype f16|f32]` to rewrite any word2vec
  text file in the compact binary format.
- `Embeddings::save` and `Embeddings::dtype` (with a `DType` enum) in Rust, and
  a read-only `dtype` property on the Python `Embeddings` class.
- Hub entry for `rishe-embed-fa-300-v2` with a pinned SHA-256.
- `docs/MODEL_CARD_0.10.0.md` for the v2 pack.

### Changed

- `rishe-embed-fa-300-v2` is now the default model for `rishe embed`,
  `rishe similar`, `rishe similarity` and `rishe serve --model`; the 0.9.x pack
  remains available as `rishe-embed-fa-300`.
- Benchmarks, READMEs, the CLI reference and the hub listing quote the v2
  numbers and both pack formats.
- `.vec` and `.rvec` paths are accepted wherever an embedding file is expected.

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

[Unreleased]: https://github.com/MahdiAlikhah/rishe/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.4...v1.0.0
[0.11.4]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.3...v0.11.4
[0.11.3]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.2...v0.11.3
[0.11.2]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.1...v0.11.2
[0.11.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.9.1...v0.10.0
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
