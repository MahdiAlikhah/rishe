<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="Rishe: fast Persian text processing for Python, built in Rust" width="100%">
</p>

# Rishe (ریشه)

**Fast Persian text processing for Python, built in Rust.**

Rishe (*root* in Persian) turns raw Persian text into clean, structured data in
a single call: normalized text, tokens, lemmas, sentences, ezafe markers, and a
formal rewrite of colloquial writing. The core is Rust; you get it as a
`pip install` with prebuilt wheels, no models to download, and no runtime
dependencies.

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")

doc.tokens       # ['آن', 'دارد', 'تو', 'خانه', 'کتاب', 'می‌خواند', '،', 'چیست', '؟']
doc.lemmas       # ['آن', 'داشت', 'تو', 'خانه', 'کتاب', 'خواند', '،', 'چیست', '؟']
doc.formal_text  # 'آن دارد تو خانه کتاب می‌خواند، چیست؟'
```

## Why Rishe

- **Normalization about 70× faster** than Hazm and Shekar: 1.4 µs per
  sentence against 95 µs and 102 µs.
- **Millions of tokens per second.** 1.0M tokens/s on one thread; `pipe()`
  runs batches in parallel with the GIL released and reaches 6.5M tokens/s on
  12 threads.
- **Everything in one call.** Normalization, informal → formal conversion,
  tokenization, lemmatization, sentence splitting and ezafe detection from one
  `Pipeline` object, in about 16 µs per sentence.
- **Accurate where it counts.** Tokenization F1 of 0.996 on UD-Seraji and
  0.991 on UD-PerDT, ahead of Hazm (0.981 and 0.961), and ahead of both Hazm
  and Shekar on PEYMA. Lemma accuracy of 0.90 on UD-Seraji and 0.88 on
  UD-PerDT, ahead of Shekar (0.81 and 0.82) and within two points of Hazm.
- **Informal Persian handled.** Colloquial forms (`اون`, `خونه`, `داره`,
  `میخوام`, `چیه`) are rewritten to their formal spelling as part of the
  pipeline, and fused clitics like `بهش` are split into `به` + `او`.
- **Nothing to set up.** Compiled-in lexicons (190k nouns, 1.2k verb stems), no
  model downloads, no Python dependencies, no network access.
- **Wheels for every major platform.** Linux (glibc and musl, x86_64 and
  aarch64), macOS (Intel and Apple Silicon) and Windows, for Python 3.9+
  (abi3). The same core is available as a Rust crate.
- **Thread-safe.** One `Pipeline` can be shared across threads.

## Benchmarks

Measured against [Hazm](https://github.com/roshan-research/hazm) and
[Shekar](https://github.com/amirivojdan/shekar) on the full UD Persian-Seraji,
UD Persian-PerDT and PEYMA test sets, with every library built once and reused.

**Speed** (per sentence, single thread)

| | Rishe | Hazm | Shekar |
|---|---|---|---|
| Normalize | **1.4 µs** | 95 µs | 102 µs |
| Tokenize | 4.8 µs | 5.7 µs | 4.3 µs |

**Accuracy**

| Task | Dataset | Rishe | Compared with |
|---|---|---|---|
| Token F1 | UD-Seraji | **0.9963** | Hazm 0.9810 |
| Token F1 | UD-PerDT | **0.9906** | Hazm 0.9609 |
| Token F1 | PEYMA | **0.9057** | Hazm 0.8901, Shekar 0.9018 |
| Lemma accuracy | UD-Seraji | **0.9005** | Shekar 0.8056 |
| Lemma accuracy | UD-PerDT | **0.8832** | Shekar 0.8182 |

The full tables, including where Hazm and Shekar score higher, the
batch-throughput scaling across thread counts, and the method are in
[docs/BENCHMARKS.md](https://github.com/MahdiAlikhah/rishe/blob/main/docs/BENCHMARKS.md).
Reproduce everything with `python scripts/benchmark.py`.

## Install

```bash
pip install rishe        # Python >= 3.9
```

or, from Rust (`Cargo.toml`): `rishe = "0.1"`

## Usage

```python
from rishe import Pipeline

pipe = Pipeline()                         # unify digits, strip diacritics, fix ZWNJ

doc = pipe("کتاب‌های خوب را به دوستانم می‌دهم.")
doc.normalized_text, doc.formal_text, doc.sentences
doc.tokens, doc.lemmas, doc.ezafe_markers

docs = pipe.pipe(texts)                   # parallel batch -> list[ProcessedDoc]

pipe.normalize("مي كنم 2026")             # 'می‌کنم ۲۰۲۶'
pipe.word_tokenize("سلام، دنیا!")         # ['سلام', '،', 'دنیا', '!']
pipe.sent_tokenize(text)                  # sentence splitting only
pipe.lemmatize("می‌روم")                   # 'رفت'  (past-stem lemmas, UD style)
```

`pipe(text)` also rewrites colloquial words to formal ones, so its `tokens` can
differ from the surface text; `word_tokenize` tokenizes without rewriting.

```rust
use rishe::Pipeline;

let pipe = Pipeline::create(true, true, true); // unify digits, strip diacritics, fix ZWNJ
let doc = pipe.process("کتاب‌های خوب را به دوستانم می‌دهم.");
println!("{:?}", doc.tokens);
```

## Scope

Rishe is a text-preprocessing engine: normalization, tokenization, sentence
splitting, lemmatization, ezafe detection and informal → formal conversion.
For POS tagging, NER or parsing, use it as the fast preprocessing step in front
of a tagger.

## Building from source

```bash
cargo test
maturin develop --release     # Python extension into the current venv
python scripts/benchmark.py   # needs: hazm shekar conllu
```

`scripts/extract_lexicons.py` regenerates `data/*.tsv` from Hazm's data files;
`build.rs` compiles them into FSTs.

## License

Dual-licensed under [Apache-2.0](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-APACHE)
or [MIT](https://github.com/MahdiAlikhah/rishe/blob/main/LICENSE-MIT), at your
option. The bundled lexicons are derived from
[Hazm](https://github.com/roshan-research/hazm) data (MIT); see
[NOTICE.md](https://github.com/MahdiAlikhah/rishe/blob/main/NOTICE.md).
