pub mod augment;
pub mod colloquial;
pub mod conjugation;
pub mod dates;
pub mod detokenizer;
pub mod embeddings;
pub mod ezafe;
pub mod hub;
pub mod inflector;
pub mod keywords;
pub mod maskers;
#[cfg(feature = "models")]
pub mod models;
pub mod morphology;
pub mod normalize_ops;
pub mod normalizer;
pub mod sbd;
pub mod spell;
pub mod stemmer;
pub mod stopwords;
pub mod token_splitter;
pub mod tokenizer;
pub mod transliterate;
pub mod utilities;
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

#[cfg_attr(feature = "python", pyo3::pyclass(frozen, get_all))]
#[derive(Debug, Clone)]
pub struct ProcessedDoc {
    pub normalized_text: String,
    pub formal_text: String,
    pub sentences: Vec<String>,
    pub tokens: Vec<String>,
    pub lemmas: Vec<String>,
    pub ezafe_markers: Vec<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessOptions {
    pub normalize: bool,
    pub formalize: bool,
    pub lemmatize: bool,
    pub sentences: bool,
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
}

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

    #[cfg(feature = "models")]
    pub fn pos(&self, text: &str) -> Result<Vec<models::TaggedToken>, String> {
        self.pack("pos")?.tags(text)
    }

    #[cfg(feature = "models")]
    pub fn ner(&self, text: &str) -> Result<Vec<models::Entity>, String> {
        self.pack("ner")?.entities(text)
    }

    #[cfg(feature = "models")]
    pub fn ezafe(&self, text: &str) -> Result<Vec<models::TaggedToken>, String> {
        self.pack("ezafe")?.tags(text)
    }

    #[cfg(feature = "models")]
    pub fn pos_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::TaggedToken>>, String> {
        self.pack("pos")?.tags_batch(texts)
    }

    #[cfg(feature = "models")]
    pub fn ner_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::Entity>>, String> {
        self.pack("ner")?.entities_batch(texts)
    }

    #[cfg(feature = "models")]
    pub fn ezafe_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::TaggedToken>>, String> {
        self.pack("ezafe")?.tags_batch(texts)
    }

    #[cfg(feature = "models")]
    pub fn depparse(&self, text: &str) -> Result<Vec<models::Dependency>, String> {
        self.pack("depparse")?.depparse(text)
    }

    #[cfg(feature = "models")]
    pub fn depparse_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::Dependency>>, String> {
        texts.par_iter().map(|text| self.depparse(text)).collect()
    }

    #[inline]
    pub fn process(&self, text: &str) -> ProcessedDoc {
        self.process_with(text, &ProcessOptions::default())
    }

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

    pub fn process_batch(&self, texts: &[String]) -> Vec<ProcessedDoc> {
        texts.par_iter().map(|t| self.process(t)).collect()
    }

    /// Normalize with an explicit, ordered list of operator names.
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

    /// Deprecated alias of [`Pipeline::normalize_with`].
    #[deprecated(since = "0.11.0", note = "use `Pipeline::normalize_with`")]
    pub fn normalize_stages(&self, text: &str, stages: &[String]) -> Result<String, String> {
        self.normalize_with(text, stages)
    }

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

    /// Deprecated alias of [`Pipeline::mask`].
    #[deprecated(since = "0.11.0", note = "use `Pipeline::mask`")]
    pub fn mask_stages(
        &self,
        text: &str,
        stages: &[String],
        mask_token: Option<&str>,
    ) -> Result<String, String> {
        self.mask(text, stages, mask_token)
    }

    pub fn stem(&self, word: &str) -> String {
        Stemmer::stem(word, &self.morphology).into_owned()
    }

    pub fn conjugate(&self, verb: &str) -> Option<Conjugation> {
        self.morphology.conjugate(verb)
    }

    /// Lemma plus the present stem of the lemma when it is a verb.
    pub fn lemmatize_verb(&self, word: &str) -> (String, Option<String>) {
        let lemma = self.morphology.lemmatize(word).into_owned();
        let present = self.morphology.present_stem(&lemma).map(str::to_owned);
        (lemma, present)
    }

    /// Deprecated alias of [`Pipeline::lemmatize_verb`].
    #[deprecated(since = "0.11.0", note = "use `Pipeline::lemmatize_verb`")]
    pub fn lemma_full(&self, word: &str) -> (String, Option<String>) {
        self.lemmatize_verb(word)
    }

    pub fn split_token(&self, token: &str) -> Vec<String> {
        TokenSplitter::split(token, &self.morphology)
    }

    pub fn spell_suggest(&self, word: &str, max: usize) -> Vec<String> {
        let normalized = self.normalizer.normalize(word);
        self.spell.suggest(&normalized, max)
    }

    pub fn spell_correct(&self, text: &str) -> String {
        let normalized = self.normalizer.normalize(text);
        self.spell.correct(&normalized)
    }

    pub fn comparative(&self, adj: &str) -> String {
        Inflector::comparative(adj)
    }

    pub fn superlative(&self, adj: &str) -> String {
        Inflector::superlative(adj)
    }

    pub fn plural(&self, noun: &str) -> String {
        Inflector::plural(noun)
    }

    pub fn possessive(&self, noun: &str, person: usize) -> Option<String> {
        Inflector::possessive(noun, person)
    }

    pub fn tokenize_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
        Tokenizer::tokenize(text)
            .into_iter()
            .map(|token| (token.text.to_owned(), token.start, token.end))
            .collect()
    }

    pub fn tokenize_char_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
        Tokenizer::tokenize(text)
            .into_iter()
            .map(|token| {
                let start = text[..token.start].chars().count();
                let end = text[..token.end].chars().count();
                (token.text.to_owned(), start, end)
            })
            .collect()
    }

    /// Tokenize into surface words. This does not normalize: run
    /// [`Pipeline::normalize`] first (or use [`Pipeline::process`], which runs
    /// the whole pipeline including normalization).
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

    /// Deprecated: keeps its historical normalize-then-tokenize behaviour.
    /// Use [`Pipeline::tokenize`] for surface tokens or [`Pipeline::process`]
    /// for normalized ones.
    #[deprecated(
        since = "0.11.0",
        note = "use `Pipeline::tokenize` for surface tokens or `Pipeline::process` for normalized tokens"
    )]
    pub fn word_tokenize(&self, text: &str) -> Vec<String> {
        self.tokenize(
            &self.normalizer.normalize(text),
            &word_tokenizer::WordTokenizerOptions::default(),
        )
    }

    /// Deprecated: keeps its historical normalize-then-tokenize behaviour.
    #[deprecated(
        since = "0.11.0",
        note = "use `Pipeline::tokenize` for surface tokens or `Pipeline::process` for normalized tokens"
    )]
    pub fn word_tokenize_with(
        &self,
        text: &str,
        options: &word_tokenizer::WordTokenizerOptions,
    ) -> Vec<String> {
        self.tokenize(&self.normalizer.normalize(text), options)
    }

    pub fn rake(&self, text: &str, top_n: usize, max_length: usize) -> Vec<String> {
        let normalized = self.normalizer.normalize(text);
        keywords::rake(&normalized, top_n, max_length)
    }

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

    /// Emit a Python ``DeprecationWarning`` for a renamed surface entry.
    ///
    /// Aliases added while the API is renamed stay for one cycle and disappear
    /// in 1.0.0; see `docs/API_CONVENTIONS.md` rule 12.
    fn warn_deprecated(py: Python<'_>, old: &str, new: &str) {
        let message =
            format!("{old} is deprecated and will be removed in rishe 1.0.0; use {new} instead.");
        if let Ok(warnings) = py.import_bound("warnings") {
            let category = py.get_type_bound::<pyo3::exceptions::PyDeprecationWarning>();
            let _ = warnings.call_method1("warn", (message, category));
        }
    }

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

        #[cfg(feature = "models")]
        #[pyo3(name = "pos")]
        pub fn py_pos(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.pos(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[cfg(feature = "models")]
        #[pyo3(name = "ner")]
        pub fn py_ner(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Entity>> {
            py.allow_threads(|| self.ner(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[cfg(feature = "models")]
        #[pyo3(name = "ezafe")]
        pub fn py_ezafe(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.ezafe(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

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

        #[cfg(feature = "models")]
        #[pyo3(name = "depparse")]
        pub fn py_depparse(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Dependency>> {
            py.allow_threads(|| self.depparse(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

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

        pub fn __call__(&self, py: Python<'_>, text: &str) -> ProcessedDoc {
            py.allow_threads(|| self.process(text))
        }

        pub fn pipe(&self, py: Python<'_>, texts: Vec<String>) -> Vec<ProcessedDoc> {
            warn_deprecated(py, "pipe", "process_batch");
            py.allow_threads(|| self.process_batch(&texts))
        }

        #[pyo3(name = "process_batch")]
        pub fn py_process_batch(&self, py: Python<'_>, texts: Vec<String>) -> Vec<ProcessedDoc> {
            py.allow_threads(|| self.process_batch(&texts))
        }

        #[pyo3(name = "process", signature = (text, stages=None))]
        pub fn py_process(
            &self,
            py: Python<'_>,
            text: &str,
            stages: Option<Vec<String>>,
        ) -> ProcessedDoc {
            match stages {
                Some(stages) => {
                    let opts = ProcessOptions::from_stages(&stages);
                    py.allow_threads(|| self.process_with(text, &opts))
                }
                None => py.allow_threads(|| self.process(text)),
            }
        }

        pub fn process_stages(
            &self,
            py: Python<'_>,
            text: &str,
            stages: Vec<String>,
        ) -> ProcessedDoc {
            warn_deprecated(py, "process_stages", "process");
            let opts = ProcessOptions::from_stages(&stages);
            py.allow_threads(|| self.process_with(text, &opts))
        }

        #[pyo3(name = "normalize_stages")]
        pub fn py_normalize_stages(
            &self,
            py: Python<'_>,
            text: &str,
            stages: Vec<String>,
        ) -> PyResult<String> {
            warn_deprecated(py, "normalize_stages", "normalize");
            self.normalize_with(text, &stages)
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

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

        #[pyo3(name = "mask_stages", signature = (text, stages, mask_token=None))]
        pub fn py_mask_stages(
            &self,
            py: Python<'_>,
            text: &str,
            stages: Vec<String>,
            mask_token: Option<String>,
        ) -> PyResult<String> {
            warn_deprecated(py, "mask_stages", "mask");
            self.mask(text, &stages, mask_token.as_deref())
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        pub fn sent_tokenize(&self, text: &str) -> Vec<String> {
            SentenceSplitter::split(text)
                .into_iter()
                .map(str::to_owned)
                .collect()
        }

        #[pyo3(signature = (text, stages=None))]
        pub fn normalize(&self, text: &str, stages: Option<Vec<String>>) -> PyResult<String> {
            match stages {
                Some(stages) => self
                    .normalize_with(text, &stages)
                    .map_err(pyo3::exceptions::PyValueError::new_err),
                None => Ok(self.normalizer.normalize(text).into_owned()),
            }
        }

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

        #[pyo3(name = "tokenize_spans")]
        pub fn py_tokenize_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
            self.tokenize_char_spans(text)
        }

        #[pyo3(name = "word_tokenize")]
        pub fn py_word_tokenize(&self, py: Python<'_>, text: &str) -> Vec<String> {
            warn_deprecated(
                py,
                "word_tokenize",
                "tokenize (or process for normalized tokens)",
            );
            Pipeline::tokenize(
                self,
                &self.normalizer.normalize(text),
                &word_tokenizer::WordTokenizerOptions::default(),
            )
        }

        #[pyo3(
            name = "word_tokenize_with",
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
        pub fn py_word_tokenize_with(
            &self,
            py: Python<'_>,
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
            warn_deprecated(
                py,
                "word_tokenize_with",
                "tokenize (or process for normalized tokens)",
            );
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
            Pipeline::tokenize(self, &self.normalizer.normalize(text), &options)
        }

        #[pyo3(name = "number_to_words")]
        pub fn py_number_to_words(
            &self,
            value: &pyo3::Bound<'_, pyo3::types::PyAny>,
        ) -> PyResult<String> {
            if let Ok(number) = value.extract::<i64>() {
                return Ok(utilities::number_to_words(number));
            }
            if let Ok(text) = value.extract::<String>() {
                return utilities::number_to_words_text(&text)
                    .map_err(pyo3::exceptions::PyValueError::new_err);
            }
            Err(pyo3::exceptions::PyTypeError::new_err(
                "expected int or str",
            ))
        }

        #[pyo3(name = "ngrams", signature = (text, min_n=1, max_n=1))]
        pub fn py_ngrams(&self, text: &str, min_n: usize, max_n: usize) -> PyResult<Vec<String>> {
            self.ngrams(text, min_n, max_n)
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "rake", signature = (text, n=5, max_length=3))]
        pub fn py_rake(&self, text: &str, n: usize, max_length: usize) -> Vec<String> {
            self.rake(text, n, max_length)
        }

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

        #[pyo3(name = "to_tajik")]
        pub fn py_to_tajik(&self, text: &str) -> String {
            transliterate::to_tajik(text)
        }

        #[pyo3(name = "to_persian")]
        pub fn py_to_persian(&self, text: &str) -> String {
            transliterate::to_persian(text)
        }

        #[pyo3(name = "persianize")]
        pub fn py_persianize(&self, text: &str) -> String {
            utilities::persianize(text)
        }

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

        pub fn lemmatize(&self, word: &str) -> String {
            self.morphology.lemmatize(word).into_owned()
        }

        #[pyo3(name = "lemmatize_verb")]
        pub fn py_lemmatize_verb(&self, word: &str) -> (String, Option<String>) {
            self.lemmatize_verb(word)
        }

        #[pyo3(name = "lemma_full")]
        pub fn py_lemma_full(&self, py: Python<'_>, word: &str) -> (String, Option<String>) {
            warn_deprecated(py, "lemma_full", "lemmatize_verb");
            self.lemmatize_verb(word)
        }

        #[pyo3(name = "stem")]
        pub fn py_stem(&self, word: &str) -> String {
            self.stem(word)
        }

        #[pyo3(name = "conjugate")]
        pub fn py_conjugate(
            &self,
            verb: &str,
        ) -> Option<std::collections::HashMap<String, Vec<String>>> {
            self.conjugate(verb).map(Conjugation::into_map)
        }

        #[pyo3(name = "split_token")]
        pub fn py_split_token(&self, token: &str) -> Vec<String> {
            self.split_token(token)
        }

        #[pyo3(name = "spell_suggest", signature = (word, n=5))]
        pub fn py_spell_suggest(&self, word: &str, n: usize) -> Vec<String> {
            self.spell_suggest(word, n)
        }

        #[pyo3(name = "spell_correct")]
        pub fn py_spell_correct(&self, text: &str) -> String {
            self.spell_correct(text)
        }

        #[pyo3(name = "comparative")]
        pub fn py_comparative(&self, adj: &str) -> String {
            self.comparative(adj)
        }

        #[pyo3(name = "superlative")]
        pub fn py_superlative(&self, adj: &str) -> String {
            self.superlative(adj)
        }

        #[pyo3(name = "plural")]
        pub fn py_plural(&self, noun: &str) -> String {
            self.plural(noun)
        }

        #[pyo3(name = "possessive")]
        pub fn py_possessive(&self, noun: &str, person: usize) -> Option<String> {
            self.possessive(noun, person)
        }

        pub fn is_stopword(&self, word: &str) -> bool {
            stopwords::is_stopword(word)
        }

        pub fn remove_stopwords(&self, tokens: Vec<String>) -> Vec<String> {
            stopwords::remove_stopwords(tokens)
        }

        pub fn stopwords(&self) -> Vec<String> {
            stopwords::stopwords()
        }
    }

    #[pymethods]
    impl embeddings::Embeddings {
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

        #[pyo3(name = "contains")]
        pub fn py_contains(&self, word: &str) -> bool {
            self.contains(word)
        }

        #[pyo3(name = "vector")]
        pub fn py_vector(&self, word: &str) -> Option<Vec<f32>> {
            self.vector(word).map(<[f32]>::to_vec)
        }

        #[pyo3(name = "similarity")]
        pub fn py_similarity(&self, first: &str, second: &str) -> Option<f32> {
            self.similarity(first, second)
        }

        #[pyo3(name = "sentence_vector")]
        pub fn py_sentence_vector(&self, text: &str) -> Vec<f32> {
            self.sentence_vector(text)
        }

        #[pyo3(name = "sentence_vectors")]
        pub fn py_sentence_vectors(&self, py: Python<'_>, texts: Vec<String>) -> Vec<Vec<f32>> {
            py.allow_threads(|| self.sentence_vectors(&texts))
        }

        #[pyo3(name = "sentence_similarity")]
        pub fn py_sentence_similarity(&self, first: &str, second: &str) -> f32 {
            self.sentence_similarity(first, second)
        }

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
        #[new]
        pub fn py_new(path: PathBuf) -> PyResult<Self> {
            Self::load(path).map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag")]
        pub fn py_tag(&self, py: Python<'_>, text: &str) -> PyResult<Vec<(String, String)>> {
            warn_deprecated(py, "tag", "tags");
            py.allow_threads(|| self.tags(text))
                .map(|tokens| {
                    tokens
                        .into_iter()
                        .map(|token| (token.text, token.tag))
                        .collect()
                })
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

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

        #[pyo3(name = "tag_words")]
        pub fn py_tag_words(
            &self,
            py: Python<'_>,
            words: Vec<String>,
        ) -> PyResult<Vec<(String, String)>> {
            warn_deprecated(py, "tag_words", "tags_from_words");
            self.py_tags_from_words(py, words)
        }

        #[pyo3(name = "tags")]
        pub fn py_tags(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.tags(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag_tokens")]
        pub fn py_tag_tokens(
            &self,
            py: Python<'_>,
            text: &str,
        ) -> PyResult<Vec<models::TaggedToken>> {
            warn_deprecated(py, "tag_tokens", "tags");
            self.py_tags(py, text)
        }

        #[pyo3(name = "entities")]
        pub fn py_entities(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Entity>> {
            py.allow_threads(|| self.entities(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tags_batch")]
        pub fn py_tags_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::TaggedToken>>> {
            py.allow_threads(|| self.tags_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag_batch")]
        pub fn py_tag_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::TaggedToken>>> {
            warn_deprecated(py, "tag_batch", "tags_batch");
            self.py_tags_batch(py, texts)
        }

        #[pyo3(name = "entities_batch")]
        pub fn py_entities_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::Entity>>> {
            py.allow_threads(|| self.entities_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "labels")]
        pub fn py_labels(&self) -> Vec<String> {
            self.labels().to_vec()
        }

        #[getter(task)]
        pub fn py_task(&self) -> &str {
            self.task()
        }

        #[getter(max_len)]
        pub fn py_max_len(&self) -> usize {
            self.max_len()
        }
    }

    #[pymodule]
    pub fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
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
    }

    #[test]
    fn test_token_split() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(nlp.split_token("کتاب‌ها"), ["کتاب", "ها"]);
        assert_eq!(nlp.split_token("بهش"), ["به", "او"]);
        assert_eq!(nlp.split_token("می‌روم"), ["می", "روم"]);
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
    #[allow(deprecated)]
    fn deprecated_aliases_still_work() {
        let nlp = Pipeline::create(true, true, true);
        let defaults = word_tokenizer::WordTokenizerOptions::default();
        // The legacy names keep their normalize-then-tokenize behaviour; the
        // canonical `tokenize` is the surface tokenizer.
        assert_eq!(nlp.word_tokenize("مي كنم"), nlp.process("مي كنم").tokens);
        assert_eq!(nlp.tokenize("مي كنم", &defaults).len(), 2);
        assert_eq!(
            nlp.word_tokenize_with("کتاب‌ها", &defaults),
            nlp.tokenize("کتاب‌ها", &defaults)
        );
        assert_eq!(nlp.lemma_full("می‌روم"), nlp.lemmatize_verb("می‌روم"));
        assert_eq!(nlp.mask_stages("x", &[], None).unwrap(), "x");
        assert_eq!(
            nlp.normalize_stages("۱۲۳", &[]).unwrap(),
            nlp.normalize_with("۱۲۳", &[]).unwrap()
        );
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
