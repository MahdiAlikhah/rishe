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

/// Copula spellings the noun lexicon also lists as identity rows, which used to
/// shadow the verb table: `است`/`نیست` stayed bare while the persons spelled
/// without them (`هستم`, `هستید`) reduced to `بود`.
const COPULA_FORMS: &[&str] = &["است", "نیست", "هست"];

/// Preverbs the token splitter and the verbal-spacing rule both treat as a
/// prefix. Each feature adds its own extras beside this list.
pub(crate) const SHARED_VERBAL_PREFIXES: &[&str] =
    &["بر", "باز", "در", "فرو", "وا", "ور", "فرا", "پس", "پیش"];

/// The lemma `packed` points at, when the range and the encoding are both valid.
///
/// `packed` can come from a caller, so neither the slice nor the UTF-8 may be
/// assumed: `build.rs` only writes valid lemmas, but it cannot stop someone
/// passing an arbitrary `u64`.
#[inline]
fn lemma_from_blob(packed: u64) -> Option<&'static str> {
    let offset = ((packed & OFFSET_MASK) >> 24) as usize;
    let len = (packed & LEN_MASK) as usize;
    let bytes = LEMMAS_BLOB.get(offset..offset.checked_add(len)?)?;
    std::str::from_utf8(bytes).ok()
}

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
pub(crate) const NON_PARTICIPLE_NOUNS: &[&str] = &[
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
    "دانه",
    "دره",
    "دهه",
    "رایانه",
    "رشته",
    "رنده",
    "رسانه",
    "روزه",
    "سایه",
    "سپرده",
    "سوخته",
    "شکسته",
    "شکنجه",
    "گسترده",
    "گذشته",
    "گروه",
    "گفته",
    "گوشه",
    "گیره",
    "کشته",
    "کنده",
    "مانده",
    "مرده",
    "نامه",
    "نشانه",
    "ناله",
    "وابسته",
    "هسته",
    "آیه",
    "پوشه",
];

// Nouns in the list above that are also genuine participles, so a verb chain
// (`گفته شده است`) still joins; lemmatization treats them as nouns either way.
pub(crate) const PARTICIPLE_NOUNS: &[&str] = &[
    "بسته",
    "پیوسته",
    "سپرده",
    "سوخته",
    "شکسته",
    "گسترده",
    "گذشته",
    "گفته",
    "کشته",
    "کنده",
    "مانده",
    "مرده",
    "وابسته",
];

/// Past and present stems of one verb.
#[derive(Debug, Clone)]
pub struct VerbRoots {
    /// Past stem or infinitive lemma, e.g. `رفت`.
    pub past: String,
    /// Present stem, e.g. `رو`; empty when the source lists none.
    pub present: String,
    /// Separable preverb (`بر` in `برگشت`), or empty. The negations and the
    /// imperfective marker go after it, and the subjunctive takes no `ب`.
    pub preverb: String,
}

/// Split an imperfective verb into its preverb and its body.
///
/// `می‌روم` gives `("", "روم")` and `برمی‌گردم` gives `("بر", "گردم")`, which is
/// the order `conjugation` emits. Returns `None` when the word has no `می`/`نمی`.
fn split_imperfective(verb: &str) -> Option<(&str, &str)> {
    // The marker is surrounded by a ZWNJ, so the split lands on a char boundary.
    for marker in ["نمی", "می"] {
        if let Some(at) = verb.find(marker) {
            let after = &verb[at + marker.len()..];
            let body = after.strip_prefix('\u{200C}').unwrap_or(after);
            if !body.is_empty() {
                return Some((&verb[..at], body));
            }
        }
    }
    None
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

/// Lookup engine over the bundled noun and verb FSTs.
pub struct MorphologyEngine {
    noun_lexicon: Map<&'static [u8]>,
    verb_stems: Map<&'static [u8]>,
    verb_roots: HashMap<String, VerbRoots>,
}

impl MorphologyEngine {
    /// Build the engine from the FSTs bundled at compile time.
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
        for (lemma, keys) in &by_lemma {
            // A lemma whose only key is itself has no present stem in the source.
            // Leaving `present` empty keeps the paradigm honest; filling it with
            // the past stem invented `می‌آرستم` for `آرست`.
            //
            // The source also lists the infinitive as a key (`بیختن` -> `بیخت`).
            // An infinitive is not a present stem, and picking it as the longest
            // key produced `می‌بیختنم`.
            let infinitive = format!("{lemma}ن");
            let present = PREFERRED_PRESENT
                .iter()
                .find(|(past, _)| *past == lemma.as_str())
                .map(|(_, present)| (*present).to_string())
                .or_else(|| {
                    keys.iter()
                        .filter(|key| key.as_str() != lemma.as_str() && **key != infinitive)
                        .max_by_key(|key| key.chars().count())
                        .cloned()
                })
                .unwrap_or_default();
            roots.insert(
                lemma.clone(),
                VerbRoots {
                    past: lemma.clone(),
                    present,
                    preverb: String::new(),
                },
            );
        }

        // A separable preverb (`بر` in `برگشت`) behaves like a prefix of the whole
        // paradigm: `برنگشتم`, not `نبرگشتم`, and `برگردم`, not `ببرگردم`. It is
        // only separable when the stem left over is a verb whose own present stem
        // is exactly what follows the preverb here, which rejects `برید`/`درید`.
        let separables: Vec<(String, String)> = roots
            .iter()
            .filter_map(|(lemma, entry)| {
                let present = entry.present.as_str();
                if present.len() < 3 {
                    return None;
                }
                let prefix = SHARED_VERBAL_PREFIXES
                    .iter()
                    .filter(|prefix| lemma.starts_with(**prefix) && present.starts_with(**prefix))
                    .max_by_key(|prefix| prefix.len())?;
                let rest = &lemma[prefix.len()..];
                let rest_present = &present[prefix.len()..];
                let rest_entry = roots.get(rest)?;
                (rest_entry.present == rest_present).then(|| (lemma.clone(), (*prefix).to_string()))
            })
            .collect();
        for (lemma, preverb) in separables {
            if let Some(entry) = roots.get_mut(&lemma) {
                entry.preverb = preverb;
            }
        }
        roots
    }

    /// Stems for a verb given in any known form, or `None` when it is not in the verb table.
    pub fn verb_roots(&self, verb: &str) -> Option<&VerbRoots> {
        if let Some(roots) = self.direct_roots(verb) {
            return Some(roots);
        }
        // An imperfective form carries `می`/`نمی`, optionally after a separable
        // preverb (`برمی‌گردم`). `conjugate` emits that shape, so it has to read
        // it back or it cannot recognize its own output.
        if let Some((preverb, body)) = split_imperfective(verb) {
            // Look the preverb back up: `گردم` alone resolves to the bare `گشت`,
            // whose own preverb is empty, so the body has to be rejoined with the
            // preverb to reach the entry that carries it.
            let with_preverb = format!("{preverb}{body}");
            for candidate in [with_preverb.as_str(), body] {
                if let Some(roots) = self.direct_roots(candidate) {
                    if preverb.is_empty() || roots.preverb == preverb {
                        return Some(roots);
                    }
                }
            }
            // `گردم` is a present stem plus a person ending, so no table key holds
            // it. Strip the ending and find the entry whose present stem, with the
            // same preverb, is what is left. The past endings resolve the same way
            // for `برمی‌گشتم`, whose body is a past stem plus an ending.
            for ending in crate::conjugation::PRESENT_ENDINGS {
                let Some(stem_body) = body.strip_suffix(ending) else {
                    continue;
                };
                if stem_body.is_empty() {
                    continue;
                }
                let wanted = format!("{preverb}{stem_body}");
                if let Some(roots) = self
                    .verb_roots
                    .values()
                    .find(|roots| roots.present == wanted && roots.preverb == preverb)
                {
                    return Some(roots);
                }
            }
            for ending in crate::conjugation::PAST_ENDINGS {
                let Some(stem_body) = body.strip_suffix(ending) else {
                    continue;
                };
                if stem_body.is_empty() {
                    continue;
                }
                let wanted = format!("{preverb}{stem_body}");
                if let Some(roots) = self
                    .verb_roots
                    .values()
                    .find(|roots| roots.past == wanted && roots.preverb == preverb)
                {
                    return Some(roots);
                }
            }
        }
        let lemma = self.lemmatize(verb);
        if let Some(roots) = self.verb_roots.get(lemma.as_ref()) {
            return Some(roots);
        }
        // A negated preverb form (`برنگشتم`, `برنمی‌گردم`) carries the `ن` between
        // the preverb and the stem, so no table key holds it either.
        if let Some(roots) = self.negated_preverb_roots(verb) {
            return Some(roots);
        }
        None
    }

    /// Resolve a negated form whose `ن` sits between a separable preverb and the
    /// stem, which is where `conjugation` puts it (`بر` + `ن` + `گشتم`).
    fn negated_preverb_roots(&self, verb: &str) -> Option<&VerbRoots> {
        for prefix in SHARED_VERBAL_PREFIXES {
            let Some(rest) = verb.strip_prefix(*prefix) else {
                continue;
            };
            // `نمی‌گردم` is handled by the imperfective path; this is the plain
            // negative past, where `ن` follows the preverb directly.
            if let Some(body) = rest.strip_prefix('ن') {
                if let Some(roots) = self.preverb_stem_roots(prefix, body) {
                    return Some(roots);
                }
            }
            // The subjunctive of a preverb verb is the bare preverb plus a present
            // stem (`برگردم`), with no `می` and no `ن`.
            if let Some(roots) = self.preverb_stem_roots(prefix, rest) {
                if roots.preverb == *prefix {
                    return Some(roots);
                }
            }
        }
        None
    }

    /// The entry whose present stem is `prefix + body-without-a-person-ending`.
    fn preverb_stem_roots(&self, prefix: &str, body: &str) -> Option<&VerbRoots> {
        for ending in crate::conjugation::PRESENT_ENDINGS {
            let Some(stem_body) = body.strip_suffix(ending) else {
                continue;
            };
            if stem_body.is_empty() {
                continue;
            }
            let wanted = format!("{prefix}{stem_body}");
            if let Some(roots) = self
                .verb_roots
                .values()
                .find(|roots| roots.present == wanted && roots.preverb == prefix)
            {
                return Some(roots);
            }
        }
        for ending in crate::conjugation::PAST_ENDINGS {
            let Some(stem_body) = body.strip_suffix(ending) else {
                continue;
            };
            if stem_body.is_empty() {
                continue;
            }
            let wanted = format!("{prefix}{stem_body}");
            if let Some(roots) = self
                .verb_roots
                .values()
                .find(|roots| roots.past == wanted && roots.preverb == prefix)
            {
                return Some(roots);
            }
        }
        None
    }

    /// Look `verb` up as a table key, an infinitive, or a form's stored lemma.
    ///
    /// Split out of [`Self::verb_roots`] so the imperfective path can resolve the
    /// rejoined `preverb + body` by the same three rules.
    fn direct_roots(&self, verb: &str) -> Option<&VerbRoots> {
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
        None
    }

    /// Present stem of a verb lemma, or `None` when the lemma has none.
    pub fn present_stem(&self, lemma: &str) -> Option<&str> {
        self.verb_roots
            .get(lemma)
            .map(|roots| roots.present.as_str())
            .filter(|present| !present.is_empty())
    }

    /// Whether `stem` is a known past stem or verb lemma.
    pub fn is_past_stem(&self, stem: &str) -> bool {
        self.verb_roots.contains_key(stem)
    }

    /// Whether the word is listed as an adjective in the noun lexicon.
    #[inline(always)]
    pub fn is_adjective(&self, word: &str) -> bool {
        const POS_ADJ: u8 = 1 << 1;
        self.noun_lexicon
            .get(word)
            .is_some_and(|packed| ((packed >> 56) as u8 & POS_ADJ) != 0)
    }

    /// Full paradigm for a verb in any known form, or `None` when it is not in the verb table.
    pub fn conjugate(&self, verb: &str) -> Option<crate::conjugation::Conjugation> {
        self.verb_roots(verb)
            .map(crate::conjugation::Conjugation::from_roots)
    }

    /// Unpack the stored lemma for `key`; identity entries borrow `key`.
    #[inline(always)]
    pub fn unpack_lemma<'a>(packed: u64, key: &'a str) -> Cow<'a, str> {
        if (packed & FLAG_IDENTITY) != 0 {
            Cow::Borrowed(key)
        } else {
            // A packed value is caller-supplied, so the range and the encoding
            // must both be checked: the old slice panicked on a wild offset and
            // `from_utf8_unchecked` was UB when an offset split a character.
            lemma_from_blob(packed).map_or(Cow::Borrowed(key), Cow::Borrowed)
        }
    }

    /// Whether the word is in the noun lexicon.
    #[inline(always)]
    pub fn is_known_noun(&self, word: &str) -> bool {
        self.noun_lexicon.contains_key(word)
    }

    /// Whether the string is a known verb stem.
    #[inline(always)]
    pub fn is_known_verb_stem(&self, stem: &str) -> bool {
        self.verb_stems.contains_key(stem)
    }

    /// Insert a ZWNJ after `می`/`نمی` when the rest is a known verb; other tokens are unchanged.
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

    /// Lemma of one token from the noun and verb tables; returns the token when nothing matches.
    #[inline]
    pub fn lemmatize<'a>(&self, token: &'a str) -> Cow<'a, str> {
        // The copula is a verb, but the noun lexicon lists the same spellings as
        // identity rows; the verb table is the authority for these.
        if COPULA_FORMS.contains(&token) {
            if let Some(packed) = self.verb_stems.get(token) {
                if let Some(lemma) = lemma_from_blob(packed) {
                    return Cow::Borrowed(lemma);
                }
            }
        }

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
                // `رفته‌ام` is perfect tense, not a noun `رفته` with a possessive
                // clitic: the verb reading has to win or 1sg/2sg collapse to the
                // participle while 3pl (`رفته‌اند`) still reduced to `رفت`.
                if let Some(verb_lemma) = self.match_verb_lemma(token) {
                    return Cow::Borrowed(verb_lemma);
                }
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
        // The stem has to be a PAST stem: a participle is past stem + `ه`, so
        // accepting any verb stem let `شماره` (`شمار`) and `کافه` (`کاف`) read as
        // participles and turn into the unrelated past stems `شمرد`/`کافت`.
        let participle = token
            .strip_suffix('ه')
            .is_some_and(|stem| self.is_past_stem(stem) && !NON_PARTICIPLE_NOUNS.contains(&token));
        // The noun table is a 187,582-entry surface-frequency list, so it also holds
        // verb forms (`نیامد`, `بیاید`). A ب/ن + ی-glide form of a vowel-initial verb
        // is only treated as a noun when the grammar cannot account for it, i.e.
        // when the token is not actually in the candidate verb's paradigm. This is
        // what keeps `نیاز` (not a form of `آزد`) and `بیار` (not `آرد`) as nouns.
        let glided = self.glided_verb_lemma(token);
        if !participle && glided.is_none() {
            if let Some(packed) = self.noun_lexicon.get(token) {
                return Self::unpack_lemma(packed, token);
            }
        }

        // 4. Conjugated Verbs (past, subjunctive ب..., negative ن...). Try the full token first:
        // بودم/بودند/نوشت are verbs, not ب/ن-prefixed forms.
        if let Some(verb_lemma) = glided {
            return Cow::Borrowed(verb_lemma);
        }
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
                // A clitic key is not a noun stem: `بهمین` is not `بهم` + `ین`.
                if stem.chars().count() >= 2 && !crate::colloquial::is_clitic_key(stem) {
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
    /// Resolve `ب`/`ن` + `ی`-glide forms of a vowel-initial verb.
    ///
    /// `آمد` starts with a vowel, so the negative and subjunctive prefixes insert a
    /// `ی` glide and drop the madda: `نیامدم` is ن + ی + امد, `بیایم` is
    /// ب + ی + ایم. The noun table is a surface-frequency list that also holds these
    /// spellings, so the match is only accepted when the token really is in the
    /// candidate verb's paradigm — otherwise `نیاز` would read as `آزد` and `بیار`
    /// as `آرد`.
    fn glided_verb_lemma(&self, token: &str) -> Option<&'static str> {
        let rest = token
            .strip_prefix('ب')
            .or_else(|| token.strip_prefix('ن'))?;
        let glided = rest.strip_prefix('ی')?;
        let restored = match glided.strip_prefix('ا') {
            Some(after_alef) => format!("آ{after_alef}"),
            None => glided.to_string(),
        };
        let lemma = self.match_verb_lemma(&restored)?;
        let roots = self.verb_roots.get(lemma)?;
        let paradigm = crate::conjugation::Conjugation::from_roots(roots);
        let is_form = [
            &paradigm.negative_past,
            &paradigm.subjunctive,
            &paradigm.negative_present,
        ]
        .into_iter()
        .flatten()
        .any(|form| form == token);
        is_form.then_some(lemma)
    }

    fn match_verb_lemma(&self, body: &str) -> Option<&'static str> {
        // The whole token first: `ترساند` is the past stem of `ترساندن`, but the
        // ending-strip below reads its final `د` as a 3sg present ending, leaving
        // `ترسان`, which is a form of the *different* verb `ترسید`. A stem that is
        // in the table whole is never a stem plus an ending.
        if let Some(packed) = self.verb_stems.get(body) {
            if let Some(lemma) = lemma_from_blob(packed) {
                return Some(lemma);
            }
        }
        for ending in VERB_PERSON_ENDINGS {
            if let Some(cand) = body.strip_suffix(ending) {
                if let Some(packed) = self.verb_stems.get(cand) {
                    if let Some(lemma) = lemma_from_blob(packed) {
                        return Some(lemma);
                    }
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
