use crate::morphology::MorphologyEngine;
use fst::Map;
use std::borrow::Cow;

static INFORMAL_WORDS_FST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/informal_words.fst"));
static INFORMAL_VERBS_FST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/informal_verbs.fst"));

pub enum ExpandedTokens<'a> {
    Single(Cow<'a, str>),
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

const NON_COPULA_STEMS: &[&str] = &[
    "ماست",
    "راست",
    "کاست",
    "خواست",
    "برخاست",
    "پوست",
    "دوست",
    "پیوست",
];

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

pub struct ColloquialConverter {
    informal_words: Map<&'static [u8]>,
    informal_verb_stems: Map<&'static [u8]>,
}

impl ColloquialConverter {
    pub fn new() -> Self {
        Self {
            informal_words: Map::new(INFORMAL_WORDS_FST).expect("Corrupted informal_words.fst"),
            informal_verb_stems: Map::new(INFORMAL_VERBS_FST)
                .expect("Corrupted informal_verbs.fst"),
        }
    }

    #[inline]
    pub fn expand_and_formalize<'a>(
        &self,
        token: &'a str,
        morph: &MorphologyEngine,
    ) -> ExpandedTokens<'a> {
        for &(colloq, (prep, pron)) in FUSED_PREP_CLITICS {
            if token == colloq {
                return ExpandedTokens::Pair(Cow::Borrowed(prep), Cow::Borrowed(pron));
            }
        }
        if let Some((stem, copula)) = self.try_split_copula(token, morph) {
            return ExpandedTokens::Pair(stem, Cow::Borrowed(copula));
        }
        ExpandedTokens::Single(self.to_formal(token, morph))
    }

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
        if let Some(base) = token.strip_suffix("\u{200C}ست") {
            let formal = self.to_formal(base, morph);
            if morph.is_known_noun(&formal) {
                return Some((formal, "است"));
            }
        }
        if let Some(base) = token.strip_suffix("ست") {
            if base.ends_with('ا') || base.ends_with('و') {
                let formal = self.to_formal(base, morph);
                if morph.is_known_noun(&formal) {
                    return Some((formal, "است"));
                }
            }
            let mut restored = String::with_capacity(base.len() + 'ه'.len_utf8());
            restored.push_str(base);
            restored.push('ه');
            let formal = self.to_formal(&restored, morph);
            if morph.is_known_noun(&formal) {
                return Some((Cow::Owned(formal.into_owned()), "است"));
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
