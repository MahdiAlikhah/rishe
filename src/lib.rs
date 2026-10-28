pub mod augment;
pub mod colloquial;
pub mod conjugation;
pub mod detokenizer;
pub mod embeddings;
pub mod ezafe;
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

#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
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
            other => return Err(format!("unknown pack task: {other}")),
        }
        .ok_or_else(|| format!("{task} pack is not loaded"))
    }

    #[cfg(feature = "models")]
    pub fn pos(&self, text: &str) -> Result<Vec<models::TaggedToken>, String> {
        self.pack("pos")?.tag_text_items(text)
    }

    #[cfg(feature = "models")]
    pub fn ner(&self, text: &str) -> Result<Vec<models::Entity>, String> {
        Ok(models::merge_entities(
            &self.pack("ner")?.tag_text_items(text)?,
        ))
    }

    #[cfg(feature = "models")]
    pub fn ezafe(&self, text: &str) -> Result<Vec<models::TaggedToken>, String> {
        self.pack("ezafe")?.tag_text_items(text)
    }

    #[cfg(feature = "models")]
    pub fn pos_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::TaggedToken>>, String> {
        self.pack("pos")?.tag_text_batch(texts)
    }

    #[cfg(feature = "models")]
    pub fn ner_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::Entity>>, String> {
        self.pack("ner")?.entities_batch(texts)
    }

    #[cfg(feature = "models")]
    pub fn ezafe_batch(&self, texts: &[String]) -> Result<Vec<Vec<models::TaggedToken>>, String> {
        self.pack("ezafe")?.tag_text_batch(texts)
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
                match self
                    .colloquial
                    .expand_and_formalize(tok.text, &self.morphology)
                {
                    ExpandedTokens::Single(formal_tok) => {
                        let fixed_tok = self.morphology.fix_spaceless_verb(&formal_tok);
                        let lemma = if opts.lemmatize {
                            self.morphology.lemmatize(&fixed_tok).into_owned()
                        } else {
                            fixed_tok.to_string()
                        };
                        tokens.push(fixed_tok.into_owned());
                        lemmas.push(lemma);
                    }
                    ExpandedTokens::Pair(first, second) => {
                        let lemma1 = if opts.lemmatize {
                            self.morphology.lemmatize(&first).into_owned()
                        } else {
                            first.to_string()
                        };
                        let lemma2 = if opts.lemmatize {
                            self.morphology.lemmatize(&second).into_owned()
                        } else {
                            second.to_string()
                        };
                        tokens.push(first.into_owned());
                        lemmas.push(lemma1);
                        tokens.push(second.into_owned());
                        lemmas.push(lemma2);
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

    pub fn normalize_stages(&self, text: &str, stages: &[String]) -> Result<String, String> {
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

    pub fn mask_stages(
        &self,
        text: &str,
        stages: &[String],
        mask_token: Option<&str>,
    ) -> Result<String, String> {
        let mut ops = Vec::with_capacity(stages.len());
        for stage in stages {
            match maskers::MaskOp::from_name(stage) {
                Some(op) => ops.push(op),
                None => return Err(format!("unknown mask stage: {stage}")),
            }
        }
        let mut current = Cow::Borrowed(text);
        for op in ops {
            current = Cow::Owned(op.apply(&current, mask_token));
        }
        Ok(current.into_owned())
    }

    pub fn stem(&self, word: &str) -> String {
        Stemmer::stem(word, &self.morphology).into_owned()
    }

    pub fn conjugate(&self, verb: &str) -> Option<Conjugation> {
        self.morphology.conjugate(verb)
    }

    pub fn lemma_full(&self, word: &str) -> (String, Option<String>) {
        let lemma = self.morphology.lemmatize(word).into_owned();
        let present = self.morphology.present_stem(&lemma).map(str::to_owned);
        (lemma, present)
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

    pub fn word_tokenize(&self, text: &str) -> Vec<String> {
        let normalized = self.normalizer.normalize(text);
        Tokenizer::tokenize(&normalized)
            .into_iter()
            .map(|token| token.text.to_owned())
            .collect()
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

    pub fn word_tokenize_with(
        &self,
        text: &str,
        options: &word_tokenizer::WordTokenizerOptions,
    ) -> Vec<String> {
        let pre = word_tokenizer::apply_pre(text, options);
        let normalized = self.normalizer.normalize(&pre);
        let normalized = if options.replace_numbers {
            word_tokenizer::replace_numbers(normalized.as_ref())
        } else {
            normalized.into_owned()
        };
        let tokens: Vec<String> = Tokenizer::tokenize(&normalized)
            .into_iter()
            .map(|token| token.text.to_owned())
            .collect();
        word_tokenizer::postprocess(tokens, options, &self.morphology)
    }

    pub fn ngrams(&self, text: &str, min_n: usize, max_n: usize) -> Result<Vec<String>, String> {
        if min_n == 0 || max_n < min_n {
            return Err("ngram range must be (min, max) with 1 <= min <= max".to_string());
        }
        let tokens = self.word_tokenize(text);
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

    #[pymethods]
    impl ProcessedDoc {
        #[getter]
        fn normalized_text(&self) -> &str {
            &self.normalized_text
        }
        #[getter]
        fn formal_text(&self) -> &str {
            &self.formal_text
        }
        #[getter]
        fn sentences(&self) -> Vec<String> {
            self.sentences.clone()
        }
        #[getter]
        fn tokens(&self) -> Vec<String> {
            self.tokens.clone()
        }
        #[getter]
        fn lemmas(&self) -> Vec<String> {
            self.lemmas.clone()
        }
        #[getter]
        fn ezafe_markers(&self) -> Vec<bool> {
            self.ezafe_markers.clone()
        }
    }

    #[pymethods]
    impl Pipeline {
        #[new]
        #[pyo3(signature = (unify_digits=true, strip_diacritics=true, fix_zwnj=true, pos_pack=None, ner_pack=None, ezafe_pack=None))]
        pub fn py_new(
            unify_digits: bool,
            strip_diacritics: bool,
            fix_zwnj: bool,
            pos_pack: Option<String>,
            ner_pack: Option<String>,
            ezafe_pack: Option<String>,
        ) -> PyResult<Self> {
            #[cfg(feature = "models")]
            let mut pipe = Self::create(unify_digits, strip_diacritics, fix_zwnj);
            #[cfg(not(feature = "models"))]
            let pipe = Self::create(unify_digits, strip_diacritics, fix_zwnj);

            #[cfg(feature = "models")]
            {
                for (task, path) in [("pos", pos_pack), ("ner", ner_pack), ("ezafe", ezafe_pack)] {
                    if let Some(path) = path {
                        pipe.load_pack(task, &path)
                            .map_err(pyo3::exceptions::PyValueError::new_err)?;
                    }
                }
            }
            #[cfg(not(feature = "models"))]
            {
                if pos_pack.is_some() || ner_pack.is_some() || ezafe_pack.is_some() {
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

        pub fn __call__(&self, py: Python<'_>, text: &str) -> ProcessedDoc {
            py.allow_threads(|| self.process(text))
        }

        pub fn pipe(&self, py: Python<'_>, texts: Vec<String>) -> Vec<ProcessedDoc> {
            py.allow_threads(|| self.process_batch(&texts))
        }

        pub fn process_stages(
            &self,
            py: Python<'_>,
            text: &str,
            stages: Vec<String>,
        ) -> ProcessedDoc {
            let opts = ProcessOptions::from_stages(&stages);
            py.allow_threads(|| self.process_with(text, &opts))
        }

        #[pyo3(name = "normalize_stages")]
        pub fn py_normalize_stages(&self, text: &str, stages: Vec<String>) -> PyResult<String> {
            self.normalize_stages(text, &stages)
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "mask_stages", signature = (text, stages, mask_token=None))]
        pub fn py_mask_stages(
            &self,
            text: &str,
            stages: Vec<String>,
            mask_token: Option<String>,
        ) -> PyResult<String> {
            self.mask_stages(text, &stages, mask_token.as_deref())
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        pub fn sent_tokenize(&self, text: &str) -> Vec<String> {
            SentenceSplitter::split(text)
                .into_iter()
                .map(str::to_owned)
                .collect()
        }
        pub fn normalize(&self, text: &str) -> String {
            self.normalizer.normalize(text).into_owned()
        }

        pub fn tokenize(&self, text: &str) -> Vec<String> {
            Tokenizer::tokenize(text)
                .into_iter()
                .map(|t| t.text.to_owned())
                .collect()
        }

        #[pyo3(name = "tokenize_spans")]
        pub fn py_tokenize_spans(&self, text: &str) -> Vec<(String, usize, usize)> {
            self.tokenize_char_spans(text)
        }

        #[pyo3(name = "word_tokenize")]
        pub fn py_word_tokenize(&self, text: &str) -> Vec<String> {
            self.word_tokenize(text)
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
            self.word_tokenize_with(text, &options)
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

        #[pyo3(name = "rake", signature = (text, top_n=5, max_length=3))]
        pub fn py_rake(&self, text: &str, top_n: usize, max_length: usize) -> Vec<String> {
            self.rake(text, top_n, max_length)
        }

        #[pyo3(name = "textrank", signature = (text, top_n=5, max_length=3, window=2))]
        pub fn py_textrank(
            &self,
            text: &str,
            top_n: usize,
            max_length: usize,
            window: usize,
        ) -> Vec<String> {
            self.textrank(text, top_n, max_length, window)
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

        #[pyo3(name = "lemma_full")]
        pub fn py_lemma_full(&self, word: &str) -> (String, Option<String>) {
            self.lemma_full(word)
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

        #[pyo3(name = "spell_suggest", signature = (word, max=5))]
        pub fn py_spell_suggest(&self, word: &str, max: usize) -> Vec<String> {
            self.spell_suggest(word, max)
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
        pub fn py_new(path: &str) -> PyResult<Self> {
            Self::load(path).map_err(pyo3::exceptions::PyIOError::new_err)
        }

        fn __len__(&self) -> usize {
            self.len()
        }

        #[getter(dim)]
        fn py_dim(&self) -> usize {
            self.dim()
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

    #[cfg(feature = "models")]
    #[pymethods]
    impl models::ModelPack {
        #[new]
        pub fn py_new(path: &str) -> PyResult<Self> {
            Self::load(path).map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag")]
        pub fn py_tag(&self, py: Python<'_>, text: &str) -> PyResult<Vec<(String, String)>> {
            py.allow_threads(|| self.tag_text(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag_words")]
        pub fn py_tag_words(
            &self,
            py: Python<'_>,
            words: Vec<String>,
        ) -> PyResult<Vec<(String, String)>> {
            let words: Vec<&str> = words.iter().map(String::as_str).collect();
            py.allow_threads(|| self.tag_words(&words))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag_tokens")]
        pub fn py_tag_tokens(
            &self,
            py: Python<'_>,
            text: &str,
        ) -> PyResult<Vec<models::TaggedToken>> {
            py.allow_threads(|| self.tag_text_items(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "entities")]
        pub fn py_entities(&self, py: Python<'_>, text: &str) -> PyResult<Vec<models::Entity>> {
            py.allow_threads(|| self.entities(text))
                .map_err(pyo3::exceptions::PyValueError::new_err)
        }

        #[pyo3(name = "tag_batch")]
        pub fn py_tag_batch(
            &self,
            py: Python<'_>,
            texts: Vec<String>,
        ) -> PyResult<Vec<Vec<models::TaggedToken>>> {
            py.allow_threads(|| self.tag_text_batch(&texts))
                .map_err(pyo3::exceptions::PyValueError::new_err)
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

    #[cfg(feature = "models")]
    #[pymethods]
    impl models::TaggedToken {
        #[getter(text)]
        pub fn py_text(&self) -> &str {
            &self.text
        }
        #[getter(tag)]
        pub fn py_tag(&self) -> &str {
            &self.tag
        }
        #[getter(start)]
        pub fn py_start(&self) -> usize {
            self.start
        }
        #[getter(end)]
        pub fn py_end(&self) -> usize {
            self.end
        }
    }

    #[cfg(feature = "models")]
    #[pymethods]
    impl models::Entity {
        #[getter(text)]
        pub fn py_text(&self) -> &str {
            &self.text
        }
        #[getter(label)]
        pub fn py_label(&self) -> &str {
            &self.label
        }
        #[getter(start)]
        pub fn py_start(&self) -> usize {
            self.start
        }
        #[getter(end)]
        pub fn py_end(&self) -> usize {
            self.end
        }
    }

    #[pymodule]
    pub fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_class::<Pipeline>()?;
        m.add_class::<ProcessedDoc>()?;
        m.add_class::<embeddings::Embeddings>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::ModelPack>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::TaggedToken>()?;
        #[cfg(feature = "models")]
        m.add_class::<models::Entity>()?;
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
    fn test_lemma_full() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.lemma_full("می‌روم"),
            ("رفت".to_string(), Some("رو".to_string()))
        );
        assert_eq!(nlp.lemma_full("کتاب"), ("کتاب".to_string(), None));
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
    fn test_mask_stages() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.mask_stages(
                "به https://a.b و info@a.co ایمیل بزن",
                &["url".to_string(), "email".to_string()],
                None,
            )
            .unwrap(),
            "به <URL> و <EMAIL> ایمیل بزن"
        );
        assert_eq!(
            nlp.mask_stages("این کتاب است", &["stopwords".to_string()], Some("[MASK]"),)
                .unwrap(),
            "[MASK] کتاب [MASK]"
        );
        assert!(nlp.mask_stages("x", &["bogus".to_string()], None).is_err());
        assert_eq!(nlp.mask_stages("x", &[], None).unwrap(), "x");
    }

    #[test]
    fn test_normalize_stages() {
        let nlp = Pipeline::create(true, true, true);
        assert_eq!(
            nlp.normalize_stages("1𝟮3٤٥⓺", &["digits".to_string()])
                .unwrap(),
            "۱۲۳۴۵۶"
        );
        assert_eq!(
            nlp.normalize_stages(
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
        assert!(nlp.normalize_stages("x", &["bogus".to_string()]).is_err());
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
