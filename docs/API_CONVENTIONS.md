# API conventions

The rules the public surface follows from `0.11.0` onward, and therefore the set
of decisions that become permanent at `1.0.0`. Each rule lists what it replaces,
so `0.11.0` can be reviewed against this file. If a name is not covered here, the
default is that the current name stays.

The goal is one thing: **a user should never have to guess which name to call.**
Every rule below is judged by that, not by internal tidiness.

## 1. One verb per operation; variations are keyword arguments

No sibling methods that differ only by an option bundle, and no `_with` /
`_full` / `_stages` / `_char_spans` suffix families.

| before | after |
|---|---|
| `word_tokenize(text)`, `word_tokenize_with(text, ...)` | `tokenize(text)`, `tokenize(text, join_verb_parts=True, ...)` |
| `normalize_stages(text, stages)`, `normalize(text)` | `normalize(text, stages=[...])` |
| `process_stages(text, stages)`, `process_with(...)`, `pipe(texts)` | `process(text, stages=[...])`, `process_batch(texts)` |
| `lemma_full(word)` | `lemmatize(word)`, `lemmatize_verb(word)` returns `(lemma, present_stem)` |
| `tokenize_spans`, `tokenize_char_spans` | `tokenize_spans(text)` → `[(token, start, end)]` |

## 2. `_batch` is the only suffix for "many inputs"

A batch call returns one result per input, in input order, and never silently
skips an input.

| before | after |
|---|---|
| `pipe(texts)` | `process_batch(texts)` |
| `sentence_vectors(texts)` | `sentence_vectors(texts)` (kept: plural noun reads as the batch form) |
| `pos_batch`, `ner_batch`, `ezafe_batch`, `depparse_batch` | unchanged — they already follow the rule |

## 3. One name for "how many"

API: `n`. CLI: `--top` where it already reads better (`similar`), documented as
the same thing.

| before | after |
|---|---|
| `rake(text, top_n=5)`, `textrank(text, top_n=5)` | `rake(text, n=5)`, `textrank(text, n=5)` |
| `spell_suggest(word, max=5)` | `spell_suggest(word, n=5)` |
| `most_similar(word, n=5)` | unchanged |

## 4. Everything returned is a span with the same shape

`TaggedToken`, `Entity` and `Dependency` all expose `text`, `label`, `start`,
`end`; `Dependency` adds `head`. Users can write one loop for all tasks.

| before | after |
|---|---|
| `TaggedToken.tag`, `Entity.label`, `Dependency.relation` | `label` on all three |
| `TaggedToken` / `Entity` / `Dependency` class names | unchanged (no rename to `Token`) |

`tag` and `relation` remain only as migration aliases during `0.11.x` and are
removed in `1.0.0`. They are plain attributes today, so they cannot emit a
`DeprecationWarning`; the CHANGELOG entry is the migration notice for them.

## 5. "stages" and "maskers" are different words

Pipeline stages (normalize → tokenize → …) are `stages=`. Masker names
(`url`, `email`, `digits`, …) are `maskers=`. They never share a name, a
parameter or a CLI flag.

| before | after |
|---|---|
| `mask_stages(text, stages, mask_token)` | `mask(text, maskers=[...], mask_token=...)` |
| `rishe mask --stages url,email` | `rishe mask --maskers url,email` |
| `serve --stages url` (maskers) | `serve --maskers url` |

## 6. One tokenizer family; each step does one thing

`tokenize` tokenizes and does nothing else. Normalization is `normalize`, and the
sequence is `process`. A name that transforms its input is a name that lies, so
the normalizing variant is not a tokenizer option — it is the pipeline.

| before | after |
|---|---|
| `tokenize` (raw), `word_tokenize` (normalizes), `word_tokenize_with` (normalizes + options) | `tokenize(text, **options)` for surface tokens, `tokenize_spans(text)`, `sent_tokenize(text)`, `normalize(text)` for normalization, `process(text).tokens` for the whole pipeline |

The deprecated `word_tokenize*` aliases keep their historical
normalize-then-tokenize behaviour for one cycle, so a migration is explicit:

```python
pipe.tokenize(text)                 # surface tokens, no transformation
pipe.tokenize(pipe.normalize(text)) # normalized tokens, spelled out
pipe.process(text).tokens           # same, through the pipeline
```

## 7. Task methods are named after the task

`pos`, `ner`, `ezafe`, `depparse` (+ `_batch`) stay exactly as they are. No
`tag(text, task=...)` dispatcher.

`ModelPack` (the low-level pack handle) uses one verb per output shape:
`tags`, `tags_batch`, `entities`, `entities_batch`, `labels`, `task`, `max_len`.
`entities` is documented as NER-only.

## 8. `ProcessedDoc` field names

| before | after |
|---|---|
| `normalized_text` | `text` |
| `formal_text` | unchanged |
| `sentences`, `tokens`, `lemmas` | unchanged |
| `ezafe_markers` | `ezafe` |

Text-valued fields may carry a `_text` suffix (`formal_text`) when the bare name
would be ambiguous; everything else is a plural noun.

## 9. Module-level front door

The first thing a user types must not require constructing an object. Each
function below is a thin wrapper over a lazily created, shared, thread-safe
default `Pipeline`:

```python
import rishe

rishe.normalize(text)
rishe.tokenize(text)
rishe.sent_tokenize(text)
rishe.lemmatize(word)
rishe.stem(word)
rishe.mask(text, maskers=[...], mask_token="<X>")
rishe.keywords(text, n=5, method="rake")     # or method="textrank"
rishe.pipeline()                             # the shared default Pipeline
```

`Pipeline(...)` remains the configured object for models, packs and options.

## 10. Hub: one verb set for models and packs

`pull`, `cached`, `path`, `info`, `verify`, `remove` accept both a model name and
a pack name; names are disjoint (`rishe-embed-fa-300-v2` vs `pos-compact`).

| before | after |
|---|---|
| `pull(name)` / `pull_pack(name)` | `pull(name)` for both; `pull_pack` is a migration alias |
| `cached(name)` / `cached_pack(name)` | `cached(name)` for both |
| `path(name)` / `path_pack(name)` | `path(name)` for both |

## 11. Errors are builtin exceptions

`FileNotFoundError` for a missing model or pack, `ValueError` for invalid
arguments, `OSError` for I/O. No custom exception hierarchy — users should not
have to import our error types to catch a failure.

## 12. Deprecation and removal

- `0.11.x`: the new name is current, the old name works and emits
  `DeprecationWarning` (Python) / `#[deprecated]` (Rust).
- `1.0.0`: aliases are removed. The CHANGELOG lists each one.
- After `1.0.0`: an alias may still be *added* in a minor; removal requires
  `2.0.0`, after at least one minor of deprecation.

## 13. Rust and Python mirror each other

The same operation has the same name in both languages, differing only where the
language forces it: constructors (`Pipeline::create` / `Pipeline()`), convenient
getters (a Python property may be a method in Rust), and keyword arguments (a Rust
options struct with `Default`).

## 14. Parameters

- Booleans are imperative, positive, and read as the action they perform:
  `fix_zwnj=True`, `unify_digits=True`, `strip_diacritics=True`,
  `join_verb_parts=True`, `replace_links=True`. No negative flags.
- The text parameter is always `text`; a word-level input is `word`; a token
  list is `tokens`; spans are returned, never accepted as input.
- `mask_token` (not `mask`, not `token`) is the replacement string.
- Pack paths are `pos_pack`, `ner_pack`, `ezafe_pack`, `depparse_pack`;
  embedding paths are `model`. CLI mirrors: `--pos-pack`, `--model`.

## 15. CLI mirrors the API

A CLI flag is the Python keyword with `-` instead of `_`
(`--mask-token` ↔ `mask_token`). One pack flag spelling per command: `pos`,
`ner`, `ezafe`, `depparse` accept `--pack`; `serve` accepts the four `--*-pack`
flags; both are documented as the same idea.

## 16. Mechanically enforced

`python/tests/test_surface.py` (and the Rust equivalent) fails when:

- a public name ends with `_with`, `_full`, `_stages` or `_char_spans`;
- a "many inputs" method does not end with `_batch` and is not a documented
  plural noun;
- a count parameter is not called `n`;
- a result class does not expose `label`.

The current violations are pinned in an explicit allowlist in that test, and the
list may only shrink. `docs/api-surface.txt` is diffed in CI, so a rename after
`1.0.0` is a red build rather than a review comment.
