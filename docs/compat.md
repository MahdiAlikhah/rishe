# Drop-in preprocessing classes

Rishe ships the preprocessing classes that Persian NLP code already imports, so
an existing preprocessing layer can run on Rishe without being rewritten:

```python
from rishe import Lemmatizer, Normalizer, WordTokenizer

Normalizer().normalize("مي كنم")        # 'می‌کنم'
WordTokenizer().tokenize("رفته است")    # ['رفته_است']
Lemmatizer().lemmatize("می‌روم")        # 'رفت#رو'
```

The classes live in `rishe.compat` and are also exported from the package root.
They are thin wrappers: every one of them calls the same `rishe.Pipeline` as the
rest of the package, so there is no second implementation and no second set of
results. Nothing is downloaded and nothing needs a lexicon path — Rishe compiles
its lexicons into the binary.

The class names, constructor flags and return shapes follow the API the Persian
NLP ecosystem standardised on, so switching a project over is an import change
rather than a rewrite. New code can use the module-level functions instead
(`rishe.normalize`, `rishe.tokenize`, `rishe.lemmatize`, …) — both surfaces call
the same engine. See [api.md](api.md) for that surface and
[API_CONVENTIONS.md](API_CONVENTIONS.md) for how names are chosen.

## Classes

| class | what it does | notes |
|---|---|---|
| `Normalizer` | normalizes text | accepts the familiar boolean flags (`correct_spacing`, `remove_diacritics`, `persian_style`, …) and maps them to normalizer operators |
| `WordTokenizer` | word tokenization | same option names; `join_verb_parts` defaults to `True`; `tokenize` normalizes first, like the classic helper |
| `SentenceTokenizer` | sentence splitting | `tokenize(text)`, and the instance is callable |
| `Lemmatizer` | lemma lookup | `lemmatize(word)` returns the `past#present` form (`'رفت#رو'`); `Pipeline.lemmatize` returns only the past lemma |
| `Stemmer` | stemming | `stem(word)` |
| `TokenSplitter` | clitic and compound splitting | `split_token_words(token)` returns a list of candidate tuples — `[('به', 'او')]` for `بهش` |
| `word_tokenize` | tokenization with verb parts joined | `rishe.compat.word_tokenize`; `Pipeline.tokenize` follows `join_verb_parts=` instead |
| `sent_tokenize` | sentence splitting | same as `rishe.sent_tokenize` |

## Differences from the classic behaviour

- **Compiled lexicons.** `words_file` and `verbs_file` are rejected with
  `NotImplementedError` instead of being silently ignored. Rishe's lexicons are
  built into the binary, so there is nothing to point at.
- **`Normalizer` switches.** `persian_numbers=False` turns digit unification
  off, and `correct_spacing=False` leaves whitespace and ZWNJ placement alone.
  Two settings cannot be honored by the default normalizer and are rejected with
  `NotImplementedError` rather than ignored:
  `unicodes_replacement=False` (Arabic letters are always mapped) and
  `seperate_mi=False` while `correct_spacing=True` (the spacing step separates
  `می`). Use `rishe.normalize(text, stages=[...])` when a step has to be
  byte-level controlled.
- **Partial coverage.** The classic wrappers for POSTaggers, parsers, chunkers
  and embeddings are not part of this module. Use `Pipeline` (or the optional
  model packs) for tagging, parsing and embeddings; corpus readers ship as
  `rishe.corpora` and embeddings as `rishe.Embeddings`.
- **Normalizer output is close, not byte-identical.** Hazm applies a larger set
  of spacing and style rules; Rishe exposes them as composable operators through
  `normalize(stages=...)`, so a stage-by-stage comparison shows exactly where the
  two differ. If a step needs a byte-identical old output, keep that step where it
  is while the rest moves over.
- **Lemmatizer output.** `Lemmatizer.lemmatize` joins the past and present stems
  with `#` to stay compatible. `Pipeline.lemmatize` returns only the UD-style
  past lemma, and `Pipeline.lemmatize_verb` returns both parts.
- **Tokenizer output.** `WordTokenizer.tokenize` normalizes before tokenizing,
  matching the classic helper. The rest of the package separates the steps:
  `rishe.tokenize` returns surface tokens and `rishe.normalize` normalizes.
