use fst::{Map, Streamer};
use std::borrow::Cow;
use std::collections::HashMap;

static NOUNS_FST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/nouns.fst"));
static VERBS_FST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/verbs.fst"));
static LEMMAS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lemmas_blob.bin"));

const FLAG_IDENTITY: u64 = 1 << 55;
const OFFSET_MASK: u64 = 0x007F_FFFF_FF00_0000;
const LEN_MASK: u64 = 0x0000_0000_00FF_FFFF;
pub(crate) const ZWNJ: &str = "\u{200C}";

// Stage A: Unambiguous ZWNJ suffixes (safe to strip BEFORE lexicon lookup)
pub(crate) const ZWNJ_NOUN_SUFFIXES: &[&str] = &[
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
pub(crate) const ATTACHED_NOUN_SUFFIXES: &[&str] = &[
    "هایمان",
    "هایتان",
    "هایشان",
    "هایم",
    "هایت",
    "هایش",
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

// Nouns that end in ه over a verb stem but are lexicalized as nouns, not participles.
const NON_PARTICIPLE_NOUNS: &[&str] = &[
    "آماده",
    "اندازه",
    "اندیشه",
    "بسته",
    "بنده",
    "پایه",
    "پیوسته",
    "تازه",
    "تپه",
    "تکه",
    "خنده",
    "دره",
    "دهه",
    "رایانه",
    "رشته",
    "رنده",
    "رسانه",
    "روزه",
    "سپرده",
    "سوخته",
    "شکسته",
    "شکنجه",
    "گسترده",
    "گذشته",
    "گروه",
    "گفته",
    "کشته",
    "مرده",
    "نامه",
    "نشانه",
    "ناله",
    "وابسته",
];

#[derive(Debug, Clone)]
pub struct VerbRoots {
    pub past: String,
    pub present: String,
}

// Present stems that cannot be picked by "longest variant" because the lexicon also
// contains light-verb alternatives (بود -> است/هست/باش, گفت -> گو/گوی, آمد -> آ/آی).
const PREFERRED_PRESENT: &[(&str, &str)] = &[
    ("بود", "باش"),
    ("آمد", "آی"),
    ("گفت", "گوی"),
    ("داد", "ده"),
    ("شد", "شو"),
    ("کرد", "کن"),
    ("رفت", "رو"),
    ("زد", "زن"),
    ("خواست", "خواه"),
    ("داشت", "دار"),
    ("گرفت", "گیر"),
    ("دید", "بین"),
    ("دانست", "دان"),
    ("توانست", "توان"),
];

pub struct MorphologyEngine {
    noun_lexicon: Map<&'static [u8]>,
    verb_stems: Map<&'static [u8]>,
    verb_roots: HashMap<String, VerbRoots>,
}

impl MorphologyEngine {
    pub fn new() -> Self {
        let noun_lexicon = Map::new(NOUNS_FST_BYTES).expect("Corrupted nouns.fst");
        let verb_stems = Map::new(VERBS_FST_BYTES).expect("Corrupted verbs.fst");
        let verb_roots = Self::build_verb_roots(&verb_stems);
        Self {
            noun_lexicon,
            verb_stems,
            verb_roots,
        }
    }

    fn build_verb_roots(verb_stems: &Map<&'static [u8]>) -> HashMap<String, VerbRoots> {
        let mut by_lemma: HashMap<String, Vec<String>> = HashMap::new();
        let mut stream = verb_stems.stream();
        while let Some((key, packed)) = stream.next() {
            let key = String::from_utf8_lossy(key).into_owned();
            let lemma = Self::unpack_lemma(packed, &key).into_owned();
            by_lemma.entry(lemma).or_default().push(key);
        }

        let mut roots = HashMap::with_capacity(by_lemma.len());
        for (lemma, keys) in by_lemma {
            let present = PREFERRED_PRESENT
                .iter()
                .find(|(past, _)| *past == lemma.as_str())
                .map(|(_, present)| (*present).to_string())
                .or_else(|| {
                    keys.iter()
                        .filter(|key| **key != lemma)
                        .max_by_key(|key| key.chars().count())
                        .cloned()
                })
                .unwrap_or_else(|| lemma.clone());
            roots.insert(
                lemma.clone(),
                VerbRoots {
                    past: lemma,
                    present,
                },
            );
        }
        roots
    }

    pub fn verb_roots(&self, verb: &str) -> Option<&VerbRoots> {
        if let Some(roots) = self.verb_roots.get(verb) {
            return Some(roots);
        }
        if let Some(stem) = verb.strip_suffix('ن') {
            if let Some(roots) = self.verb_roots.get(stem) {
                return Some(roots);
            }
        }
        if let Some(packed) = self.verb_stems.get(verb) {
            let lemma = Self::unpack_lemma(packed, verb);
            if let Some(roots) = self.verb_roots.get(lemma.as_ref()) {
                return Some(roots);
            }
        }
        let lemma = self.lemmatize(verb);
        self.verb_roots.get(lemma.as_ref())
    }

    pub fn present_stem(&self, lemma: &str) -> Option<&str> {
        self.verb_roots
            .get(lemma)
            .map(|roots| roots.present.as_str())
    }

    pub fn conjugate(&self, verb: &str) -> Option<crate::conjugation::Conjugation> {
        self.verb_roots(verb)
            .map(crate::conjugation::Conjugation::from_roots)
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

    #[inline(always)]
    pub fn is_known_noun(&self, word: &str) -> bool {
        self.noun_lexicon.contains_key(word)
    }

    #[inline(always)]
    pub fn is_known_verb_stem(&self, stem: &str) -> bool {
        self.verb_stems.contains_key(stem)
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

        // 3. Direct Noun / Mokassar Lexicon lookup (protects دولت, ایرانی, دانش, تهران from over-stemming!).
        // Past participles (کرده, شده, داده) are in the noun lexicon as identity entries, so let the
        // verb analysis below handle them when the token is a known stem + ه.
        // ponytail: stem+ه heuristic with a stoplist for lexicalized nouns. A POS-aware
        // participle check would generalize better, but needs tagged training data.
        let participle = token.strip_suffix('ه').is_some_and(|stem| {
            self.is_known_verb_stem(stem) && !NON_PARTICIPLE_NOUNS.contains(&token)
        });
        if !participle {
            if let Some(packed) = self.noun_lexicon.get(token) {
                return Self::unpack_lemma(packed, token);
            }
        }

        // 4. Conjugated Verbs (past, subjunctive ب..., negative ن...). Try the full token first:
        // بودم/بودند/نوشت are verbs, not ب/ن-prefixed forms.
        if let Some(verb_lemma) = self.match_verb_lemma(token) {
            return Cow::Borrowed(verb_lemma);
        }
        if let Some(verb_body) = token.strip_prefix('ب').or_else(|| token.strip_prefix('ن')) {
            if let Some(verb_lemma) = self.match_verb_lemma(verb_body) {
                return Cow::Borrowed(verb_lemma);
            }
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

        // 6. Pronoun clitics follow the UD lemma convention (ش -> او, م -> من, ...)
        let clitic = match token {
            "ش" => Some("او"),
            "م" => Some("من"),
            "ت" => Some("تو"),
            "مان" => Some("ما"),
            "تان" => Some("شما"),
            "شان" => Some("آن‌ها"),
            _ => None,
        };
        if let Some(lemma) = clitic {
            return Cow::Borrowed(lemma);
        }

        // 7. Ezafe hamza: همهٔ -> همه, حوزهٔ -> حوزه
        if let Some(stem) = token.strip_suffix('\u{0654}') {
            if let Some(packed) = self.noun_lexicon.get(stem) {
                return Self::unpack_lemma(packed, stem);
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
