//! Persian NLP engine: normalization, tokenization, morphology, dates and model inference.

#![deny(missing_docs)]

/// Seeded keyboard, OCR and whitespace noise for training data.
pub mod augment;
/// Colloquial-to-formal word expansion.
pub mod colloquial;
/// Verb conjugation from past and present stems.
pub mod conjugation;
pub mod dates;
/// Joining tokens back into readable text.
pub mod detokenizer;
/// Read-only word and sentence embeddings from a memory-mapped vector file.
pub mod embeddings;
/// Rule-based ezafe marker prediction.
pub mod ezafe;
/// Model hub: cache paths, downloads and checksum verification.
pub mod hub;
/// Comparative, superlative, plural and possessive inflection.
pub mod inflector;
/// RAKE and TextRank keyword extraction.
pub mod keywords;
/// Masking of URLs, emails, IDs, numbers and stopwords.
pub mod maskers;
/// ONNX model packs for POS, NER, ezafe and dependency parsing.
#[cfg(feature = "models")]
pub mod models;
/// Lemma, verb-root and noun-table lookups.
pub mod morphology;
/// Individual normalization operators and their registry.
pub mod normalize_ops;
/// Full normalization pipeline and its configuration.
pub mod normalizer;
/// Sentence boundary detection.
pub mod sbd;
/// Frequency-ranked spelling suggestions and correction.
pub mod spell;
/// Light stemmer built on the morphology tables.
pub mod stemmer;
/// Bundled Persian stopword list.
pub mod stopwords;
/// Splitting fused tokens into morphemes.
pub mod token_splitter;
/// Byte-offset tokenizer.
pub mod tokenizer;
/// Persian and Tajik script transliteration.
pub mod transliterate;
/// Number-to-words and Persian character utilities.
pub mod utilities;
/// Word-level tokenization options and post-processing.
pub mod word_tokenizer;

use colloquial::{ColloquialConverter, ExpandedTokens};
use conjugation::Conjugation;
use detokenizer::Detokenizer;
use ezafe::RuleBasedEzafe;
use inflector::Inflector;
use morphology::MorphologyEngine;
use normalizer::{Normalizer, NormalizerConfig};
use rayon::prelude::*;
use sbd::SentenceSplitter;
use spell::SpellChecker;
use std::borrow::Cow;
use stemmer::Stemmer;
use token_splitter::TokenSplitter;
use tokenizer::{TokenKind, Tokenizer};

/// Result of one pipeline pass; every vector is aligned with `tokens` except `sentences`.
#[cfg_attr(feature = "python", pyo3::pyclass(frozen, get_all))]
#[derive(Debug, Clone)]
pub struct ProcessedDoc {
    /// Input after normalization, or the input unchanged when normalization is off.
    pub normalized_text: String,
    /// Detokenized formal text built from `tokens`.
    pub formal_text: String,
    /// Sentence strings; empty when the `sentences` stage is off.
    pub sentences: Vec<String>,
    /// Word and punctuation tokens after formalization.
    pub tokens: Vec<String>,
    /// Lemma per token; equals the token when lemmatization is off.
    pub lemmas: Vec<String>,
    /// One ezafe flag per token; empty when the ezafe stage is off.
    pub ezafe_markers: Vec<bool>,
}

/// Stage toggles for [`Pipeline::process_with`].
///
/// ```
/// use rishe::ProcessOptions;
///
/// let stages = ["normalize".to_string(), "sentences".to_string()];
/// let options = ProcessOptions::from_stages(&stages);
/// assert!(!options.lemmatize && options.sentences);
///
/// let unknown = ProcessOptions::from_stages_checked(&["bogus".to_string()]);
/// assert_eq!(unknown.unwrap_err(), "unknown process stage: bogus");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ProcessOptions {
    /// Run the normalizer before tokenizing.
    pub normalize: bool,
    /// Expand colloquial words to formal ones.
    pub formalize: bool,
    /// Compute a lemma for each word token.
    pub lemmatize: bool,
    /// Split the formal text into sentences.
    pub sentences: bool,
    /// Predict ezafe markers.
    pub ezafe: bool,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            normalize: true,
            formalize: true,
            lemmatize: true,
            sentences: true,
            ezafe: true,
        }
    }
}

impl ProcessOptions {
    /// Stage names [`ProcessOptions::from_stages`] understands, in pipeline order.
    pub const STAGES: [&'static str; 5] =
        ["normalize", "formalize", "lemmatize", "sentences", "ezafe"];

    /// Turn a list of stage names into flags. Unknown names are ignored here;
    /// a user-facing boundary should call [`ProcessOptions::from_stages_checked`].
    pub fn from_stages(stages: &[String]) -> Self {
        if stages.is_empty() {
            return Self::default();
        }
        let has = |name: &str| stages.iter().any(|stage| stage.as_str() == name);
        Self {
            normalize: has("normalize"),
            formalize: has("formalize"),
            lemmatize: has("lemmatize"),
            sentences: has("sentences"),
            ezafe: has("ezafe"),
        }
    }

    /// Like [`ProcessOptions::from_stages`], but an unknown stage name is an
    /// error instead of being ignored, so a typo cannot silently change the
    /// output. An empty list means every stage, the same as omitting `stages`.
    pub fn from_stages_checked(stages: &[String]) -> Result<Self, String> {
        match stages
            .iter()
            .find(|stage| !Self::STAGES.contains(&stage.as_str()))
        {
            Some(stage) => Err(format!("unknown process stage: {stage}")),
            None => Ok(Self::from_stages(stages)),
        }
    }
}

/// Reusable pipeline; build once and share, since model packs are loaded per instance.
#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
pub struct Pipeline {
    normalizer: Normalizer,
    morphology: MorphologyEngine,
    colloquial: ColloquialConverter,
    spell: SpellChecker,
    #[cfg(feature = "models")]
    pos: Option<models::ModelPack>,
    #[cfg(feature = "models")]
    ner: Option<models::ModelPack>,
    #[cfg(feature = "models")]
    ezafe: Option<models::ModelPack>,
    #[cfg(feature = "models")]
    depparse: Option<models::ModelPack>,
}

impl Pipeline {
    /// Build a pipeline with the given normalizer switches; model packs stay unloaded.
    ///
    /// ```
    /// use rishe::Pipeline;
    ///
    /// let ascii = Pipeline::create(false, false, false);
    /// let persian = Pipeline::create(true, true, true);
    /// assert_eq!(ascii.normalize_with("123", &[]).unwrap(), "123");
    /// assert_eq!(persian.normalize_with("123", &[]).unwrap(), "۱۲۳");
    /// ```
    pub fn create(unify_digits: bool, strip_diacritics: bool, fix_zwnj: bool) -> Self {
        let config = NormalizerConfig {
            unify_digits,
            strip_diacritics,
            fix_zwnj,
        };

        Self {
            normalizer: Normalizer::new(config),
            morphology: MorphologyEngine::new(),
            colloquial: ColloquialConverter::new(),
            spell: SpellChecker::new(),
            #[cfg(feature = "models")]
            pos: None,
            #[cfg(feature = "models")]
            ner: None,
            #[cfg(feature = "models")]
            ezafe: None,
            #[cfg(feature = "models")]
            depparse: None,
        }
    }

    /// Load a pack for `task` (`pos`, `ner`, `ezafe`, `depparse`), erroring when its
    /// task does not match.
    #[cfg(feature = "models")]
    pub fn load_pack(&mut self, task: &str, path: &str) -> Result<(), String> {
        let pack = models::ModelPack::load(path)?;
        if pack.task() != task {
            return Err(format!(
                "pack at {path} is {}, expected {task}",
                pack.task()
            ));
        }
        match task {
            "pos" => self.pos = Some(pack),
            "ner" => self.ner = Some(pack),
            "ezafe" => self.ezafe = Some(pack),
            "depparse" => self.depparse = Some(pack),
            other => return Err(format!("unknown pack task: {other}")),
        }
        Ok(())
    }

    #[cfg(feature = "models")]
    fn pack(&self, task: &str) -> Result<&models::ModelPack, String> {
        match task {
            "pos" => self.pos.as_ref(),
            "ner" => self.ner.as_ref(),
            "ezafe" => self.ezafe.as_ref(),
            "depparse" => self.depparse.as_ref(),
            other => return Err(format!("unknown pack task: {other}")),
        }
        .ok_or_else(|| format!("{task} pack is not loaded"))
    }

    /// POS-tag `text` with the loaded `pos` pack; errors when it is not loaded.
    #[cfg(feature = "models")]
    pub fn pos(&self, text: &str) -> Result<Vec<models::TaggedToken>, String> {
        self.pack("pos")?.tags(text)
    }

    /// Extract entities with the loaded `ner` pack; errors when it is not loaded.
    #[cfg(feature = "models")]
    pub fn ner(&self, text: &str) -> Result<Vec<models::Entity>, String> {
        self.pack("ner")?.entities(text)
    }

    /// Predict ezafe tags with the loaded `ezafe` pack; errors when it is not loaded.
    #[cfg(feature = "models")]
    pub fn ezafe(&self, text: &str) -> Result<Vec<models::TaggedToken>, String> {
        self.pack("ezafe")?.tags(text)
    }

    /// POS-tag each text; output order matches input order.
    #[cfg(feature = "models")]
    pub fn pos_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::TaggedToken>>, String> {
        self.pack("pos")?.tags_batch(texts)
    }

    /// Extract entities from each text; output order matches input order.
    #[cfg(feature = "models")]
    pub fn ner_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::Entity>>, String> {
        self.pack("ner")?.entities_batch(texts)
    }

    /// Predict ezafe tags for each text; output order matches input order.
    #[cfg(feature = "models")]
    pub fn ezafe_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::TaggedToken>>, String> {
        self.pack("ezafe")?.tags_batch(texts)
    }

    /// Parse dependencies with the loaded `depparse` pack; errors when it is not loaded.
    #[cfg(feature = "models")]
    pub fn depparse(&self, text: &str) -> Result<Vec<models::Dependency>, String> {
        self.pack("depparse")?.depparse(text)
    }

    /// Parse each text in parallel; output order matches input order.
    #[cfg(feature = "models")]
    pub fn depparse_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::Dependency>>, String> {
        texts.par_iter().map(|text| self.depparse(text)).collect()
    }

    /// Run every stage with [`ProcessOptions::default`].
    ///
    /// ```
    /// use rishe::Pipeline;
    ///
    /// let nlp = Pipeline::create(true, true, true);
    /// let text = "من در سال 2026 به کتاب   ها نگاه مي كنم چون سرعتش بالاست!";
    /// let doc = nlp.process(text);
    /// assert_eq!(doc.formal_text, "من در سال ۲۰۲۶ به کتاب‌ها نگاه می‌کنم چون سرعتش بالا است!");
    /// assert_eq!(doc.tokens.len(), doc.lemmas.len());
    /// ```
    #[inline]
    pub fn process(&self, text: &str) -> ProcessedDoc {
        self.process_with(text, &ProcessOptions::default())
    }

    /// Run the selected stages; all vectors are aligned with `tokens` except `sentences`.
    pub fn process_with(&self, text: &str, opts: &ProcessOptions) -> ProcessedDoc {
        let norm_cow = if opts.normalize {
            self.normalizer.normalize(text)
        } else {
            Cow::Borrowed(text)
        };
        let raw_tokens = Tokenizer::tokenize(&norm_cow);

        let mut tokens = Vec::with_capacity(raw_tokens.len() + 4);
        let mut lemmas = Vec::with_capacity(raw_tokens.len() + 4);

        for tok in raw_tokens {
            if tok.kind == TokenKind::Word && opts.formalize {
                let mut push_token = |token: Cow<'_, str>| {
                    let lemma = if opts.lemmatize {
                        self.morphology.lemmatize(&token).into_owned()
                    } else {
                        token.to_string()
                    };
                    tokens.push(token.into_owned());
                    lemmas.push(lemma);
                };
                match self
                    .colloquial
                    .expand_and_formalize(tok.text, &self.morphology)
                {
                    ExpandedTokens::Single(formal_tok) => {
                        let fixed_tok = self.morphology.fix_spaceless_verb(&formal_tok);
                        push_token(fixed_tok);
                    }
                    ExpandedTokens::Pair(first, second) => {
                        push_token(first);
                        push_token(second);
                    }
                }
            } else if tok.kind == TokenKind::Word && opts.lemmatize {
                lemmas.push(self.morphology.lemmatize(tok.text).into_owned());
                tokens.push(tok.text.to_string());
            } else {
                tokens.push(tok.text.to_string());
                lemmas.push(tok.text.to_string());
            }
        }

        let formal_text = Detokenizer::detokenize(&tokens);
        let sentences = if opts.sentences {
            let source = if tokens.is_empty() {
                norm_cow.as_ref()
            } else {
                formal_text.as_str()
            };
            SentenceSplitter::split(source)
                .into_iter()
                .map(str::to_owned)
                .collect()
        } else {
            Vec::new()
        };

        let ezafe_markers = if opts.ezafe {
            RuleBasedEzafe::predict(&tokens, &self.morphology)
        } else {
            Vec::new()
        };

        ProcessedDoc {
            normalized_text: norm_cow.into_owned(),
            formal_text,
            sentences,
            tokens,
            lemmas,
            ezafe_markers,
        }
    }

    /// Run [`Pipeline::process`] over the texts in parallel, preserving input order.
    pub fn process_batch(&self, texts: &[String]) -> Vec<ProcessedDoc> {
        texts.par_iter().map(|t| self.process(t)).collect()
    }

    /// Normalize with an explicit, ordered list of operator names.
    ///
    /// ```
    /// use rishe::Pipeline;
    ///
    /// let nlp = Pipeline::create(true, true, true);
    /// let stages = ["digits".to_string()];
    /// assert_eq!(nlp.normalize_with("1𝟮3٤٥⓺", &stages).unwrap(), "۱۲۳۴۵۶");
    /// ```
    pub fn normalize_with(&self, text: &str, stages: &[String]) -> Result<String, String> {
        let mut ops = Vec::with_capacity(stages.len());
        for stage in stages {
            match normalize_ops::NormalizeOp::from_name(stage) {
                Some(op) => ops.push(op),
                None => return Err(format!("unknown normalize stage: {stage}")),
            }
        }
        if ops.is_empty() {
            return Ok(self.normalizer.normalize(text).into_owned());
        }
        let mut current = text.to_owned();
        for op in ops {
            current = normalize_ops::apply(&current, op, &self.morphology).into_owned();
        }
        Ok(current)
    }

    /// Apply maskers in order; an unknown name is an error and `None` uses each
    /// masker's default placeholder.
    ///
    /// ```
    /// use rishe::Pipeline;
    ///
    /// let nlp = Pipeline::create(true, true, true);
    /// let maskers = ["url".to_string(), "email".to_string()];
    /// let masked = nlp.mask("به https://a.b و info@a.co ایمیل بزن", &maskers, None);
    /// assert_eq!(masked.unwrap(), "به <URL> و <EMAIL> ایمیل بزن");
    /// ```
    pub fn mask(
        &self,
        text: &str,
        maskers: &[String],
        mask_token: Option<&str>,
    ) -> Result<String, String> {
        let mut ops = Vec::with_capacity(maskers.len());
        for name in maskers {
            match maskers::MaskOp::from_name(name) {
                Some(op) => ops.push(op),
                None => return Err(format!("unknown masker: {name}")),
            }
        }
        let mut current = Cow::Borrowed(text);
        for op in ops {
            current = Cow::Owned(op.apply(&current, mask_token));
        }
        Ok(current.into_owned())
    }

    /// Stem one word, falling back to the lemma when no noun suffix matches.
    ///
    /// ```
    /// use rishe::Pipeline;
    ///
    /// let nlp = Pipeline::create(true, true, true);
    /// assert_eq!(nlp.stem("کتاب‌ها"), "کتاب");
    /// ```
    pub fn stem(&self, word: &str) -> String {
        Stemmer::stem(word, &self.morphology).into_owned()
    }

    /// All forms of a known verb, or `None` when the verb is not in the table.
    pub fn conjugate(&self, verb: &str) -> Option<Conjugation> {
        self.morphology.conjugate(verb)
    }

    /// Lemma plus the present stem of the lemma when it is a verb.
    pub fn lemmatize_verb(&self, word: &str) -> (String, Option<String>) {
        let lemma = self.morphology.lemmatize(word).into_owned();
        let present = self.morphology.present_stem(&lemma).map(str::to_owned);
        (lemma, present)
    }

    /// Split one fused token into morphemes.
    pub fn split_token(&self, token: &str) -> Vec<String> {
        TokenSplitter::split(token, &self.morphology)
    }

    /// Normalized spelling suggestions for one word.
    pub fn spell_suggest(&self, word: &str, max: usize) -> Vec<String> {
        let normalized = self.normalizer.normalize(word);
        self.spell.suggest(&normalized, max)
    }

    /// Normalize then correct unknown words of three or more characters.
    pub fn spell_correct(&self, text: &str) -> String {
        let normalized = self.normalizer.normalize(text);
        self.spell.correct(&normalized)
    }

    /// Comparative form of an adjective.
    pub fn comparative(&self, adj: &str) -> String {
        Inflector::comparative(adj)
    }

    /// Superlative form of an adjective.
    pub fn superlative(&self, adj: &str) -> String {
        Inflector::superlative(adj)
    }

    /// Plural form of a noun.
    pub fn plural(&self, noun: &str) -> String {
        Inflector::plural(noun)
    }

    /// Possessive form of a noun for person `0..=5`, or `None` when out of range.
    pub fn possessive(&self, noun: &str, person: usize) -> Option<String> {
        Inflector::possessive(noun, person)
    }

    /// Tokens with byte spans into `text`.
    pub fn tokenize_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
        Tokenizer::tokenize(text)
            .into_iter()
            .map(|token| (token.text.to_owned(), token.start, token.end))
            .collect()
    }

    /// Tokens with character spans into `text`.
    pub fn tokenize_char_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
        let mut byte_end = 0;
        let mut char_end = 0;
        Tokenizer::tokenize(text)
            .into_iter()
            .map(|token| {
                // Count the gap between tokens once, so this stays linear in `text`.
                let start = char_end + text[byte_end..token.start].chars().count();
                let end = start + token.text.chars().count();
                byte_end = token.end;
                char_end = end;
                (token.text.to_owned(), start, end)
            })
            .collect()
    }

    /// Tokenize into surface words. This does not normalize: call
    /// [`Pipeline::normalize_with`] with an empty stage list first (or use
    /// [`Pipeline::process`], which runs the whole pipeline including normalization).
    ///
    /// ```
    /// use rishe::{word_tokenizer::WordTokenizerOptions, Pipeline};
    ///
    /// let nlp = Pipeline::create(true, true, true);
    /// let options = WordTokenizerOptions::default();
    /// assert_eq!(nlp.tokenize("سلام دنیا", &options), ["سلام", "دنیا"]);
    /// ```
    pub fn tokenize(
        &self,
        text: &str,
        options: &word_tokenizer::WordTokenizerOptions,
    ) -> Vec<String> {
        let pre = word_tokenizer::apply_pre(text, options);
        let pre = if options.replace_numbers {
            Cow::Owned(word_tokenizer::replace_numbers(&pre))
        } else {
            Cow::Borrowed(pre.as_str())
        };
        let tokens: Vec<String> = Tokenizer::tokenize(pre.as_ref())
            .into_iter()
            .map(|token| token.text.to_owned())
            .collect();
        word_tokenizer::postprocess(tokens, options, &self.morphology)
    }

    /// Top `top_n` RAKE keywords of at most `max_length` words, after normalization.
    pub fn rake(&self, text: &str, top_n: usize, max_length: usize) -> Vec<String> {
        let normalized = self.normalizer.normalize(text);
        keywords::rake(&normalized, top_n, max_length)
    }

    /// Top `top_n` TextRank keywords within a `window`-token co-occurrence window,
    /// after normalization.
    pub fn textrank(
        &self,
        text: &str,
        top_n: usize,
        max_length: usize,
        window: usize,
    ) -> Vec<String> {
        let normalized = self.normalizer.normalize(text);
        keywords::textrank(&normalized, top_n, max_length, window)
    }

    /// All word n-grams for `min_n..=max_n` after normalization; errors when the
    /// range is empty or reversed.
    pub fn ngrams(&self, text: &str, min_n: usize, max_n: usize) -> Result<Vec<String>, String> {
        if min_n == 0 || max_n < min_n {
            return Err("ngram range must be (min, max) with 1 <= min <= max".to_string());
        }
        let normalized = self.normalizer.normalize(text);
        let tokens = self.tokenize(
            &normalized,
            &word_tokenizer::WordTokenizerOptions::default(),
        );
        let mut out = Vec::new();
        for n in min_n..=max_n {
            if n > tokens.len() {
                break;
            }
            out.extend(tokens.windows(n).map(|window| window.join(" ")));
        }
        Ok(out)
    }
}

#[cfg(feature = "python")]
mod py_bindings {
    #![allow(clippy::useless_conversion)]

    use super::*;
    use pyo3::prelude::*;
    use std::path::PathBuf;

    #[pyclass(frozen, get_all, module = "rishe._core")]
    #[derive(Clone, Copy)]
    pub struct JalaliDate {
        pub year: i32,
        pub month: u8,
        pub day: u8,
    }

    impl JalaliDate {
        fn as_date(self) -> dates::Jalali {
            dates::Jalali {
                year: self.year,
                month: self.month,
                day: self.day,
            }
        }
    }

    #[pymethods]
    impl JalaliDate {
        #[new]
        pub fn py_new(year: i32, month: u8, day: u8) -> PyResult<Self> {
            dates::Jalali::new(year, month, day)
                .map(|date| Self {
                    year: date.year,
                    month: date.month,
                    day: date.day,
                })
                .ok_or_else(|| {
                    pyo3::exceptions::PyValueError::new_err(format!(
                        "invalid Jalali date: {year}-{month}-{day}"
                    ))
                })
        }

        #[staticmethod]
        pub fn parse(text: &str) -> Option<Self> {
            dates::parse(text).map(|date| Self {
                year: date.year,
                month: date.month,
                day: date.day,
            })
        }

        #[staticmethod]
        pub fn from_gregorian(year: i32, month: u8, day: u8) -> Option<Self> {
            dates::Jalali::from_gregorian(dates::Gregorian { year, month, day }).map(|date| Self {
                year: date.year,
                month: date.month,
                day: date.day,
            })
        }

        #[getter]
        pub fn iso(&self) -> Option<String> {
            self.as_date().to_iso()
        }

        #[allow(clippy::wrong_self_convention)]
        pub fn to_gregorian(&self) -> Option<(i32, u8, u8)> {
            self.as_date()
                .to_gregorian()
                .map(|date| (date.year, date.month, date.day))
        }

        pub fn format_persian(&self) -> String {
            self.as_date().format_persian()
        }

        pub fn format_numeric(&self) -> String {
            self.as_date().format_numeric()
        }

        fn __repr__(&self) -> String {
            format!("JalaliDate({}, {}, {})", self.year, self.month, self.day)
        }
    }

    #[pymethods]
    impl Pipeline {
        /// Python constructor: pack paths are loaded only with the `models` feature.
        #[new]
        #[pyo3(signature = (unify_digits=true, strip_diacritics=true, fix_zwnj=true, pos_pack=None, ner_pack=None, ezafe_pack=None, depparse_pack=None))]
        #[allow(unused_variables)]
        pub fn py_new(
            unify_digits: bool,
            strip_diacritics: bool,
            fix_zwnj: bool,
            pos_pack: Option<PathBuf>,
            ner_pack: Option<PathBuf>,
            ezafe_pack: Option<PathBuf>,
            depparse_pack: Option<PathBuf>,
        ) -> PyResult<Self> {
            #[cfg(feature = "models")]
            let mut pipe = Self::create(unify_digits, strip_diacritics, fix_zwnj);
            #[cfg(not(feature = "models"))]
            let pipe = Self::create(unify_digits, strip_diacritics, fix_zwnj);

            #[cfg(feature = "models")]
            {
                for (task, path) in [
                    ("pos", pos_pack),
                    ("ner", ner_pack),
                    ("ezafe", ezafe_pack),
                    ("depparse", depparse_pack),
                ] {
                    if let Some(path) = path {
                        pipe.load_pack(task, &path.to_string_lossy())
                            .map_err(pyo3::exceptions::PyValueError::new_err)?;
                    }
                }
            }
            #[cfg(not(feature = "models"))]
            {
                if pos_pack.is_some()
                    || ner_pack.is_some()
                    || ezafe_pack.is_some()
                    || depparse_pack.is_some()
                {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "model packs require the `models` feature",
                    ));
                }
            }
            Ok(pipe)
        }

        /// Python `pos`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "pos")]
        pub fn py_pos(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.pos(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `ner`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "ner")]
        pub fn py_ner(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Entity>> {
            py.allow_threads(|| self.ner(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `ezafe`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "ezafe")]
        pub fn py_ezafe(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.ezafe(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `pos_batch`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "pos_batch")]
        pub fn py_pos_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::TaggedToken>>> {
            py.allow_threads(|| self.pos_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `ner_batch`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "ner_batch")]
        pub fn py_ner_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::Entity>>> {
            py.allow_threads(|| self.ner_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `ezafe_batch`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "ezafe_batch")]
        pub fn py_ezafe_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::TaggedToken>>> {
            py.allow_threads(|| self.ezafe_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `depparse`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "depparse")]
        pub fn py_depparse(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Dependency>> {
            py.allow_threads(|| self.depparse(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `depparse_batch`: releases the GIL and maps errors to `ValueError`.
        #[cfg(feature = "models")]
        #[pyo3(name = "depparse_batch")]
        pub fn py_depparse_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::Dependency>>> {
            py.allow_threads(|| self.depparse_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `__call__`: runs [`Pipeline::process`] with the GIL released.
        pub fn __call__(&self, py: Python<'_>, text: &str) -> ProcessedDoc {
            py.allow_threads(|| self.process(text))
        }

        /// Python `process_batch`; the GIL is released.
        #[pyo3(name = "process_batch")]
        pub fn py_process_batch(&self, py: Python<'_>, texts: Vec<String>) -> Vec<ProcessedDoc> {
            py.allow_threads(|| self.process_batch(&texts))
        }

        /// Python `process`: optional `stages` select the stages and unknown names raise
        /// `ValueError`.
        #[pyo3(name = "process", signature = (text, stages=None))]
        pub fn py_process(
            &self,
            py: Python<'_>,
            text: &str,
            stages: Option<Vec<String>>,
        ) -> PyResult<ProcessedDoc> {
            match stages {
                Some(stages) => {
                    let opts = ProcessOptions::from_stages_checked(&stages)
                        .map_err(pyo3::exceptions::PyValueError::new_err)?;
                    Ok(py.allow_threads(|| self.process_with(text, &opts)))
                }
                None => Ok(py.allow_threads(|| self.process(text))),
            }
        }

        /// Python `mask`: maps errors to `ValueError`.
        #[pyo3(name = "mask", signature = (text, maskers, mask_token=None))]
        pub fn py_mask(
            &self,
            text: &str,
            maskers: Vec<String>,
            mask_token: Option<String>,
        ) -> PyResult<String> {
            self.mask(text, &maskers, mask_token.as_deref())
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `sent_tokenize`: one string per sentence.
        pub fn sent_tokenize(&self, text: &str) -> Vec<String> {
            SentenceSplitter::split(text)
                .into_iter()
                .map(str::to_owned)
                .collect()
        }

        /// Python `normalize`: applies `stages` in order when given, else the full normalizer.
        #[pyo3(signature = (text, stages=None))]
        pub fn normalize(&self, text: &str, stages: Option<Vec<String>>) -> PyResult<String> {
            match stages {
                Some(stages) => self
                    .normalize_with(text, &stages)
                    .map_err(pyo3::exceptions::PyValueError::new_err),
                None => Ok(self.normalizer.normalize(text).into_owned()),
            }
        }

        /// Python `tokenize`: surface tokens with the word-tokenizer switches.
        #[pyo3(name = "tokenize")]
        #[pyo3(
            signature = (
                text,
                join_verb_parts=false,
                join_abbreviations=false,
                separate_emoji=false,
                replace_links=false,
                replace_ids=false,
                replace_emails=false,
                replace_numbers=false,
                replace_hashtags=false
            )
        )]
        #[allow(clippy::too_many_arguments)]
        pub fn py_tokenize(
            &self,
            text: &str,
            join_verb_parts: bool,
            join_abbreviations: bool,
            separate_emoji: bool,
            replace_links: bool,
            replace_ids: bool,
            replace_emails: bool,
            replace_numbers: bool,
            replace_hashtags: bool,
        ) -> Vec<String> {
            let options = word_tokenizer::WordTokenizerOptions {
                join_verb_parts,
                join_abbreviations,
                separate_emoji,
                replace_links,
                replace_ids,
                replace_emails,
                replace_numbers,
                replace_hashtags,
            };
            Pipeline::tokenize(self, text, &options)
        }

        /// Python `tokenize_spans`: tokens with character offsets.
        #[pyo3(name = "tokenize_spans")]
        pub fn py_tokenize_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
            self.tokenize_char_spans(text)
        }

        /// Python `number_to_words`: accepts an int or digit string and raises
        /// `TypeError` otherwise.
        #[pyo3(name = "number_to_words")]
        pub fn py_number_to_words(
            &self,
            value: &pyo3::Bound<'_, pyo3::types::PyAny>,
        ) -> PyResult<String> {
            if let Ok(number) = value.extract::<i64>() {
                return Ok(utilities::number_to_words(number));
            }
            if value.is_instance_of::<pyo3::types::PyInt>() {
                return Err(pyo3::exceptions::PyOverflowError::new_err(
                    "number out of range for i64",
                ));
            }
            if let Ok(text) = value.extract::<String>() {
                return utilities::number_to_words_text(&text)
                    .map_err(pyo3::exceptions::PyValueError::new_err);
            }
            Err(pyo3::exceptions::PyTypeError::new_err(
                "expected int or str",
            ))
        }

        /// Python `ngrams`: maps range errors to `ValueError`.
        #[pyo3(name = "ngrams", signature = (text, min_n=1, max_n=1))]
        pub fn py_ngrams(&self, text: &str, min_n: usize, max_n: usize) -> PyResult<Vec<String>> {
            self.ngrams(text, min_n, max_n)
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `rake`: top `n` keywords, normalized first.
        #[pyo3(name = "rake", signature = (text, n=5, max_length=3))]
        pub fn py_rake(&self, text: &str, n: usize, max_length: usize) -> Vec<String> {
            self.rake(text, n, max_length)
        }

        /// Python `textrank`: top `n` keywords, normalized first.
        #[pyo3(name = "textrank", signature = (text, n=5, max_length=3, window=2))]
        pub fn py_textrank(
            &self,
            text: &str,
            n: usize,
            max_length: usize,
            window: usize,
        ) -> Vec<String> {
            self.textrank(text, n, max_length, window)
        }

        /// Python `keyboard_noise`: default options with the optional `seed`.
        #[pyo3(name = "keyboard_noise", signature = (text, seed=None))]
        pub fn py_keyboard_noise(&self, text: &str, seed: Option<u64>) -> String {
            augment::keyboard_noise(
                text,
                augment::KeyboardNoiseOptions {
                    seed,
                    ..Default::default()
                },
            )
        }

        /// Python `ocr_noise`: default options with the optional `seed`.
        #[pyo3(name = "ocr_noise", signature = (text, seed=None))]
        pub fn py_ocr_noise(&self, text: &str, seed: Option<u64>) -> String {
            augment::ocr_noise(
                text,
                augment::OcrNoiseOptions {
                    seed,
                    ..Default::default()
                },
            )
        }

        /// Python `whitespace_noise`: default options with the optional `seed`.
        #[pyo3(name = "whitespace_noise", signature = (text, seed=None))]
        pub fn py_whitespace_noise(&self, text: &str, seed: Option<u64>) -> String {
            augment::whitespace_noise(
                text,
                augment::WhitespaceNoiseOptions {
                    seed,
                    ..Default::default()
                },
            )
        }

        /// Python `to_tajik`: Persian letters to Tajik Cyrillic.
        #[pyo3(name = "to_tajik")]
        pub fn py_to_tajik(&self, text: &str) -> String {
            transliterate::to_tajik(text)
        }

        /// Python `to_persian`: Tajik Cyrillic to Persian letters.
        #[pyo3(name = "to_persian")]
        pub fn py_to_persian(&self, text: &str) -> String {
            transliterate::to_persian(text)
        }

        /// Python `persianize`: replaces known loanwords with Persian equivalents.
        #[pyo3(name = "persianize")]
        pub fn py_persianize(&self, text: &str) -> String {
            utilities::persianize(text)
        }

        /// Python `persianize_suggest`: loanword matches as `(word, alternatives, start, end)`.
        #[pyo3(name = "persianize_suggest")]
        pub fn py_persianize_suggest(
            &self,
            text: &str,
        ) -> Vec<(String, Vec<String>, usize, usize)> {
            utilities::persianize_suggest(text)
                .into_iter()
                .map(|item| (item.word, item.alternatives, item.start, item.end))
                .collect()
        }

        /// Python `lemmatize`: lemma of one word.
        pub fn lemmatize(&self, word: &str) -> String {
            self.morphology.lemmatize(word).into_owned()
        }

        /// Python `lemmatize_verb`: lemma plus present stem when the word is a verb.
        #[pyo3(name = "lemmatize_verb")]
        pub fn py_lemmatize_verb(&self, word: &str) -> (String, Option<String>) {
            self.lemmatize_verb(word)
        }

        /// Python `stem`: stems one word.
        #[pyo3(name = "stem")]
        pub fn py_stem(&self, word: &str) -> String {
            self.stem(word)
        }

        /// Python `conjugate`: forms as a map, or `None` for an unknown verb.
        #[pyo3(name = "conjugate")]
        pub fn py_conjugate(
            &self,
            verb: &str,
        ) -> Option<std::collections::HashMap<String, Vec<String>>> {
            self.conjugate(verb).map(Conjugation::into_map)
        }

        /// Python `split_token`: morphemes of one fused token.
        #[pyo3(name = "split_token")]
        pub fn py_split_token(&self, token: &str) -> Vec<String> {
            self.split_token(token)
        }

        /// Python `spell_suggest`: up to `n` candidates.
        #[pyo3(name = "spell_suggest", signature = (word, n=5))]
        pub fn py_spell_suggest(&self, word: &str, n: usize) -> Vec<String> {
            self.spell_suggest(word, n)
        }

        /// Python `spell_correct`: normalizes then corrects the text.
        #[pyo3(name = "spell_correct")]
        pub fn py_spell_correct(&self, text: &str) -> String {
            self.spell_correct(text)
        }

        /// Python `comparative`: comparative form of an adjective.
        #[pyo3(name = "comparative")]
        pub fn py_comparative(&self, adj: &str) -> String {
            self.comparative(adj)
        }

        /// Python `superlative`: superlative form of an adjective.
        #[pyo3(name = "superlative")]
        pub fn py_superlative(&self, adj: &str) -> String {
            self.superlative(adj)
        }

        /// Python `plural`: plural form of a noun.
        #[pyo3(name = "plural")]
        pub fn py_plural(&self, noun: &str) -> String {
            self.plural(noun)
        }

        /// Python `possessive`: possessive form for person `0..=5`, or `None`.
        #[pyo3(name = "possessive")]
        pub fn py_possessive(&self, noun: &str, person: isize) -> Option<String> {
            self.possessive(noun, usize::try_from(person).ok()?)
        }

        /// Python `is_stopword`: whether the word is in the bundled list.
        pub fn is_stopword(&self, word: &str) -> bool {
            stopwords::is_stopword(word)
        }

        /// Python `remove_stopwords`: drops stopword tokens, keeping the rest in order.
        pub fn remove_stopwords(&self, tokens: Vec<String>) -> Vec<String> {
            stopwords::remove_stopwords(tokens)
        }

        /// Python `stopwords`: every bundled stopword.
        pub fn stopwords(&self) -> Vec<String> {
            stopwords::stopwords()
        }
    }

    #[pymethods]
    impl embeddings::Embeddings {
        /// Python constructor: loads vectors from `path`.
        #[new]
        pub fn py_new(path: PathBuf) -> PyResult<Self> {
            Self::load(path).map_err(pyo3::exceptions::PyIOError::new_err)
        }

        fn __len__(&self) -> usize {
            self.len()
        }

        #[getter(dim)]
        fn py_dim(&self) -> usize {
            self.dim()
        }

        /// Storage width of the file the vectors were loaded from: "f32" or "f16".
        #[getter(dtype)]
        fn py_dtype(&self) -> &'static str {
            self.dtype().as_str()
        }

        /// Python `contains`: vocabulary membership.
        #[pyo3(name = "contains")]
        pub fn py_contains(&self, word: &str) -> bool {
            self.contains(word)
        }

        /// Python `vector`: a copy of the row, or `None` for an unknown word.
        #[pyo3(name = "vector")]
        pub fn py_vector(&self, word: &str) -> Option<Vec<f32>> {
            self.vector(word).map(<[f32]>::to_vec)
        }

        /// Python `similarity`: cosine similarity, or `None` when either word is unknown.
        #[pyo3(name = "similarity")]
        pub fn py_similarity(&self, first: &str, second: &str) -> Option<f32> {
            self.similarity(first, second)
        }

        /// Python `sentence_vector`: mean vector of the known words.
        #[pyo3(name = "sentence_vector")]
        pub fn py_sentence_vector(&self, text: &str) -> Vec<f32> {
            self.sentence_vector(text)
        }

        /// Python `sentence_vectors`: releases the GIL while averaging each text.
        #[pyo3(name = "sentence_vectors")]
        pub fn py_sentence_vectors(&self, py: Python<'_>, texts: Vec<String>) -> Vec<Vec<f32>> {
            py.allow_threads(|| self.sentence_vectors(&texts))
        }

        /// Python `sentence_similarity`: cosine similarity of the two sentence vectors.
        #[pyo3(name = "sentence_similarity")]
        pub fn py_sentence_similarity(&self, first: &str, second: &str) -> f32 {
            self.sentence_similarity(first, second)
        }

        /// Python `most_similar`: up to `n` neighbours by descending cosine similarity.
        #[pyo3(name = "most_similar", signature = (word, n=5))]
        pub fn py_most_similar(&self, word: &str, n: usize) -> Vec<(String, f32)> {
            self.most_similar(word, n)
        }
    }

    /// Result spans expose ``label`` regardless of the task, so user code can
    /// loop over POS, NER and dependency output the same way
    /// (`docs/API_CONVENTIONS.md` rule 4).
    #[cfg(feature = "models")]
    #[pymethods]
    impl models::TaggedToken {
        #[getter(label)]
        fn py_label(&self) -> &str {
            &self.tag
        }
    }

    #[cfg(feature = "models")]
    #[pymethods]
    impl models::Dependency {
        #[getter(label)]
        fn py_label(&self) -> &str {
            &self.relation
        }
    }

    #[cfg(feature = "models")]
    #[pymethods]
    impl models::ModelPack {
        /// Python constructor: loads a pack directory.
        #[new]
        pub fn py_new(path: PathBuf) -> PyResult<Self> {
            Self::load(path).map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `tags_from_words`: tags for pre-tokenized words.
        #[pyo3(name = "tags_from_words")]
        pub fn py_tags_from_words(
            &self,
            py: Python<'_>,
            words: Vec<String>,
        ) -> PyResult<Vec<(String, String)>> {
            let words: Vec<&str> = words.iter().map(String::as_str).collect();
            py.allow_threads(|| self.tags_from_words(&words))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `tags`: tagged tokens for `text`, with the GIL released.
        #[pyo3(name = "tags")]
        pub fn py_tags(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.tags(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `entities`: entities for `text`, with the GIL released.
        #[pyo3(name = "entities")]
        pub fn py_entities(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Entity>> {
            py.allow_threads(|| self.entities(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `tags_batch`: tagged tokens per text, with the GIL released.
        #[pyo3(name = "tags_batch")]
        pub fn py_tags_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::TaggedToken>>> {
            py.allow_threads(|| self.tags_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `entities_batch`: entities per text, with the GIL released.
        #[pyo3(name = "entities_batch")]
        pub fn py_entities_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::Entity>>> {
            py.allow_threads(|| self.entities_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        /// Python `labels`: label names as owned strings.
        #[pyo3(name = "labels")]
        pub fn py_labels(&self) -> Vec<String> {
            self.labels().to_vec()
        }

        /// Python `task`: the pack task.
        #[getter(task)]
        pub fn py_task(&self) -> &str {
            self.task()
        }

        /// Python `max_len`: maximum token count the pack accepts.
        #[getter(max_len)]
        pub fn py_max_len(&self) -> usize {
            self.max_len()
        }
    }

    #[pymodule]
    pub fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("DEFAULT_MASKERS", crate::maskers::DEFAULT_MASKERS.to_vec())?;
        m.add_class::<Pipeline>()?;
        m.add_class::<ProcessedDoc>()?;
        m.add_class::<JalaliDate>()?;
        m.add_class::<embeddings::Embeddings>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::ModelPack>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::TaggedToken>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::Entity>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::Dependency>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end_pipeline() {
        let nlp = Pipeline::create(true, true, true);
        let doc = nlp
            .process("من در سال 2026 به کتاب   ها نگاه مي كنم چون سرعتش بالاست! نظر د. احمدي چيه؟");
        assert_eq!(
            doc.formal_text,
            "من در سال ۲۰۲۶ به کتاب\u{200C}ها نگاه می\u{200C}کنم چون سرعتش بالا است! نظر د. احمدی چیست؟"
        );
        assert_eq!(doc.sentences.len(), 2);
    }

    #[test]
    fn test_tokenizer_and_lemma_fixes() {
        let nlp = Pipeline::create(true, true, true);

        fn tokens(text: &str) -> Vec<&str> {
            Tokenizer::tokenize(text)
                .into_iter()
                .map(|t| t.text)
                .collect()
        }
        assert_eq!(tokens("در سال 2026."), ["در", "سال", "2026", "."]);
        assert_eq!(tokens("نظر dsm4 و A1C"), ["نظر", "dsm4", "و", "A1C"]);

        let lemma = |word: &str| nlp.morphology.lemmatize(word).into_owned();
        assert_eq!(lemma("کرده"), "کرد");
        assert_eq!(lemma("بودند"), "بود");
        assert_eq!(lemma("نوشت"), "نوشت");
        assert_eq!(lemma("ش"), "او");
        assert_eq!(lemma("همهٔ"), "همه");
        assert_eq!(lemma("اخیراً"), "اخیراً");
        assert_eq!(lemma("20"), "20");
    }

    #[test]
    fn test_stemming() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(nlp.stem("کتاب‌ها"), "کتاب");
        assert_eq!(nlp.stem("خانه‌هایی"), "خانه");
        assert_eq!(nlp.stem("می‌روم"), "رفت");
        assert_eq!(nlp.stem("تنها"), "تنها");
    }

    #[test]
    fn test_conjugation() {
        let nlp = Pipeline::create(true, true, true);
        let forms = nlp.conjugate("رفت").expect("رفت is a known verb");
        assert!(forms.past.contains(&"رفتم".to_string()));
        assert!(forms.present.contains(&"می‌روم".to_string()));
        assert!(forms.subjunctive.contains(&"بروم".to_string()));
        assert!(forms.future.contains(&"خواهم رفت".to_string()));
        assert!(forms.perfect.contains(&"رفته‌ام".to_string()));

        let bood = nlp.conjugate("بود").expect("بود is a known verb");
        assert!(bood.subjunctive.contains(&"باشم".to_string()));
    }

    #[test]
    fn test_inflection() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(nlp.comparative("خوب"), "بهتر");
        assert_eq!(nlp.comparative("بزرگ"), "بزرگ‌تر");
        assert_eq!(nlp.superlative("بزرگ"), "بزرگ‌ترین");
        assert_eq!(nlp.plural("کتاب"), "کتاب‌ها");
        assert_eq!(nlp.possessive("خانه", 0), Some("خانه‌ام".to_string()));
        assert_eq!(nlp.possessive("دنیا", 0), Some("دنیایم".to_string()));
        assert_eq!(nlp.possessive("پا", 3), Some("پایمان".to_string()));
        assert_eq!(nlp.possessive("بازو", 2), Some("بازویش".to_string()));
        assert_eq!(nlp.comparative(""), "");
        assert_eq!(nlp.superlative(""), "");
        assert_eq!(nlp.plural(""), "");
        assert_eq!(nlp.possessive("", 0), Some(String::new()));
        assert_eq!(nlp.possessive("", 6), None);
    }

    #[test]
    fn test_token_split() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(nlp.split_token("کتاب‌ها"), ["کتاب", "ها"]);
        assert_eq!(nlp.split_token("بهش"), ["به", "او"]);
        assert_eq!(nlp.split_token("می‌روم"), ["می", "روم"]);
        assert!(nlp.split_token("").is_empty());
    }

    #[test]
    fn ordinary_words_ending_in_copula_are_left_alone() {
        // These are words, not a stem plus `است`. The old rule appended a `ه` and
        // split them, and the lexicon's one-off entries made the result "valid".
        let nlp = Pipeline::create(true, true, true);
        for text in [
            "کتاب خوب است",
            "دست من",
            "هست",
            "نیست",
            "چیست",
            "درست",
            "مست",
        ] {
            assert_eq!(
                nlp.process(text).formal_text,
                text,
                "{text:?} must not be split"
            );
        }

        // A real copula still expands, and the stem is the base, not base + `ه`.
        assert_eq!(nlp.process("بالاست").formal_text, "بالا است");
        assert_eq!(nlp.process("کتاب\u{200C}ست").formal_text, "کتاب است");
        assert_eq!(nlp.process("کتابست").formal_text, "کتاب است");
        assert_eq!(nlp.process("عالیست").formal_text, "عالی است");
    }

    #[test]
    fn known_whole_words_are_not_split_on_a_prefix() {
        // `بر`+`نامه` and `در`+`خت` are both lexicon words, so the prefix loop used
        // to win before the whole-word check; these are ordinary words.
        let nlp = Pipeline::create(true, true, true);
        for word in ["برنامه", "درخت", "بازار", "واژه", "دریا", "فروشگاه"]
        {
            assert_eq!(nlp.split_token(word), [word], "{word:?} must stay whole");
        }
        // The documented splits still work.
        assert_eq!(nlp.split_token("بهش"), ["به", "او"]);
        assert_eq!(nlp.split_token("کتاب\u{200C}ها"), ["کتاب", "ها"]);
        // `توت` is mulberry, not `در` + `تو`.
        assert_eq!(nlp.process("توت را خوردم").tokens, ["توت", "را", "خوردم"]);
        assert_eq!(nlp.process("بهم گفت").formal_text, "به من گفت");
    }

    #[test]
    fn lexicalized_nouns_ending_in_he_are_not_participle_stems() {
        // The stoplist keeps a noun from lemmatizing through a verb stem that only
        // looks like the rest of the word.
        let morph = morphology::MorphologyEngine::new();
        for (word, wrong) in [
            ("دانه", "دانست"),
            ("هسته", "بود"),
            ("آیه", "آمد"),
            ("گوشه", "گوشید"),
            ("سایه", "سایید"),
            ("پوشه", "پوشید"),
        ] {
            assert_ne!(
                morph.lemmatize(word),
                wrong,
                "{word:?} must not lemmatize to {wrong:?}"
            );
        }
        // Genuine participles still reduce to their verb.
        assert_eq!(morph.lemmatize("کرده"), "کرد");
        assert_eq!(morph.lemmatize("شده"), "شد");
    }

    #[test]
    fn test_spell() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(nlp.spell_suggest("سسلام", 1), ["سلام"]);
        assert_eq!(nlp.spell_correct("سسلام بر ششما"), "سلام بر شما");
    }

    #[test]
    fn test_stopwords() {
        assert!(stopwords::is_stopword("و"));
        assert!(!stopwords::is_stopword("کتاب"));
        assert_eq!(
            stopwords::remove_stopwords(vec!["و".to_string(), "کتاب".to_string()]),
            ["کتاب"]
        );
    }

    #[test]
    fn a_clitic_form_is_not_a_noun_stem() {
        // `بهمین` ("to this") came back as `بهم` + `ین`, because `بهم` is a clitic
        // key the lexicon happens to list and the attached-suffix loop accepted it
        // as a stem. A clitic form is never a noun stem.
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(nlp.split_token("بهمین"), ["بهمین"]);

        // The clitic readings themselves must survive.
        assert_eq!(nlp.split_token("بهم"), ["به", "من"]);
        assert_eq!(nlp.split_token("ازم"), ["از", "من"]);
        assert_eq!(nlp.split_token("واسم"), ["برای", "من"]);

        // A real attached suffix still splits off a noun stem.
        assert_eq!(nlp.split_token("کتابم"), ["کتاب", "م"]);
        assert_eq!(nlp.split_token("کتاب\u{200C}ها"), ["کتاب", "ها"]);
        // But a word the lexicon knows stays whole, even when it looks like
        // `کتاب` + `ت`: `کتابت` is "calligraphy", and `توتم`/`بهتان`/`تومان`
        // likewise are not clitic-plus-suffix.
        for word in ["کتابت", "توتم", "بهتان", "تومان", "بهشت"] {
            assert_eq!(nlp.split_token(word), [word], "{word:?} must stay whole");
        }

        // A ZWNJ is where the morpheme boundary belongs, and these forms are
        // lexicon entries too, so the whole-word guard must not swallow them.
        let zwnj = '\u{200C}';
        for (word, expected) in [
            (format!("می{zwnj}فروش"), ["می", "فروش"]),
            (format!("می{zwnj}خوار"), ["می", "خوار"]),
            (format!("می{zwnj}گسار"), ["می", "گسار"]),
            (format!("می{zwnj}روم"), ["می", "روم"]),
            (format!("نمی{zwnj}دانم"), ["نمی", "دانم"]),
        ] {
            assert_eq!(nlp.split_token(&word), expected, "{word:?}");
        }
    }

    #[test]
    fn test_token_spans() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.tokenize_spans("سلام دنیا"),
            vec![("سلام".to_string(), 0, 8), ("دنیا".to_string(), 9, 17)]
        );
        assert_eq!(
            nlp.tokenize_char_spans("سلام دنیا"),
            vec![("سلام".to_string(), 0, 4), ("دنیا".to_string(), 5, 9)]
        );

        // Walking the tokens must give the same character offsets as counting the
        // prefix of every token independently.
        let text = "سلام😊 دنیا\u{00A0}۱۲۳ (خوب)";
        let expected: Vec<_> = Tokenizer::tokenize(text)
            .into_iter()
            .map(|token| {
                (
                    token.text.to_owned(),
                    text[..token.start].chars().count(),
                    text[..token.end].chars().count(),
                )
            })
            .collect();
        assert_eq!(nlp.tokenize_char_spans(text), expected);
    }

    #[test]
    fn test_lemmatize_verb() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.lemmatize_verb("می‌روم"),
            ("رفت".to_string(), Some("رو".to_string()))
        );
        assert_eq!(nlp.lemmatize_verb("کتاب"), ("کتاب".to_string(), None));
    }

    #[test]
    fn test_ngrams() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.ngrams("این یک متن", 1, 2).unwrap(),
            vec!["این", "یک", "متن", "این یک", "یک متن"]
        );
        assert!(nlp.ngrams("x", 0, 1).is_err());
        assert!(nlp.ngrams("x", 2, 1).is_err());
    }

    #[test]
    fn test_mask() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.mask(
                "به https://a.b و info@a.co ایمیل بزن",
                &["url".to_string(), "email".to_string()],
                None,
            )
            .unwrap(),
            "به <URL> و <EMAIL> ایمیل بزن"
        );
        assert_eq!(
            nlp.mask("این کتاب است", &["stopwords".to_string()], Some("[MASK]"),)
                .unwrap(),
            "[MASK] کتاب [MASK]"
        );
        assert!(nlp.mask("x", &["bogus".to_string()], None).is_err());
        assert_eq!(nlp.mask("x", &[], None).unwrap(), "x");
    }

    #[test]
    fn test_normalize_with() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.normalize_with("1𝟮3٤٥⓺", &["digits".to_string()])
                .unwrap(),
            "۱۲۳۴۵۶"
        );
        assert_eq!(
            nlp.normalize_with(
                "سلام(دنیا) ! می روم",
                &[
                    "punctuation".to_string(),
                    "punctuation_spacing".to_string(),
                    "verbal_spacing".to_string(),
                ],
            )
            .unwrap(),
            "سلام (دنیا)! می‌روم"
        );
        assert!(nlp.normalize_with("x", &["bogus".to_string()]).is_err());
    }

    #[test]
    fn test_process_stages() {
        let nlp = Pipeline::create(true, true, true);
        let doc = nlp.process_with(
            "سلام دنیا.",
            &ProcessOptions {
                normalize: true,
                formalize: false,
                lemmatize: false,
                sentences: true,
                ezafe: false,
            },
        );
        assert_eq!(doc.tokens, ["سلام", "دنیا", "."]);
        assert_eq!(doc.lemmas, doc.tokens);
        assert_eq!(doc.sentences.len(), 1);
        assert!(doc.ezafe_markers.is_empty());
    }
}
