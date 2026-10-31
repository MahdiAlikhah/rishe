<p align="center">
  <img src="https://raw.githubusercontent.com/MahdiAlikhah/rishe/main/docs/banner.jpg" alt="Rishe: fast Persian NLP" width="100%">
</p>

# Rishe (ریشه)

Fast Persian NLP and morphology for Rust.

[Persian README](https://github.com/MahdiAlikhah/rishe/blob/main/README.fa.md)

## Install

```bash
cargo add rishe
```

The default crate has no model downloads and builds on Rust 1.75. The optional
`models` feature adds POS, NER and ezafe pack inference and needs Rust 1.91.

## What you get

- `Pipeline`: normalization, tokenization, sentence splitting, lemmatization,
  ezafe marking, formal rewrites and batch processing.
- Morphology methods: stemming, conjugation, inflection, clitic splitting,
  spell correction and stopword helpers.
- Normalization and masking stages, key-phrase extraction, transliteration,
  augmentation and number/utility helpers.
- Static fastText/word2vec embeddings, loaded once and shared read-only.
- Optional POS, NER and learned ezafe model packs.

The individual engines are public modules too: `normalizer`, `tokenizer`,
`word_tokenizer`, `sbd`, `ezafe`, `morphology`, `conjugation`, `inflector`,
`stemmer`, `spell`, `stopwords`, `keywords`, `transliterate`, `augment`,
`utilities` and `embeddings`.

## Quickstart

```rust
use rishe::Pipeline;

let pipe = Pipeline::create(true, true, true);
let doc = pipe.process("کتاب‌های خوب را به دوستانم می‌دهم.");
println!("{:?}", doc.tokens);
println!("{}", pipe.stem("کتاب‌ها"));
println!("{}", pipe.spell_correct("سسلام بر ششما"));
```

The `Pipeline` exposes the same utilities directly: `normalize_stages`,
`mask_stages`, `word_tokenize`, `lemma_full`, `stem`, `conjugate`,
`comparative`, `superlative`, `plural`, `possessive`, `tokenize_spans`,
`tokenize_char_spans`, `spell_suggest`, `spell_correct`, `split_token`, `rake`,
`textrank`, `ngrams` and batch processing. Sentence splitting lives in
`rishe::sbd::SentenceSplitter`.

```rust
use rishe::sbd::SentenceSplitter;
use rishe::Pipeline;

let pipe = Pipeline::create(true, true, true);
let text = "کتاب‌های خوب را به دوستانم می‌دهم.";

let normalized = pipe
    .normalize_stages("سلام ۱۲۳", &["digits".to_string()])
    .unwrap();
let masked = pipe
    .mask_stages("info@a.co", &["email".to_string()], None)
    .unwrap();

let tokens: Vec<String> = pipe.word_tokenize(text);
let (lemma, present) = pipe.lemma_full("می‌روم"); // ("رفت", Some("رو"))
let stem = pipe.stem("کتاب‌ها");
let conjugation = pipe.conjugate("رفت");           // Option<Conjugation>
let comparative = pipe.comparative("بزرگ");
let superlative = pipe.superlative("بزرگ");
let plural = pipe.plural("کتاب");
let possessive = pipe.possessive("خانه", 0);      // Option<String>
let byte_spans = pipe.tokenize_spans(text);       // (token, byte_start, byte_end)
let char_spans = pipe.tokenize_char_spans(text);   // (token, char_start, char_end)
let suggestions = pipe.spell_suggest("سسلام", 5);
let corrected = pipe.spell_correct("سسلام بر ششما");
let parts = pipe.split_token("بهش");
let rake_keywords = pipe.rake(text, 5, 3);
let textrank_keywords = pipe.textrank(text, 5, 3, 3);
let ngrams = pipe.ngrams(text, 1, 2).unwrap();
let docs = pipe.process_batch(&["یک متن".to_string(), "متن دیگر".to_string()]);
let sentences = SentenceSplitter::split(text);
```

## Model packs

```bash
cargo add rishe --features models
```

Download a pack from
[MahdiAlikhah/rishe-models](https://huggingface.co/MahdiAlikhah/rishe-models),
then load it into a `Pipeline`:

```rust
let mut pipe = rishe::Pipeline::create(true, true, true);
pipe.load_pack("pos", "path/to/pos").expect("load pos");

for token in pipe.pos("کتاب‌های خوب را به دوستانم می‌دهم.").expect("pos") {
    println!("{}\t{}\t{}..{}", token.text, token.tag, token.start, token.end);
}

pipe.load_pack("ner", "path/to/ner").expect("load ner");
for entity in pipe.ner("سرلشگر فیروزآبادی به پادشاه عربستان گفت.").expect("ner") {
    println!("{}\t{}\t{}..{}", entity.text, entity.label, entity.start, entity.end);
}

let batch = pipe
    .pos_batch(&["کتاب‌های خوب".to_string(), "خوب است".to_string()])
    .expect("batch");
```

`TaggedToken` exposes `text`, `tag`, `start` and `end`; `Entity` exposes
`text`, `label`, `start` and `end`. Offsets are character offsets. The
lower-level `rishe::models::ModelPack` is still available for one task.

The packs are word-level ONNX files run through tract. No ONNX Runtime install
is required.

## Embeddings

Static fastText/word2vec embeddings are available through
`rishe::embeddings::Embeddings`; the model is loaded once and shared read-only.

```rust
let embeddings = rishe::embeddings::Embeddings::load("path/to/model.vec")
    .expect("load embeddings");
println!("{:?}", embeddings.vector("کتاب"));
println!("{}", embeddings.sentence_similarity("این کتاب خوب است", "این کتاب بد است"));
```

## Docs

- [docs.rs](https://docs.rs/rishe)
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
