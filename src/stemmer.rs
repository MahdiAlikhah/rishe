use crate::colloquial::is_clitic_key;
use crate::morphology::{MorphologyEngine, ATTACHED_NOUN_SUFFIXES, ZWNJ_NOUN_SUFFIXES};
use std::borrow::Cow;

/// Noun-suffix stripper that falls back to lemmatization.
pub struct Stemmer;

impl Stemmer {
    /// Strip a known noun suffix when the remainder is a known noun, otherwise lemmatize.
    pub fn stem<'a>(word: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
        // A word that lemmatizes to a known verb is a verb form, not a noun plus a
        // suffix: `کردیم` is `کردن` in the 1pl, not the noun `کردی` plus `م`.
        let lemma = morph.lemmatize(word);
        if morph.present_stem(&lemma).is_some() {
            return lemma;
        }

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
                // A clitic key is not a noun stem: `بهمین` is not `بهم` + `ین`.
                // The splitter refuses the same word, so both APIs agree.
                if stem.chars().count() >= 2 && !is_clitic_key(stem) {
                    // `مردگان` is the plural of `مرده`; the ᴳᵃⁿ ending restores
                    // the `ه` the same way `lemmatize` does.
                    if suffix == "گان" {
                        let restored = format!("{stem}ه");
                        if morph.is_known_noun(&restored) {
                            return Cow::Owned(restored);
                        }
                    }
                    if morph.is_known_noun(stem) {
                        return Cow::Borrowed(stem);
                    }
                }
            }
        }

        lemma
    }
}
