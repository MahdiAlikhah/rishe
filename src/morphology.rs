use fst::Map;
use std::borrow::Cow;

static NOUNS_FST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/nouns.fst"));
static VERBS_FST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/verbs.fst"));
static LEMMAS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lemmas_blob.bin"));

// Bitflag constants matching Python extract_lexicons.py
pub const POS_NOUN: u8 = 1 << 0; // 0x01
pub const POS_ADJ: u8 = 1 << 1; // 0x02
pub const POS_PROPN: u8 = 1 << 2; // 0x04
pub const POS_PRON: u8 = 1 << 3; // 0x08
pub const POS_ADV: u8 = 1 << 4; // 0x10
pub const POS_NUM: u8 = 1 << 5; // 0x20
pub const POS_VERB: u8 = 1 << 7; // 0x80

const FLAG_IDENTITY: u64 = 1 << 55;
const OFFSET_MASK: u64 = 0x007F_FFFF_FF00_0000;
const LEN_MASK: u64 = 0x0000_0000_00FF_FFFF;
const ZWNJ: &str = "\u{200C}";

// Stage A: Unambiguous ZWNJ suffixes (safe to strip BEFORE lexicon lookup)
const ZWNJ_NOUN_SUFFIXES: &[&str] = &[
    "\u{200C}هایمان",
    "\u{200C}هایتان",
    "\u{200C}هایشان",
    "\u{200C}هایم",
    "\u{200C}هایت",
    "\u{200C}هایش",
    "\u{200C}هایی",
    "\u{200C}های",
    "\u{200C}ها",
    "\u{200C}ترین",
    "\u{200C}تر",
    "\u{200C}ام",
    "\u{200C}ات",
    "\u{200C}اش",
    "\u{200C}ای",
];

// Stage B: Attached suffixes (ONLY strip if the full word is NOT in noun_lexicon)
const ATTACHED_NOUN_SUFFIXES: &[&str] = &[
    "هایمان",
    "هایتان",
    "هایشان",
    "هایی",
    "های",
    "ها",
    "مان",
    "تان",
    "شان",
    "گان",
    "یان",
    "ان",
    "ات",
    "ین",
    "ام",
    "ات",
    "اش",
    "م",
    "ت",
    "ش",
    "ی",
];

const VERB_PERSON_ENDINGS: &[&str] = &[
    // Present perfect / participle endings with ZWNJ (e.g., رفته‌اند -> رفت, کرده‌ام -> کرد)
    "ه\u{200C}ایم",
    "ه\u{200C}اید",
    "ه\u{200C}اند",
    "ه\u{200C}ام",
    "ه\u{200C}ای",
    "ه\u{200C}است",
    // Standard person endings
    "\u{200C}ام",
    "\u{200C}ای",
    "\u{200C}است",
    "\u{200C}ایم",
    "\u{200C}اید",
    "\u{200C}اند",
    "ایم",
    "اید",
    "اند",
    "ام",
    "ای",
    "ند",
    "ید",
    "یم",
    "م",
    "ی",
    "د",
    "ه",
    "",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NounEntry<'a> {
    pub lemma: Cow<'a, str>,
    pub pos_flags: u8,
}

pub struct MorphologyEngine {
    noun_lexicon: Map<&'static [u8]>,
    verb_stems: Map<&'static [u8]>,
}

impl MorphologyEngine {
    pub fn new() -> Self {
        Self {
            noun_lexicon: Map::new(NOUNS_FST_BYTES).expect("Corrupted nouns.fst"),
            verb_stems: Map::new(VERBS_FST_BYTES).expect("Corrupted verbs.fst"),
        }
    }

    #[inline(always)]
    pub fn unpack_pos_flags(packed: u64) -> u8 {
        (packed >> 56) as u8
    }

    #[inline(always)]
    pub fn unpack_lemma<'a>(packed: u64, key: &'a str) -> Cow<'a, str> {
        if (packed & FLAG_IDENTITY) != 0 {
            Cow::Borrowed(key)
        } else {
            let offset = ((packed & OFFSET_MASK) >> 24) as usize;
            let len = (packed & LEN_MASK) as usize;
            let bytes = &LEMMAS_BLOB[offset..offset + len];
            // SAFETY: build.rs only writes valid UTF-8 lemmas into LEMMAS_BLOB
            Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(bytes) })
        }
    }

    /// Returns the canonical lemma AND POS bitflags in a single FST lookup.
    #[inline]
    pub fn lookup_noun<'a>(&self, key: &'a str) -> Option<NounEntry<'a>> {
        self.noun_lexicon.get(key).map(|packed| NounEntry {
            lemma: Self::unpack_lemma(packed, key),
            pos_flags: Self::unpack_pos_flags(packed),
        })
    }

    #[inline(always)]
    pub fn is_known_noun(&self, word: &str) -> bool {
        self.noun_lexicon.contains_key(word)
    }

    #[inline(always)]
    pub fn is_known_verb_stem(&self, stem: &str) -> bool {
        self.verb_stems.contains_key(stem)
    }

    #[inline(always)]
    pub fn is_propn(&self, word: &str) -> bool {
        self.noun_lexicon
            .get(word)
            .is_some_and(|p| (Self::unpack_pos_flags(p) & POS_PROPN) != 0)
    }

    #[inline(always)]
    pub fn is_adjective(&self, word: &str) -> bool {
        self.noun_lexicon
            .get(word)
            .is_some_and(|p| (Self::unpack_pos_flags(p) & POS_ADJ) != 0)
    }

    #[inline]
    pub fn fix_spaceless_verb<'a>(&self, token: &'a str) -> Cow<'a, str> {
        if self.is_known_noun(token) {
            return Cow::Borrowed(token);
        }
        for prefix in &["نمی", "می"] {
            if let Some(rest) = token.strip_prefix(prefix) {
                if !rest.starts_with(ZWNJ) && self.match_verb_lemma(rest).is_some() {
                    let mut fixed = String::with_capacity(token.len() + ZWNJ.len());
                    fixed.push_str(prefix);
                    fixed.push_str(ZWNJ);
                    fixed.push_str(rest);
                    return Cow::Owned(fixed);
                }
            }
        }
        Cow::Borrowed(token)
    }

    #[inline]
    pub fn lemmatize<'a>(&self, token: &'a str) -> Cow<'a, str> {
        // 1. Explicit imperfective verbs (می‌... / نمی‌...) always go to verb lemmatizer first
        if let Some(verb_body) = token
            .strip_prefix("نمی\u{200C}")
            .or_else(|| token.strip_prefix("می\u{200C}"))
        {
            if let Some(verb_lemma) = self.match_verb_lemma(verb_body) {
                return Cow::Borrowed(verb_lemma);
            }
        }

        // 2. Strip unambiguous ZWNJ-attached plural/comparative suffixes first (e.g., کتاب‌ها -> کتاب)
        for suffix in ZWNJ_NOUN_SUFFIXES {
            if let Some(stem) = token.strip_suffix(suffix) {
                if let Some(packed) = self.noun_lexicon.get(stem) {
                    return Self::unpack_lemma(packed, stem);
                }
            }
        }

        // 3. Direct Noun / Mokassar Lexicon lookup (protects دولت, ایرانی, دانش, تهران from over-stemming!)
        if let Some(packed) = self.noun_lexicon.get(token) {
            return Self::unpack_lemma(packed, token);
        }

        // 4. Conjugated Verbs (past, subjunctive ب..., negative ن...)
        let verb_body = token
            .strip_prefix('ب')
            .or_else(|| token.strip_prefix('ن'))
            .unwrap_or(token);

        if let Some(verb_lemma) = self.match_verb_lemma(verb_body) {
            return Cow::Borrowed(verb_lemma);
        }

        // 5. Fallback: Strip attached suffixes ONLY when the surface word wasn't in noun_lexicon
        for suffix in ATTACHED_NOUN_SUFFIXES {
            if let Some(stem) = token.strip_suffix(suffix) {
                if stem.chars().count() >= 2 {
                    if *suffix == "گان" {
                        let restored = format!("{}ه", stem);
                        if let Some(packed) = self.noun_lexicon.get(&restored) {
                            return Cow::Owned(Self::unpack_lemma(packed, &restored).into_owned());
                        }
                    }
                    if let Some(packed) = self.noun_lexicon.get(stem) {
                        return Self::unpack_lemma(packed, stem);
                    }
                }
            }
        }

        Cow::Borrowed(token)
    }

    #[inline]
    fn match_verb_lemma(&self, body: &str) -> Option<&'static str> {
        for ending in VERB_PERSON_ENDINGS {
            if let Some(cand) = body.strip_suffix(ending) {
                if let Some(packed) = self.verb_stems.get(cand) {
                    let offset = ((packed & OFFSET_MASK) >> 24) as usize;
                    let len = (packed & LEN_MASK) as usize;
                    let bytes = &LEMMAS_BLOB[offset..offset + len];
                    return Some(unsafe { std::str::from_utf8_unchecked(bytes) });
                }
            }
        }
        None
    }
}

impl Default for MorphologyEngine {
    fn default() -> Self {
        Self::new()
    }
}
