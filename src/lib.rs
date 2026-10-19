pub mod colloquial;
pub mod detokenizer;
pub mod ezafe;
pub mod morphology;
pub mod normalizer;
pub mod sbd;
pub mod tokenizer;

use colloquial::{ColloquialConverter, ExpandedTokens};
use detokenizer::Detokenizer;
use ezafe::RuleBasedEzafe;
use morphology::MorphologyEngine;
use normalizer::{Normalizer, NormalizerConfig};
use rayon::prelude::*;
use sbd::SentenceSplitter;
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

#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
pub struct Pipeline {
    normalizer: Normalizer,
    morphology: MorphologyEngine,
    colloquial: ColloquialConverter,
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
        }
    }

    #[inline]
    pub fn process(&self, text: &str) -> ProcessedDoc {
        let norm_cow = self.normalizer.normalize(text);
        let raw_tokens = Tokenizer::tokenize(&norm_cow);

        let mut tokens = Vec::with_capacity(raw_tokens.len() + 4);
        let mut lemmas = Vec::with_capacity(raw_tokens.len() + 4);

        for tok in raw_tokens {
            if tok.kind == TokenKind::Word {
                match self
                    .colloquial
                    .expand_and_formalize(tok.text, &self.morphology)
                {
                    ExpandedTokens::Single(formal_tok) => {
                        let fixed_tok = self.morphology.fix_spaceless_verb(&formal_tok);
                        let lemma = self.morphology.lemmatize(&fixed_tok).into_owned();
                        tokens.push(fixed_tok.into_owned());
                        lemmas.push(lemma);
                    }
                    ExpandedTokens::Pair(first, second) => {
                        let lemma1 = self.morphology.lemmatize(&first).into_owned();
                        let lemma2 = self.morphology.lemmatize(&second).into_owned();
                        tokens.push(first.into_owned());
                        lemmas.push(lemma1);
                        tokens.push(second.into_owned());
                        lemmas.push(lemma2);
                    }
                }
            } else {
                tokens.push(tok.text.to_string());
                lemmas.push(tok.text.to_string());
            }
        }

        let formal_text = Detokenizer::detokenize(&tokens);
        let sentences = SentenceSplitter::split(&formal_text)
            .into_iter()
            .map(str::to_owned)
            .collect();

        let ezafe_markers = RuleBasedEzafe::predict(&tokens, &self.morphology);

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

        pub fn sent_tokenize(&self, text: &str) -> Vec<String> {
            SentenceSplitter::split(text)
                .into_iter()
                .map(str::to_owned)
                .collect()
        }
        pub fn normalize(&self, text: &str) -> String {
            self.normalizer.normalize(text).into_owned()
        }

        pub fn word_tokenize(&self, text: &str) -> Vec<String> {
            let norm_cow = self.normalizer.normalize(text);
            Tokenizer::tokenize(&norm_cow)
                .into_iter()
                .map(|t| t.text.to_owned())
                .collect()
        }

        pub fn lemmatize(&self, word: &str) -> String {
            let norm = self.normalizer.normalize(word);
            self.morphology.lemmatize(&norm).into_owned()
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
}
