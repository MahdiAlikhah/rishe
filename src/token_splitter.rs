use crate::colloquial::FUSED_PREP_CLITICS;
use crate::morphology::{MorphologyEngine, ATTACHED_NOUN_SUFFIXES, ZWNJ_NOUN_SUFFIXES};

const ZWNJ: &str = "\u{200C}";
const PREFIXES: &[&str] = &[
    "نمی", "می", "بر", "باز", "در", "فرو", "وا", "ور", "فرا", "پس", "پیش",
];

pub struct TokenSplitter;

impl TokenSplitter {
    pub fn split(token: &str, morph: &MorphologyEngine) -> Vec<String> {
        for &(colloq, (first, second)) in FUSED_PREP_CLITICS {
            if token == colloq {
                return vec![first.to_string(), second.to_string()];
            }
        }

        for &prefix in PREFIXES {
            if let Some(rest) = token.strip_prefix(prefix) {
                let rest = rest.strip_prefix(ZWNJ).unwrap_or(rest);
                if rest.is_empty() {
                    continue;
                }
                let valid = if prefix == "می" || prefix == "نمی" {
                    morph.verb_roots(rest).is_some()
                } else {
                    morph.is_known_noun(rest) || morph.verb_roots(rest).is_some()
                };
                if valid {
                    return vec![prefix.to_string(), rest.to_string()];
                }
            }
        }

        if let Some((first, second)) = token.split_once(ZWNJ) {
            if !first.is_empty()
                && !second.is_empty()
                && (morph.is_known_noun(first) || morph.is_known_noun(second))
            {
                return vec![first.to_string(), second.to_string()];
            }
        }

        for &suffix in ZWNJ_NOUN_SUFFIXES {
            if let Some(stem) = token.strip_suffix(suffix) {
                if !stem.is_empty() && morph.is_known_noun(stem) {
                    return vec![
                        stem.to_string(),
                        suffix.trim_start_matches(ZWNJ).to_string(),
                    ];
                }
            }
        }

        if morph.is_known_noun(token) {
            return vec![token.to_string()];
        }

        for &suffix in ATTACHED_NOUN_SUFFIXES {
            if let Some(stem) = token.strip_suffix(suffix) {
                if stem.chars().count() >= 2 && morph.is_known_noun(stem) {
                    return vec![stem.to_string(), suffix.to_string()];
                }
            }
        }

        vec![token.to_string()]
    }
}
