pub mod colloquial;
pub mod conjugation;
pub mod detokenizer;
pub mod ezafe;
pub mod inflector;
pub mod morphology;
pub mod normalizer;
pub mod sbd;
pub mod spell;
pub mod stemmer;
pub mod stopwords;
pub mod token_splitter;
pub mod tokenizer;

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
        }
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
}

#[cfg(feature = "python")]
mod py_bindings {
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
        #[pyo3(signature = (unify_digits=true, strip_diacritics=true, fix_zwnj=true))]
        pub fn py_new(unify_digits: bool, strip_diacritics: bool, fix_zwnj: bool) -> Self {
            Self::create(unify_digits, strip_diacritics, fix_zwnj)
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

        pub fn word_tokenize(&self, text: &str) -> Vec<String> {
            let norm_cow = self.normalizer.normalize(text);
            Tokenizer::tokenize(&norm_cow)
                .into_iter()
                .map(|t| t.text.to_owned())
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

    #[pymodule]
    pub fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_class::<Pipeline>()?;
        m.add_class::<ProcessedDoc>()?;
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
