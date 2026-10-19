# Rishe (ریشه): fast Persian text processing for Python, built in Rust

`pip install rishe` gives you Persian normalization, tokenization, sentence
splitting, lemmatization, ezafe detection and informal → formal conversion
from one Rust core: no models to download, prebuilt wheels for Linux, macOS
and Windows. The core is also available as a Rust crate.

- Normalization is ~70× faster than Hazm and Shekar; tokenization is on par
  with them. See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for the full
  numbers, including where Rishe loses.
- `Pipeline.pipe()` processes batches in parallel (rayon) with the GIL
  released.

**Status: alpha (0.1).** Rishe is a fast preprocessing layer, not a
replacement for Hazm or Shekar: there is no POS tagging, NER, parsing,
spell checking or embeddings.

## Install

```bash
pip install rishe        # Python >= 3.9
```

or, from Rust (`Cargo.toml`): `rishe = "0.1"`

## Quick start

```python
from rishe import Pipeline

pipe = Pipeline()
doc = pipe("اون داره تو خونه کتاب میخونه، چیه؟")
doc.tokens       # ['آن', 'دارد', 'تو', 'خانه', 'کتاب', 'می‌خواند', '،', 'چیست', '؟']
doc.lemmas       # ['آن', 'داشت', 'تو', 'خانه', 'کتاب', 'خواند', '،', 'چیست', '؟']
doc.formal_text  # 'آن دارد تو خانه کتاب می‌خواند، چیست؟'
doc.sentences, doc.normalized_text, doc.ezafe_markers

pipe.word_tokenize("سلام، دنیا!")   # ['سلام', '،', 'دنیا', '!']
pipe.lemmatize("می‌روم")             # 'رفت'  (past-stem lemmas, UD style)
pipe.pipe(texts)                     # parallel batch -> list[ProcessedDoc]
```

```rust
use rishe::Pipeline;

let pipe = Pipeline::create(true, true, true); // unify digits, strip diacritics, fix ZWNJ
let doc = pipe.process("کتاب‌های خوب را به دوستانم می‌دهم.");
println!("{:?}", doc.tokens);
```

## Known limitations

- `Pipeline.__call__` also rewrites colloquial words to formal ones, so
  `doc.tokens` can differ from the surface text. Use `word_tokenize` for
  plain tokenization.
- Informal → formal conversion is lexicon- and rule-based. Common forms
  (`اون`, `خونه`, `داره`, `چیه`, `میخوام`) work; many suffixed or rarer forms
  (`خونه‌ام`, `کتابه`) are left unchanged.
- Lemmas follow the Universal Dependencies convention (verbs lemmatize to the
  past stem). Accuracy on UD is ~0.90 (Seraji) and ~0.88 (PerDT).
- Ezafe detection is rule-based and has not been evaluated.

## Building from source

```bash
cargo test
maturin develop --release     # Python extension into the current venv
python scripts/benchmark.py   # needs: hazm shekar conllu
```

`scripts/extract_lexicons.py` regenerates `data/*.tsv` from Hazm's data
files; `build.rs` compiles them into FSTs.

## License

Dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at
your option. The bundled lexicons are derived from
[Hazm](https://github.com/roshan-research/hazm) data (MIT); see
[NOTICE.md](NOTICE.md).
