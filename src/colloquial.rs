use crate::morphology::MorphologyEngine;
use fst::Map;
use std::borrow::Cow;

static INFORMAL_WORDS_FST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/informal_words.fst"));
static INFORMAL_VERBS_FST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/informal_verbs.fst"));

/// Result of formalizing one colloquial token.
pub enum ExpandedTokens<'a> {
    /// One formal token.
    Single(Cow<'a, str>),
    /// Two formal tokens, e.g. a preposition plus its pronoun clitic.
    Pair(Cow<'a, str>, Cow<'a, str>),
}

pub(crate) const FUSED_PREP_CLITICS: &[(&str, (&str, &str))] = &[
    ("بهم", ("به", "من")),
    ("بهت", ("به", "تو")),
    ("بهش", ("به", "او")),
    ("بهمون", ("به", "ما")),
    ("بهتون", ("به", "شما")),
    ("بهشون", ("به", "آن\u{200C}ها")),
    ("ازم", ("از", "من")),
    ("ازت", ("از", "تو")),
    ("ازش", ("از", "او")),
    ("ازمون", ("از", "ما")),
    ("ازتون", ("از", "شما")),
    ("ازشون", ("از", "آن\u{200C}ها")),
    ("توم", ("در", "من")),
    ("توت", ("در", "تو")),
    ("توش", ("در", "آن")),
    ("توشون", ("در", "آن\u{200C}ها")),
    ("باهام", ("با", "من")),
    ("باهات", ("با", "تو")),
    ("باهاش", ("با", "او")),
    ("باهامون", ("با", "ما")),
    ("باهاتون", ("با", "شما")),
    ("باهاشون", ("با", "آن\u{200C}ها")),
    ("واسم", ("برای", "من")),
    ("واست", ("برای", "تو")),
    ("واسش", ("برای", "او")),
    ("واسمون", ("برای", "ما")),
    ("واستون", ("برای", "شما")),
    ("واسشون", ("برای", "آن\u{200C}ها")),
];

/// Fused forms that are also ordinary words, so the clitic reading is wrong.
///
/// `توت` is mulberry; every other key in [`FUSED_PREP_CLITICS`] is a form that
/// only exists as a clitic, so the table stays useful. Tested by frequency, not
/// by lexicon membership: all 28 keys are frequency-1 lexicon entries, so a
/// blanket "is a known noun" guard would switch the whole feature off.
pub(crate) const CLITIC_LOOKALIKES: &[&str] = &["توت"];

/// Copula lookalikes that the whole-word guard does not already cover.
///
/// The guard rejects any token the lexicon knows, which covers 7 of the 8 former
/// entries here; `کاست` is the one the lexicon does not list.
const NON_COPULA_STEMS: &[&str] = &["کاست"];

const COLLOQUIAL_NOUN_SUFFIXES: &[(&str, &str)] = &[
    ("\u{200C}هامون", "\u{200C}هایمان"),
    ("\u{200C}هاتون", "\u{200C}هایتان"),
    ("\u{200C}هاشون", "\u{200C}هایشان"),
    ("هامون", "\u{200C}هایمان"),
    ("هاتون", "\u{200C}هایتان"),
    ("هاشون", "\u{200C}هایشان"),
    ("مون", "مان"),
    ("تون", "تان"),
    ("شون", "شان"),
];

const COLLOQUIAL_VERB_ENDINGS: &[(&str, &str)] = &[
    ("یم", "یم"),
    ("ین", "ید"),
    ("ید", "ید"),
    ("م", "م"),
    ("ی", "ی"),
    ("ه", "د"),
    ("ن", "ند"),
    ("ند", "ند"),
];

/// Colloquial-to-formal converter backed by bundled FSTs.
pub struct ColloquialConverter {
    informal_words: Map<&'static [u8]>,
    informal_verb_stems: Map<&'static [u8]>,
}

impl ColloquialConverter {
    /// Build the converter from the FSTs bundled at compile time.
    pub fn new() -> Self {
        Self {
            informal_words: Map::new(INFORMAL_WORDS_FST).expect("Corrupted informal_words.fst"),
            informal_verb_stems: Map::new(INFORMAL_VERBS_FST)
                .expect("Corrupted informal_verbs.fst"),
        }
    }

    /// Formalize one token: fused preposition pronouns and copulas expand to two
    /// tokens, the rest to one.
    #[inline]
    pub fn expand_and_formalize<'a>(
        &self,
        token: &'a str,
        morph: &MorphologyEngine,
    ) -> ExpandedTokens<'a> {
        for &(colloq, (prep, pron)) in FUSED_PREP_CLITICS {
            if token == colloq && !CLITIC_LOOKALIKES.contains(&token) {
                return ExpandedTokens::Pair(Cow::Borrowed(prep), Cow::Borrowed(pron));
            }
        }
        if let Some((stem, copula)) = self.try_split_copula(token, morph) {
            return ExpandedTokens::Pair(stem, Cow::Borrowed(copula));
        }
        ExpandedTokens::Single(self.to_formal(token, morph))
    }

    /// Map a colloquial word or verb form to its formal form, returning the input
    /// when nothing matches.
    #[inline]
    pub fn to_formal<'a>(&self, token: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
        if let Some(packed) = self.informal_words.get(token) {
            return MorphologyEngine::unpack_lemma(packed, token);
        }
        if morph.is_known_noun(token) {
            return Cow::Borrowed(token);
        }
        if let Some(v) = self.try_convert_colloquial_verb(token, morph) {
            return Cow::Owned(v);
        }
        if let Some(n) = self.try_convert_colloquial_noun(token, morph) {
            return n;
        }
        Cow::Borrowed(token)
    }

    fn try_split_copula<'a>(
        &self,
        token: &'a str,
        morph: &MorphologyEngine,
    ) -> Option<(Cow<'a, str>, &'static str)> {
        if NON_COPULA_STEMS.contains(&token) {
            return None;
        }
        // `است`, `دست`, `هست`, `نیست`, `درست`, `مست` are ordinary words that happen
        // to end in `ست`, and 732 lexicon entries do the same. Splitting them yields
        // nonsense (`دست` -> `ده است`), and the lexicon's `ا`/`دره`/`مه` entries
        // otherwise make almost any stem look like a word. A whole word that is in
        // the lexicon is the word, not a stem plus a copula.
        if morph.is_known_noun(token) {
            return None;
        }
        if let Some(base) = token.strip_suffix("\u{200C}ست") {
            let formal = self.to_formal(base, morph);
            if morph.is_known_noun(&formal) {
                return Some((formal, "است"));
            }
        }
        // The contraction drops the `ا` of `است` (`کتابست` = `کتاب` + `است`), so
        // the stem is the base exactly as written, whatever it ends in. Appending a
        // `ه` invented a different word instead (`کتابست` -> `کتابه است`,
        // `عالیست` -> `عالیه است`), and because the lexicon holds one-off junk
        // entries the invented word was accepted almost every time.
        if let Some(base) = token.strip_suffix("ست") {
            if base.is_empty() {
                return None;
            }
            let formal = self.to_formal(base, morph);
            if morph.is_known_noun(&formal) {
                return Some((formal, "است"));
            }
        }
        None
    }

    fn try_convert_colloquial_verb(&self, token: &str, morph: &MorphologyEngine) -> Option<String> {
        let (formal_prefix, body) = if let Some(r) = token
            .strip_prefix("نمی\u{200C}")
            .or_else(|| token.strip_prefix("نمی"))
        {
            ("نمی\u{200C}", r)
        } else if let Some(r) = token
            .strip_prefix("می\u{200C}")
            .or_else(|| token.strip_prefix("می"))
        {
            ("می\u{200C}", r)
        } else {
            let r = token.strip_prefix('ب')?;
            ("ب", r)
        };

        for &(colloq_end, formal_end) in COLLOQUIAL_VERB_ENDINGS {
            if let Some(cand_stem) = body.strip_suffix(colloq_end) {
                if cand_stem.is_empty() {
                    continue;
                }
                if let Some(packed) = self.informal_verb_stems.get(cand_stem) {
                    let formal_stem = MorphologyEngine::unpack_lemma(packed, cand_stem);
                    return Some(format!("{}{}{}", formal_prefix, formal_stem, formal_end));
                }
                if morph.is_known_verb_stem(cand_stem) {
                    return Some(format!("{}{}{}", formal_prefix, cand_stem, formal_end));
                }
            }
        }
        None
    }

    fn try_convert_colloquial_noun<'a>(
        &self,
        token: &'a str,
        morph: &MorphologyEngine,
    ) -> Option<Cow<'a, str>> {
        for &(colloq_suf, formal_suf) in COLLOQUIAL_NOUN_SUFFIXES {
            if let Some(stem) = token.strip_suffix(colloq_suf) {
                let formal_stem = self.to_formal(stem, morph);
                if morph.is_known_noun(&formal_stem) {
                    return Some(Cow::Owned(format!("{}{}", formal_stem, formal_suf)));
                }
            }
        }
        for &(c_end, f_end) in &[("ونه", "انه"), ("ون", "ان"), ("وم", "ام")] {
            if let Some(prefix) = token.strip_suffix(c_end) {
                if !prefix.is_empty() {
                    let cand = format!("{}{}", prefix, f_end);
                    if morph.is_known_noun(&cand) {
                        return Some(Cow::Owned(cand));
                    }
                }
            }
        }
        None
    }
}

impl Default for ColloquialConverter {
    fn default() -> Self {
        Self::new()
    }
}
