# Migrating from Hazm

Rishe ships Hazm-style wrappers for the core preprocessing classes, so most
migration is an import change:

```python
# from hazm import Lemmatizer, Normalizer, WordTokenizer
from rishe import Lemmatizer, Normalizer, WordTokenizer
```

## Covered classes

| Hazm | Rishe wrapper | Notes |
|---|---|---|
| `Normalizer` | `Normalizer` | Accepts Hazm's boolean flags and maps them to normalizer operators. |
| `WordTokenizer` | `WordTokenizer` | Same option names; `join_verb_parts` defaults to `True` like Hazm. |
| `SentenceTokenizer` | `SentenceTokenizer` | `tokenize(text)` and callable. |
| `Lemmatizer` | `Lemmatizer` | `lemmatize(word)` returns Hazm's `past#present` form. |
| `Stemmer` | `Stemmer` | `stem(word)`. |
| `TokenSplitter` | `TokenSplitter` | `split_token_words(token)` returns a list of candidate tuples. |
| `word_tokenize` | `word_tokenize` | Uses verb-part joining like Hazm's helper. |
| `sent_tokenize` | `sent_tokenize` | Sentence splitting only. |

## Differences

- **Compiled lexicons.** `words_file` and `verbs_file` are rejected with
  `NotImplementedError` instead of being silently ignored. Rishe's lexicons are
  built into the binary.
- **Partial coverage.** POSTaggers, parsers, chunkers, corpus-specific readers
  and embeddings are not part of the base package. Use the `Pipeline` API for
  tokenization, lemmatization, stemming, normalization, masking and morphology.
- **Normalizer output is close, not byte-identical.** Hazm applies a larger set
  of spacing and style rules; Rishe exposes them as composable operators via
  `normalize_stages`. If you need an exact Hazm output, keep Hazm for that step
  while migrating the rest.
- **Lemmatizer output.** `Lemmatizer.lemmatize` joins the past and present stems
  with `#` for compatibility with Hazm. The plain `Pipeline.lemmatize` returns
  only the UD-style past lemma; `Pipeline.lemma_full` returns both parts.
