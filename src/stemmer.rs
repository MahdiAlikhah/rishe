use crate::morphology::{MorphologyEngine, ATTACHED_NOUN_SUFFIXES, ZWNJ_NOUN_SUFFIXES};
use std::borrow::Cow;

/// Noun-suffix stripper that falls back to lemmatization.
pub struct Stemmer;

impl Stemmer {
    /// Strip a known noun suffix when the remainder is a known noun, otherwise lemmatize.
    pub fn stem<'a>(word: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
        for &suffix in ZWNJ_NOUN_SUFFIXES {
            if let Some(stem) = word.strip_suffix(suffix) {
                if !stem.is_empty() && morph.is_known_noun(stem) {
                    return Cow::Borrowed(stem);
                }
            }
        }

        if morph.is_known_noun(word) {
            return Cow::Borrowed(word);
        }

        for &suffix in ATTACHED_NOUN_SUFFIXES {
            if let Some(stem) = word.strip_suffix(suffix) {
                if stem.chars().count() >= 2 && morph.is_known_noun(stem) {
                    return Cow::Borrowed(stem);
                }
            }
        }

        morph.lemmatize(word)
    }
}
