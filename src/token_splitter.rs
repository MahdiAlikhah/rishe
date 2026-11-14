use crate::colloquial::CLITIC_LOOKALIKES;
use crate::colloquial::FUSED_PREP_CLITICS;
use crate::morphology::{MorphologyEngine, ATTACHED_NOUN_SUFFIXES, ZWNJ_NOUN_SUFFIXES};

const ZWNJ: &str = "\u{200C}";
const PREFIXES: &[&str] = &[
    "نمی", "می", "بر", "باز", "در", "فرو", "وا", "ور", "فرا", "پس", "پیش",
];

/// Splits fused Persian tokens into their morphemes.
pub struct TokenSplitter;

impl TokenSplitter {
    /// Split on a known prefix, ZWNJ or noun suffix when the remainder is a known
    /// word; otherwise return the token unchanged.
    pub fn split(token: &str, morph: &MorphologyEngine) -> Vec<String> {
        if token.is_empty() {
            return Vec::new();
        }

        // The fused-clitic table goes first: `بهش` is a lexicon entry, but the
        // documented split is `به` + `او`, and that reading is what the table is for.
        for &(colloq, (first, second)) in FUSED_PREP_CLITICS {
            if token == colloq && !CLITIC_LOOKALIKES.contains(&token) {
                return vec![first.to_string(), second.to_string()];
            }
        }

        // A word the lexicon already knows is a word, not a prefix plus a word.
        // Without this, `برنامه` splits into `بر` + `نامه` and `درخت` into
        // `در` + `خت`, because the remainder happens to be in the lexicon.
        //
        // A ZWNJ is different: `می‌فروش` and `پیش‌بینی` are lexicon entries too,
        // yet the ZWNJ is exactly where the morpheme boundary belongs, so those
        // still go through the prefix loop below.
        if !token.contains(ZWNJ) && morph.is_known_noun(token) {
            return vec![token.to_string()];
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
                if stem.chars().count() >= 2
                    && morph.is_known_noun(stem)
                    // `بهمین` is `بهم` + `ین` only because `بهم` is a clitic key
                    // that the lexicon happens to list; a clitic form is not a noun
                    // stem, so stripping a suffix off one produces nonsense.
                    && !is_clitic_key(stem)
                {
                    return vec![stem.to_string(), suffix.to_string()];
                }
            }
        }

        vec![token.to_string()]
    }
}

/// Whether `stem` is a fused preposition+pronoun form rather than a noun stem.
fn is_clitic_key(stem: &str) -> bool {
    FUSED_PREP_CLITICS.iter().any(|&(colloq, _)| colloq == stem)
}
