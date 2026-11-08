# Roadmap to 1.0.0

Status: `0.10.0` shipped (2026-11-07). This file is the release-ordered backlog
from here to `1.0.0` and the policy that applies after it. It replaces the
scattered "what's next" notes: if an item is not here, it is not planned.

`1.0.0` is a promise about the **surface** (Python API, Rust API, CLI, JSON,
on-disk formats, published names), not about model quality. Weights are versioned
separately and ship under their own names. The exact wording of the promise lives
in `STABILITY.md` (added in 0.11.1).

Nothing in the ladder below is research; it is surface design, contracts, CI and
documentation. Model work starts after `1.0.0` and is additive by construction.

## Support matrix to freeze at 1.0.0

| surface | target at 1.0.0 |
|---|---|
| Python | CPython 3.9 – 3.13, abi3 wheel, no runtime dependencies |
| Rust | MSRV 1.75, `stable` and `--features python`, `models`, `python,models` |
| Platforms | Linux x86_64/aarch64 (glibc + musl), macOS x86_64/arm64, Windows x86_64 |
| CLI | the 15 commands in `docs/cli.md`, plus `serve` JSON-lines |
| Formats | `.vec` text, `.rvec` v1, pack `manifest.json` schema 1 |

## Release ladder

| release | theme | breaking? |
|---|---|---|
| **0.11.0** | Surface audit: naming, tokenizer entry points, deprecation shims | **yes** (last window) |
| **0.11.1** | Contracts and CI gates: `STABILITY.md`, `serve` schema, error contract, MSRV/Python matrix | no |
| **0.11.2** | Docs completeness: every public item documented, API-surface snapshot gate | no |
| **0.11.3** | Additive conveniences and guarantee tests (offsets, vocabulary, streaming, thread-safety) | no |
| **0.11.4** | Freeze candidate: no code changes except fixes, full matrix re-run, baselines recorded | no |
| **1.0.0** | The promise | — |
| *after* | `1.x` additive only; removals require `2.0.0` | — |

### 0.11.0 — surface audit (breaking, pre-1.0)

Status: released as 0.11.0 on 2026-11-08. Front door, unified hub verbs,
`--maskers` on both binaries, deprecation helper, naming ratchet,
`docs/API_CONVENTIONS.md` (reviewed), the canonical `#[pyo3(name = …)]` surface,
and Rust parity (`Pipeline::tokenize` with `WordTokenizerOptions`, `mask`,
`normalize_with`, `lemmatize_verb`, `ModelPack::tags*`, `label()` on result
spans). `tokenize` is pure tokenization; `normalize` and `process` are separate.
The stub matches the runtime surface exactly (all 8 classes, zero drift).
Next: 0.11.1 (contracts + CI gates).

The only release allowed to rename things. Convention to adopt, documented in
`docs/API_CONVENTIONS.md`:

1. One verb per operation; variations are keyword arguments, not extra methods
   (`word_tokenize_with(...)` becomes flags on `tokenize`).
2. `_batch` is the only suffix for "same call, many inputs", returning one list
   per input.
3. Stage selection is a keyword (`stages=`), not a parallel method name.
4. One spelling for tag/span results across tasks.

- [x] `docs/API_CONVENTIONS.md` written
- [x] `docs/API_CONVENTIONS.md` reviewed line by line
- [x] Tokenizer entry points collapsed (Python): `tokenize`, `tokenize_spans`,
      plus options; `word_tokenize*` are deprecated aliases
- [x] Same for Rust: one `Pipeline::tokenize` taking `WordTokenizerOptions`
- [x] `process_stages` / `normalize_stages` / `mask_stages` folded into
      `process`, `normalize`, `mask` with `stages=` / `maskers=` (Python and
      Rust: `normalize_with`, `mask`)
- [x] `mask` flag renamed to `--maskers` in both binaries (`--stages` aliased)
- [x] `ModelPack` reduced to `tags`, `tags_batch`, `tags_from_words` (+ aliases,
      Python and Rust)
- [x] `Embeddings.sentence_vector` / `sentence_vectors` naming settled
      (`sentence_vector` singular, `sentence_vectors` batch — kept and
      documented in `docs/api.md`)
- [x] Aliases emit `DeprecationWarning` in Python (helper in
      `python/rishe/_deprecation.py`)
- [x] Rust `#[deprecated]` aliases and core `mask(..)` parity
- [x] Rename table recorded in the CHANGELOG under `[Unreleased]`
- [x] Removal schedule recorded: aliases go away in `1.0.0`, not before

**Exit gate:** full matrix green; stub diff reviewed line by line; no alias is
advertised in docs or examples.

### 0.11.1 — contracts and CI gates

- [ ] `docs/STABILITY.md`: per-surface promise, what may change in minors,
      deprecation policy (deprecate in a minor, remove only in 2.0.0), explicit
      non-goals (model quality, benchmark ranking)
- [ ] `docs/serve-schema.md` + tests: JSON-lines v1, every response shape,
      `version` field in the handshake
- [ ] Error contract doc + tests: exit codes (`0` ok, `1` runtime failure,
      `2` usage/IO), Python exception types per failure mode
- [ ] Pack `manifest.json` gains `"schema": 1`; parsing accepts it and refuses
      unknown major versions
- [ ] `.rvec` v1 fixture committed and loaded by a test that must never be
      edited (reader-forever check)
- [ ] CI: MSRV 1.75 job; Python 3.9 and 3.13 test jobs; `examples/` smoke job;
      `cargo doc -D warnings`; feature-combination matrix
- [ ] Verify each contract test fails when the contract is deliberately broken

**Exit gate:** every new CI job green; one deliberate break per contract shows a
red build.

### 0.11.2 — docs completeness and surface snapshot

- [ ] `#![deny(missing_docs)]` on the crate; doc comments on all 284 public
      items (223 `pub fn`)
- [ ] Docstrings in `python/rishe/_core.pyi` for all public classes and methods
      (currently zero)
- [ ] Doctests for `Pipeline`, `Embeddings`, `hub`, `dates`, `corpora`
- [ ] API-surface snapshot committed (`docs/api-surface.txt`) and diffed in CI
      against the tagged `1.0.0` surface once it exists
- [ ] Every README/`docs/` example executed by CI

**Exit gate:** `cargo doc` and doctests clean; snapshot gate active; zero
undocumented public items.

### 0.11.3 — additive conveniences and guarantee tests

- [ ] `ProcessedDoc.token_spans` (character offsets), consistent with
      `TaggedToken.start` / `Entity.start` / `Dependency.start`
- [ ] `Embeddings.words()` (vocabulary enumeration) and documented ordering
- [ ] Streaming entry point for large corpora (`pipe_iter(texts)`)
- [ ] Guarantee tests: `Pipeline` and `Embeddings` are `Send + Sync`; concurrent
      use from N threads produces identical results; determinism across runs
- [ ] Hostile-input tests: truncated/garbage `.vec`, `.rvec`, pack files, huge
      single token, 10 MB line — must error, not panic or hang
- [ ] Guarantees written into `STABILITY.md`

**Exit gate:** property/determinism/concurrency tests green; no panic reachable
from the public API with fuzzed text input.

### 0.11.4 — freeze candidate

- [ ] No functional changes; only fixes and docs from 0.11.3
- [ ] Full matrix re-run on all platforms and feature combinations
- [ ] Reference benchmark numbers recorded in `docs/BENCHMARKS.md` as the 1.0.0
      baseline (this host, documented method)
- [ ] `rishe hub verify` for every published pack and embedding name
- [ ] Optional: tag `v1.0.0-rc1` and publish to TestPyPI for an install smoke
      test before the real release

**Exit gate:** a release that could be tagged `1.0.0` today, with the only
difference being the version number and `STABILITY.md`'s tense.

### 1.0.0 — the promise

- [ ] Version bump; CHANGELOG "1.0.0 — API freeze"
- [ ] `STABILITY.md` final, linked from README, `docs/index.md` and the CLI help
- [ ] Deprecated aliases removed (documented in the CHANGELOG)
- [ ] `cargo semver-checks` enabled against the 1.0.0 baseline in CI
- [ ] All Python wheels published for the frozen matrix, docs.rs green
- [ ] Hub entries verified; `.rvec` v1 and pack schema 1 fixtures load
- [ ] Announcement notes state exactly what is frozen and what is not

## After 1.0.0

| change | allowed in |
|---|---|
| new pack under a new name, new hub entry, new optional Cargo feature, new method, additive keyword, new CLI command, perf work, docs | any `1.x` |
| new default pack (old name still resolves, changelog note) | `1.x` minor |
| adding a deprecated alias | `1.x` minor |
| renaming/removing a public name, changing a return type, changing documented behavior, removing a platform or Python version, changing `.rvec` v1 or pack schema 1 semantics, removing a CLI flag | `2.0.0` only, after at least one minor of deprecation |

## Optional backlog after 1.0.0

Ordered by value at the time of writing; none of it is a 1.0.0 blocker.

**P1**

- [ ] Sentence-embedding pack: EmbeddingGemma 2, text-only backbone, fine-tuned
      or distilled on license-clean Persian pairs; `SentenceEmbedder` class,
      `sentence` Cargo feature, ONNX via tract (no onnxruntime in the base wheel)
- [ ] int8 / distilled task packs, once a runtime path is fast enough
- [ ] License-clean Persian STS/NLI evaluation set we can publish (ParsiNLU is
      CC BY-NC-SA, evaluation only)

**P2**

- [ ] Shared encoder for POS/NER/ezafe to cut the ~182 MB pack download
- [ ] Promote the contextual-encoder evaluation into `scripts/` (no notebook
      needed to reproduce a model card)

**P3**

- [ ] FastText subword static variant (measured: better intrinsic gap, flat F1,
      367 MB, larger runtime)
- [ ] Larger static vocabulary (measured saturated past 150k articles)
- [ ] Retrieval helpers (BM25, chunking) as a separate module
- [ ] Extractive summarization on top of existing keyword/TextRank work

## Non-goals

- Publishing training notebooks.
- Shipping competitor weights (`hazm-fasttext-300`) as a default or as "our
  model"; it stays a labelled external comparison.
- Adding runtime dependencies to the base install (no ONNX Runtime, spaCy,
  gensim, numpy in the base wheel).
- Guaranteeing benchmark ranking or model quality inside the version number.

## Open decisions

1. Approve the naming conventions in `docs/API_CONVENTIONS.md` (0.11.0 blocker).
2. Confirm defaults are not frozen: a minor may change the default pack while
   every published name keeps resolving.
3. Confirm the support matrix above, in particular Python 3.9 and MSRV 1.75.
