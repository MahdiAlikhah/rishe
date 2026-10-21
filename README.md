<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="Rishe: fast Persian NLP and morphology, built in Rust" width="100%">
</p>

# Rishe (ریشه)

**Fast Persian NLP and morphology, built in Rust.**

Rishe (*root* in Persian) turns raw Persian text into clean, structured data in
one call: normalized text, tokens, lemmas, sentences, ezafe markers, and a
formal rewrite of colloquial writing. It also ships the morphology utilities
that usually live in a separate library: stemming, verb conjugation,
inflection, token splitting, spell correction, and stopword removal.

The core is Rust. You get it as a `pip install` with prebuilt wheels, no model
downloads, no runtime dependencies, and the same engine as a Rust crate.

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")

doc.tokens       # ['آن', 'دارد', 'تو', 'خانه', 'کتاب', 'می‌خواند', '،', 'چیست', '؟']
doc.lemmas       # ['آن', 'داشت', 'تو', 'خانه', 'کتاب', 'خواند', '،', 'چیست', '؟']
doc.formal_text  # 'آن دارد تو خانه کتاب می‌خواند، چیست؟'
```

## Why Rishe

- **Normalization about 80× faster** than Hazm and Shekar: 1.2 µs per sentence
  against 96 µs and 104 µs.
- **Best tokenization and lemmatization in this comparison.** Token F1 of
  1.0000 on UD-Seraji, 0.9989 on UD-PerDT and 0.9028 on PEYMA; lemma accuracy
  of 0.9209 and 0.9316, ahead of Hazm (0.9048 / 0.9009) and Shekar (0.81 / 0.82).
- **Morphology in the same core.** Stemming, conjugation, inflection, token
  splitting, spell correction and stopwords — no second library, no extra
  runtime. On the UD lemma proxy, stemming is 89.2% / 89.5% exact against Hazm
  (66.4% / 64.4%) and Shekar (76.1% / 72.0%).
- **Spell correction that is fast enough to stay in the loop.** 80.8% top-1 and
  96.2% top-5 on synthetic 1-edit typos, at 0.16 ms per word; Shekar's
  statistical checker is 60.8% / 77.6% at 94 ms per word on the same set.
- **Millions of tokens per second.** 1.0M tokens/s on one thread; `pipe()`
  runs batches in parallel with the GIL released and reaches 6.5M tokens/s on
  12 threads.
- **Everything in one call.** Normalization, informal → formal conversion,
  tokenization, lemmatization, sentence splitting and ezafe detection from one
  `Pipeline` object, in about 16 µs per sentence.
- **Informal Persian handled.** Colloquial forms (`اون`, `خونه`, `داره`,
  `میخوام`, `چیه`) are rewritten to their formal spelling, and fused clitics
  such as `بهش` and `ازش` are split into `به` + `او` and `از` + `او`.
- **Nothing to set up.** Compiled-in lexicons (190k nouns, 1.2k verb stems, word
  frequencies, stopwords), no model downloads, no Python dependencies, no
  network access.
- **Wheels for every major platform.** Linux (glibc and musl, x86_64 and
  aarch64), macOS (Intel and Apple Silicon) and Windows, for Python 3.9+
  (abi3). The same core is available as a Rust crate.
- **Thread-safe.** One `Pipeline` can be shared across threads.

## Benchmarks

Measured against [Hazm](https://github.com/roshan-research/hazm) and
[Shekar](https://github.com/amirivojdan/shekar) on the full UD Persian-Seraji,
UD Persian-PerDT and PEYMA test sets. Every library is built once and reused.

**Core speed** (per sentence, single thread)

| | Rishe | Hazm | Shekar |
|---|---|---|---|
| Normalize | **1.2 µs** | 96 µs | 104 µs |
| Tokenize | **4.1 µs** | 5.7 µs | 4.4 µs |
| Full pipeline | **16.3 µs** | — | — |

**Core accuracy**

| Task | Dataset | Rishe | Compared with |
|---|---|---|---|
| Token F1 | UD-Seraji | **1.0000** | Shekar 0.9992, Hazm 0.9810 |
| Token F1 | UD-PerDT | **0.9989** | Shekar 0.9976, Hazm 0.9609 |
| Token F1 | PEYMA | **0.9028** | Shekar 0.9018, Hazm 0.8901 |
| Lemma accuracy | UD-Seraji | **0.9209** | Hazm 0.9048, Shekar 0.8058 |
| Lemma accuracy | UD-PerDT | **0.9316** | Hazm 0.9009, Shekar 0.8170 |

**New feature highlights**

| Feature | Metric | Rishe | Hazm | Shekar |
|---|---|---|---|---|
| Stemming | exact UD-lemma proxy (Seraji / PerDT) | **89.2% / 89.5%** | 66.4% / 64.4% | 76.1% / 72.0% |
| Conjugation | hand-written spot-check (29 forms) | **29/29** | 28/29 | 29/29 |
| Conjugation | ms/verb | **0.010** | 0.152 | 0.080 |
| Inflection | comparative / superlative / plural | **100%** | N/A | **100%** |
| Token splitting | ms/token | **0.0006** | 12.05 | N/A |
| Spell correction | top-1 / top-5 on 1-edit typos | **80.8% / 96.2%** | N/A | 60.8% / 77.6% |
| Stopwords | list size | 389 | 389 | 301 |

The full core tables, batch scaling and method are in
[docs/BENCHMARKS.md](https://github.com/MahdiAlikhah/rishe/blob/main/docs/BENCHMARKS.md).
The feature tables, proxy metrics and caveats are in
[docs/FEATURE_BENCHMARKS.md](https://github.com/MahdiAlikhah/rishe/blob/main/docs/FEATURE_BENCHMARKS.md).

Reproduce:

```bash
python scripts/benchmark.py            # core token/lemma/speed
python scripts/benchmark_features.py  # stemming, conjugation, splitting, spelling
```

## Install

```bash
pip install rishe        # Python >= 3.9
```

or, from Rust (`Cargo.toml`): `rishe = "0.2"`.

No Python dependencies. No model downloads. No network access at runtime.

## Usage

```python
from rishe import Pipeline

pipe = Pipeline()                         # unify digits, strip diacritics, fix ZWNJ

doc = pipe("کتاب‌های خوب را به دوستانم می‌دهم.")
doc.normalized_text, doc.formal_text, doc.sentences
doc.tokens, doc.lemmas, doc.ezafe_markers

docs = pipe.pipe(texts)                   # parallel batch -> list[ProcessedDoc]

pipe.normalize("مي كنم 2026")             # 'می‌کنم ۲۰۲۶'
pipe.tokenize("سلام، دنیا!")              # raw tokenization, no normalization
pipe.word_tokenize("سلام، دنیا!")         # normalize + tokenize
pipe.sent_tokenize(text)                  # sentence splitting only
pipe.lemmatize("می‌روم")                   # 'رفت'  (past-stem lemmas, UD style)
pipe.lemma_full("می‌روم")                 # ('رفت', 'رو')  (past + present stem)
```

`pipe(text)` also rewrites colloquial words to formal ones, so its `tokens` can
differ from the surface text; `word_tokenize` tokenizes without rewriting.
`tokenize` is the fast raw path used for the tokenization benchmarks.

### Morphology and utilities

```python
pipe.stem("کتاب‌ها")                      # 'کتاب'
pipe.conjugate("رفت")["present"][:3]      # ['می‌روم', 'می‌روی', 'می‌رود']
pipe.comparative("بزرگ")                  # 'بزرگ‌تر'
pipe.superlative("بزرگ")                  # 'بزرگ‌ترین'
pipe.plural("کتاب")                       # 'کتاب‌ها'
pipe.possessive("خانه", 0)                # 'خانه‌ام'
pipe.split_token("بهش")                   # ['به', 'او']
pipe.spell_suggest("سسلام")               # ['سلام', 'اسلام', ...]
pipe.spell_correct("سسلام بر ششما")       # 'سلام بر شما'
pipe.tokenize_spans("سلام دنیا")          # [('سلام', 0, 4), ('دنیا', 5, 9)]  (char offsets)
pipe.remove_stopwords(tokens)             # drop Persian stopwords
pipe.process_stages(text, ["normalize", "sentences"])  # choose pipeline stages
```

Spell correction uses a 190k-word frequency lexicon and edit-distance 1; typos
with two or more edits are left unchanged.

```rust
use rishe::Pipeline;

let pipe = Pipeline::create(true, true, true);
let doc = pipe.process("کتاب‌های خوب را به دوستانم می‌دهم.");
println!("{:?}", doc.tokens);
println!("{}", pipe.stem("کتاب‌ها"));
```

## Scope

Rishe is a text-preprocessing and morphology engine: normalization,
tokenization, sentence splitting, lemmatization, stemming, verb conjugation,
inflection, token splitting, spell correction, stopword removal, ezafe
detection and informal → formal conversion.

It does **not** yet do POS tagging, chunking, NER, dependency parsing,
embeddings or text classification. Those are the next optional model packs;
the base stays zero-dependency and fast. Today, use Rishe as the preprocessing
step in front of a tagger or parser.

## Building from source

```bash
cargo test
maturin develop --release     # Python extension into the current venv
python scripts/benchmark.py   # needs: hazm shekar conllu
python scripts/benchmark_features.py
```

`scripts/extract_lexicons.py` regenerates `data/*.tsv` and
`data/word_freq.tsv` from Hazm's data files; `build.rs` compiles them into FSTs.

## License

Dual-licensed under [Apache-2.0](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-APACHE)
or [MIT](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-MIT), at your
option. The bundled lexicons, stopwords and word frequencies are derived from
[Hazm](https://github.com/roshan-research/hazm) data (MIT); see
[NOTICE.md](https://github.com/MahdiAlikhah/rishe/blob/main/NOTICE.md).
