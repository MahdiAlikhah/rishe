<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="Rishe: fast Persian NLP" width="100%">
</p>

# Rishe (ریشه)

Fast Persian NLP and morphology for Python. Prebuilt wheels, no Python
dependencies, and the base pipeline needs no model downloads.

## Install

```bash
pip install rishe
```

Python 3.9+. Linux (glibc and musl), macOS and Windows wheels are published.

## Quickstart

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")

doc.tokens       # ['آن', 'دارد', 'تو', 'خانه', 'کتاب', 'می‌خواند', '،', 'چیست', '؟']
doc.lemmas       # ['آن', 'داشت', 'تو', 'خانه', 'کتاب', 'خواند', '،', 'چیست', '؟']
doc.formal_text  # 'آن دارد تو خانه کتاب می‌خواند، چیست؟'
```

`pipe.normalize`, `word_tokenize`, `sent_tokenize`, `lemmatize`, `stem`,
`conjugate`, `spell_correct` and the rest are methods on the same object.
`pipe(text)` rewrites colloquial forms; `word_tokenize` tokenizes the surface
text without rewriting.

## Class-based API

The same engine is available as small, reusable classes:

```python
from rishe import (
    Lemmatizer,
    Normalizer,
    SentenceTokenizer,
    Stemmer,
    TokenSplitter,
    WordTokenizer,
    sent_tokenize,
    word_tokenize,
)

Normalizer().normalize("مي كنم")             # 'می‌کنم'
WordTokenizer().tokenize("رفته است")         # ['رفته_است']
SentenceTokenizer().tokenize("یک. دو.")
Lemmatizer().lemmatize("می‌روم")              # 'رفت#رو'
Stemmer().stem("کتاب‌ها")                    # 'کتاب'
TokenSplitter().split_token_words("بهش")     # [('به', 'او')]

word_tokenize("رفته است")                    # ['رفته_است']
sent_tokenize("یک. دو.")
```

No lexicon paths are needed: the classes use Rishe's compiled-in lexicons.
`WordTokenizer` accepts the same options as the pipeline tokenizer
(`join_verb_parts`, `replace_emails`, `replace_links`, and so on).

## Features

- Normalization, tokenization, sentence splitting and lemmatization.
- Stemming, conjugation, inflection, clitic splitting, spell correction,
  stopwords and ezafe detection.
- Static fastText/word2vec embeddings with sentence pooling.
- Optional POS, NER and learned ezafe model packs.

## Feature tour

### Morphology and utilities

```python
pipe.stem("کتاب‌ها")                      # 'کتاب'
pipe.conjugate("رفت")["present"][:3]      # ['می‌روم', 'می‌روی', 'می‌رود']
pipe.comparative("بزرگ")                  # 'بزرگ‌تر'
pipe.superlative("بزرگ")                  # 'بزرگ‌ترین'
pipe.plural("کتاب")                       # 'کتاب‌ها'
pipe.possessive("خانه", 0)                # 'خانه‌ام'
pipe.split_token("بهش")                   # ['به', 'او']
pipe.spell_correct("سسلام بر ششما")       # 'سلام بر شما'
pipe.tokenize_spans("سلام دنیا")          # [('سلام', 0, 4), ('دنیا', 5, 9)]
```

### Normalization and masking

`normalize_stages` applies individual operators in the order you list them;
`mask_stages` removes or masks URLs, emails, mentions, hashtags, HTML, digits,
diacritics, punctuation, stopwords, offensive words and non-Persian text.

```python
pipe.normalize_stages("﷽ 1𝟮3٤٥⓺", ["arabic_unicode", "digits"])
# 'بسم الله الرحمن الرحیم ۱۲۳۴۵۶'

pipe.mask_stages("سلام @user، به https://rishe.dev یا info@rishe.dev بنویس.",
                 ["url", "email", "mention"])
# 'سلام   به <URL> یا <EMAIL> بنویس.'
```

### Utilities, keywords and transliteration

```python
pipe.number_to_words(1404)                 # 'یک هزار و چهارصد و چهار'
pipe.ngrams("این یک متن", 1, 2)             # ['این', 'یک', 'متن', 'این یک', 'یک متن']
pipe.persianize("فایل را با کامپیوتر باز کن")  # 'پرونده را با رایانه باز کن'
pipe.rake(text, top_n=5)
pipe.textrank(text, top_n=5)
pipe.to_tajik("خانه")                      # 'хона'
pipe.to_persian("хона")                    # 'خانه'
```

`keyboard_noise`, `ocr_noise` and `whitespace_noise` add reproducible typing, OCR
and spacing noise; pass `seed=` for deterministic output.

### Tokenizer options

```python
pipe.word_tokenize_with("رفته است", join_verb_parts=True)   # ['رفته_است']
pipe.word_tokenize_with("info@a.co", replace_emails=True)   # ['EMAIL']
```

Options include `join_verb_parts`, `join_abbreviations`, `separate_emoji`,
`replace_links`, `replace_ids`, `replace_emails`, `replace_numbers` and
`replace_hashtags`.

### Corpora

```python
from rishe import ConlluReader, PeymaReader, PlainTextReader

for sentence in ConlluReader("data/corpora/fa_perdt-ud-test.conllu"):
    print(sentence.text, sentence.tokens[0].lemma)
```

`ConlluReader` handles CoNLL-U, `PeymaReader` reads the PEYMA format and
`PlainTextReader` reads one sentence per line.

## Model packs

POS, NER and ezafe are separate, checksum-verified downloads. They run inside
the package without ONNX Runtime or a Python ML stack.

```bash
rishe hub pull pos-compact       # also: ner-compact, ezafe-compact
```

```python
from rishe import ModelPack, hub

pos = ModelPack(hub.path_pack("pos-compact"))
pos.tag("کتاب‌های خوب را به دوستانم می‌دهم.")
# [('کتاب‌های', 'NOUN'), ('خوب', 'ADJ'), ('را', 'ADP'), ('به', 'ADP'),
#  ('دوستانم', 'NOUN'), ('می‌دهم', 'VERB'), ('.', 'PUNCT')]

pos.tag_words(["کتاب‌های", "خوب"])   # pre-tokenized input

ner = ModelPack(hub.pull_pack("ner-compact"))
ezafe = ModelPack(hub.pull_pack("ezafe-compact"))
```

Use `hub.pull_pack(name)` to download and return the directory, or
`hub.path_pack(name)` for an existing cache entry.

## Embeddings

```bash
rishe hub pull hazm-fasttext-300   # optional 2.3 GB static embedding file
```

```python
from rishe import Embeddings, hub

emb = Embeddings(hub.path("hazm-fasttext-300"))
emb.dim, len(emb)                      # vector size, vocabulary size
emb.vector("کتاب")                     # list[float] or None
emb.sentence_vector("این کتاب خوب است") # mean of in-vocab token vectors
emb.most_similar("کتاب", 5)            # [(word, cosine), ...]
```

## CLI

```bash
rishe normalize "مي كنم"
rishe tokenize "سلام دنیا" --json
rishe mask "info@a.co" --stages email
```

## Docs

- [API](https://github.com/MahdiAlikhah/rishe/blob/main/docs/api.md)
- [CLI](https://github.com/MahdiAlikhah/rishe/blob/main/docs/cli.md)
- [Benchmarks](https://github.com/MahdiAlikhah/rishe/blob/main/docs/BENCHMARKS.md)
- [Model-pack benchmarks](https://github.com/MahdiAlikhah/rishe/blob/main/docs/MODEL_BENCHMARKS.md)
- [GitHub repository](https://github.com/MahdiAlikhah/rishe)

## License

Dual-licensed under
[Apache-2.0](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-APACHE) or
[MIT](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-MIT), at your
option. Lexicon attributions are in
[NOTICE.md](https://github.com/MahdiAlikhah/rishe/blob/main/NOTICE.md).

Built by Mahdi Alikhah.
