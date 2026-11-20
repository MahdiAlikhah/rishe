use crate::inflector::Inflector;
use crate::morphology::{MorphologyEngine, SHARED_VERBAL_PREFIXES};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasherDefault;
use std::sync::OnceLock;

/// Hasher for tables keyed by `char`: the key is its own hash. SipHash13 costs
/// more than the lookup it guards, and these tables are read per character.
#[derive(Default)]
struct CharHasher(u64);

impl std::hash::Hasher for CharHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 << 8) ^ u64::from(*byte);
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.0 = u64::from(value);
    }
}

type CharMap = HashMap<char, &'static str, BuildHasherDefault<CharHasher>>;

const ZWNJ: char = '\u{200C}';

pub(crate) const PERSIAN_LETTERS: &str = "آابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهیءؤۀأئ";
pub(crate) const NON_LEFT_JOINERS: &str = "دۀذاأآورژز";
const SINGLE_PUNCT: &str = "…،:.؟!؛";
const OPENER_PUNCT: &str = ">{[(«";
const CLOSER_PUNCT: &str = "<}])»";
pub(crate) const ALL_PUNCT: &str = "…،:.؟!؛>{[(«<}])»-٪@/#";
const INVISIBLE: &str =
    "\u{200B}\u{200D}\u{200E}\u{200F}\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}\u{2066}\u{2067}\u{2068}\u{2069}\u{FEFF}";

const VERB_SUFFIXES: &[&str] = &["ام", "ای", "است", "ایم", "اید", "اند"];
const PERFECT_SUFFIXES: &[&str] = &[
    "\u{200C}ام",
    "\u{200C}ای",
    "\u{200C}ایم",
    "\u{200C}اید",
    "\u{200C}اند",
    " است",
    "\u{200C}است",
];
const VERB_PERSON_ENDINGS: &[&str] = &["م", "ی", "د", "یم", "ید", "ند"];

const PREFIXES: &[&str] = &[
    "برون", "تک", "درون", "زیست", "میان", "نیم", "سوء", "ضد", "غیر", "بی", "هم", "نصفه", "پاره",
    "نیمه", "نا", "خوش",
];

const MORPH_SUFFIXES: &[&str] = &[
    "ام",
    "ات",
    "اش",
    "مان",
    "تان",
    "شان",
    "ای",
    "مون",
    "تون",
    "شون",
    "تر",
    "تری",
    "ترین",
    "ها",
    "های",
    "هایی",
    "هایم",
    "هایت",
    "هایش",
    "هایمان",
    "هایتان",
    "هایشان",
    "هام",
    "هات",
    "هاش",
    "هامون",
    "هاتون",
    "هاشون",
];

const WORD_SUFFIXES_BASE: &[&str] = &[
    "گر",
    "گرا",
    "آسا",
    "فام",
    "جات",
    "وش",
    "آگین",
    "وار",
    "نگر",
    "زا",
    "آمیز",
    "زدا",
    "فرسا",
    "سنج",
    "گشا",
    "سپار",
    "طلب",
    "یاب",
    "شمول",
    "پیما",
    "بند",
    "نویس",
    "نشین",
    "انگیز",
    "کش",
    "آور",
    "آلود",
    "گزین",
    "ساز",
    "کنان",
    "دار",
    "ریز",
    "گستر",
    "شناس",
    "پذیر",
    "ناپذیر",
    "پراکن",
    "پژوه",
    "پرست",
    "پرور",
    "گون",
    "دهی",
    "ستیز",
    "رسان",
    "نما",
    "ورز",
    "پرداز",
    "افکن",
    "خیز",
    "اندیش",
    "گیر",
    "گذار",
    "پسند",
    "فشان",
    "بخش",
];

const E_SUFFIXES: &[&str] = &[
    "باره",
    "بارگی",
    "گیرنده",
    "گیرندگی",
    "کارانه",
    "دهنده",
    "دهندگی",
    "کننده",
    "کنندگی",
    "مایه",
    "مایگی",
    "گرایانه",
    "افکنانه",
    "شدگی",
    "افتادگی",
    "زدگی",
    "شونده",
    "شوندگی",
    "گونه",
    "گونگی",
    "پیشه",
    "پیشگی",
];

/// How a word-final `ه ی` ending is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YaStyle {
    /// `ۀ` (standard orthography).
    Standard,
    /// `ه‌ی` with a ZWNJ.
    Joda,
}

/// One normalization step; a stage list applies steps in the order given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizeOp {
    /// Map Arabic presentation forms and letter variants to Persian letters.
    Alphabet,
    /// Expand Arabic ligatures into Persian words.
    ArabicUnicode,
    /// Normalize the word-final `ه‌ی`/`ۀ` ending to the given style.
    Ya(YaStyle),
    /// Map Arabic, math and circled digit forms to Persian digits.
    Digits,
    /// Map punctuation variants to their Persian forms.
    Punctuation,
    /// Truncate runs of a character to two and strip kashida.
    RepeatedLetters,
    /// Run orthographic, punctuation, word and verbal spacing in that order.
    Spacing,
    /// Collapse whitespace, drop invisible controls and fix ZWNJ placement.
    OrthographicSpacing,
    /// Attach punctuation to its neighbouring word without a space.
    PunctuationSpacing,
    /// Join a separate suffix or prefix to a known word.
    WordSpacing,
    /// Join `می`/`نمی` and verb suffixes to known verb forms.
    VerbalSpacing,
}

impl NormalizeOp {
    /// Resolve an operator name or alias, or `None` when unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "alphabet" => Self::Alphabet,
            "arabic_unicode" | "arabic-unicode" | "ligatures" => Self::ArabicUnicode,
            "ya" | "ya_standard" | "ya-standard" => Self::Ya(YaStyle::Standard),
            "ya_joda" | "ya-joda" => Self::Ya(YaStyle::Joda),
            "digits" | "digit" => Self::Digits,
            "punctuation" | "punctuations" => Self::Punctuation,
            "repeated_letters" | "repeated-letters" => Self::RepeatedLetters,
            "spacing" => Self::Spacing,
            "orthographic_spacing" | "orthographic-spacing" => Self::OrthographicSpacing,
            "punctuation_spacing" | "punctuation-spacing" => Self::PunctuationSpacing,
            "word_spacing" | "word-spacing" => Self::WordSpacing,
            "verbal_spacing" | "verbal-spacing" => Self::VerbalSpacing,
            _ => return None,
        })
    }
}

/// Apply one operator, borrowing the input when nothing changes.
pub fn apply<'a>(input: &'a str, op: NormalizeOp, morph: &MorphologyEngine) -> Cow<'a, str> {
    match op {
        NormalizeOp::Alphabet => alphabet(input),
        NormalizeOp::ArabicUnicode => arabic_unicode(input),
        NormalizeOp::Ya(style) => ya(input, style),
        NormalizeOp::Digits => digits(input),
        NormalizeOp::Punctuation => punctuation(input),
        NormalizeOp::RepeatedLetters => repeated_letters(input),
        NormalizeOp::Spacing => spacing(input, morph),
        NormalizeOp::OrthographicSpacing => orthographic_spacing(input),
        NormalizeOp::PunctuationSpacing => punctuation_spacing(input),
        NormalizeOp::WordSpacing => word_spacing(input, morph),
        NormalizeOp::VerbalSpacing => verbal_spacing(input, morph),
    }
}

static ALPHABET_TABLE: OnceLock<CharMap> = OnceLock::new();
static ARABIC_UNICODE_TABLE: OnceLock<CharMap> = OnceLock::new();
static DIGIT_TABLE: OnceLock<CharMap> = OnceLock::new();
static PUNCTUATION_TABLE: OnceLock<CharMap> = OnceLock::new();
static WORD_SUFFIXES: OnceLock<HashSet<String>> = OnceLock::new();
static SORTED_MORPH_SUFFIXES: OnceLock<Vec<&'static str>> = OnceLock::new();

/// Map Arabic presentation forms and letter variants to Persian letters.
pub fn alphabet(input: &str) -> Cow<'_, str> {
    mapped(input, ALPHABET_TABLE.get_or_init(|| table(ALPHABET_GROUPS)))
}

/// The `alphabet` replacement for one character, if the table has one.
#[inline]
pub(crate) fn alphabet_char(ch: char) -> Option<&'static str> {
    ALPHABET_TABLE
        .get_or_init(|| table(ALPHABET_GROUPS))
        .get(&ch)
        .copied()
}

/// Expand Arabic ligature characters into Persian words.
pub fn arabic_unicode(input: &str) -> Cow<'_, str> {
    mapped(
        input,
        ARABIC_UNICODE_TABLE.get_or_init(|| table(ARABIC_UNICODE_GROUPS)),
    )
}

/// Map Arabic, math and circled digits to Persian digits.
pub fn digits(input: &str) -> Cow<'_, str> {
    mapped(input, DIGIT_TABLE.get_or_init(|| table(DIGIT_GROUPS)))
}

/// Map punctuation variants to Persian punctuation.
pub fn punctuation(input: &str) -> Cow<'_, str> {
    mapped(
        input,
        PUNCTUATION_TABLE.get_or_init(|| table(PUNCTUATION_GROUPS)),
    )
}

/// Normalize the word-final `ه‌ی`/`ه ی` ending to the given style.
pub fn ya(input: &str, style: YaStyle) -> Cow<'_, str> {
    // `ه‌ی` is the word-final ending only when the word ends there, so
    // `خانه‌یی` must stay as it is. `ۀ` is always word-final, so it is replaced
    // wherever it appears.
    let (replaced, replacement) = match style {
        YaStyle::Standard => (replace_at_word_end(input, "ه\u{200C}ی", "ۀ"), "ۀ"),
        YaStyle::Joda => (
            if input.contains('ۀ') {
                Cow::Owned(input.replace('ۀ', "ه\u{200C}ی"))
            } else {
                Cow::Borrowed(input)
            },
            "ه\u{200C}ی",
        ),
    };
    match replaced {
        Cow::Borrowed(text) => ya_ha_yeh_space(text, replacement),
        Cow::Owned(text) => Cow::Owned(ya_ha_yeh_space(&text, replacement).into_owned()),
    }
}

/// Truncate runs of a letter to two, strip kashida and trim the result.
///
/// Only letters: `...` is an ellipsis and `!!!` is emphasis, not a repeated
/// character, and digits/whitespace were already exempt.
pub fn repeated_letters(input: &str) -> Cow<'_, str> {
    let mut out = String::with_capacity(input.len());
    let mut prev: Option<char> = None;
    let mut run = 0usize;
    let mut changed = false;

    for ch in input.chars() {
        if ch == '\u{0640}' {
            changed = true;
            continue;
        }
        if prev == Some(ch) {
            run += 1;
        } else {
            prev = Some(ch);
            run = 1;
        }
        if run > 2 && ch.is_alphabetic() {
            changed = true;
            continue;
        }
        out.push(ch);
    }

    // Collapsing a run is this stage's job; trimming leading/trailing space is
    // not, and it silently changed text the caller passed through untouched — and
    // it also made the "nothing changed" fast path unreachable whenever the input
    // had a space at either end.
    if !changed {
        return Cow::Borrowed(input);
    }
    Cow::Owned(out)
}

/// Collapse horizontal whitespace, drop invisible controls and keep at most one blank line.
pub fn orthographic_spacing(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    let mut laid_out = String::with_capacity(input.len());
    let mut last_horizontal = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        // A carriage return is a line ending on its own (old Mac) or the first
        // half of CRLF. Dropping it joined the two lines: `خط اول\rخط دوم` lost
        // its break and CRLF files looked like one long line.
        if ch == '\r' {
            if chars.peek() != Some(&'\n') {
                laid_out.push('\n');
                last_horizontal = false;
            }
            continue;
        }
        if INVISIBLE.contains(ch) {
            continue;
        }
        if ch == '\n' {
            while laid_out.ends_with(' ') {
                laid_out.pop();
            }
            let newlines = laid_out.chars().rev().take_while(|c| *c == '\n').count();
            if newlines < 2 {
                laid_out.push('\n');
            }
            last_horizontal = false;
        } else if ch.is_whitespace() {
            if !last_horizontal {
                laid_out.push(' ');
                last_horizontal = true;
            }
        } else {
            laid_out.push(ch);
            last_horizontal = false;
        }
    }

    // The first pass already collapses horizontal whitespace, so the extra
    // `collapse_horizontal` pass only copied the string again. Each later step is
    // skipped when it cannot change anything, and the input is borrowed when the
    // result equals it.
    let mut out = if laid_out.contains("هها") {
        laid_out.replace("هها", "ه\u{200C}ها")
    } else {
        laid_out
    };
    if out.contains(ZWNJ) {
        out = clean_zwnj(&out);
    }
    let leading = out.len() - out.trim_start().len();
    out.drain(..leading);
    out.truncate(out.trim_end().len());
    if out == input {
        return Cow::Borrowed(input);
    }
    Cow::Owned(out)
}

/// Attach punctuation to its neighbouring word without a space.
pub fn punctuation_spacing(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }
    // Every rule below moves a space next to a punctuation character, so text
    // with none — a single token, say — cannot change.
    if !input.chars().any(|ch| {
        OPENER_PUNCT.contains(ch) || CLOSER_PUNCT.contains(ch) || SINGLE_PUNCT.contains(ch)
    }) {
        return Cow::Borrowed(input);
    }

    let chars: Vec<char> = input.chars().collect();
    let chars = strip_space_after_openers(chars);
    let chars = strip_space_before_closers(chars);
    let chars = insert_space_before_openers(chars);
    let chars = insert_space_after_singles(chars);
    Cow::Owned(finalize_spacing(chars))
}

/// Join known affixes to known words; returns the input borrowed when nothing changes.
pub fn word_spacing<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let joined = join_spans(input, |prev, next| try_join_words(prev, next, morph));
    let with_suffixes = join_attached_morph_suffixes(&joined, morph);
    match with_suffixes {
        Cow::Borrowed(_) if matches!(joined, Cow::Borrowed(_)) => Cow::Borrowed(input),
        other => Cow::Owned(other.into_owned()),
    }
}

/// Join `می`/`نمی` and verb suffixes to known verb forms; returns the input
/// borrowed when nothing changes.
pub fn verbal_spacing<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let after_preverbs = join_preverb_mi(input, morph);
    let after_suffixes = join_spans(&after_preverbs, |prev, next| {
        let (core, punct) = split_trailing_punct(next);
        if !VERB_SUFFIXES.contains(&core) || !is_persian_word(prev) {
            return None;
        }
        let joined = join_parts(prev, core);
        if is_verb_form(&joined, morph) {
            return Some(format!("{joined}{punct}"));
        }
        None
    });
    let after_mi = join_spans(&after_suffixes, |prev, next| {
        let (core, punct) = split_trailing_punct(next);
        if (prev == "می" || prev == "نمی") && is_persian_word(core) {
            let joined = format!("{prev}\u{200C}{core}");
            if is_verb_form(&joined, morph) {
                return Some(format!("{joined}{punct}"));
            }
        }
        None
    });
    let after_joined_mi = join_mi_tokens(&after_mi, morph);
    let after_ast = fix_ast_spacing(&after_joined_mi);
    let changed = matches!(after_preverbs, Cow::Owned(_))
        || matches!(after_suffixes, Cow::Owned(_))
        || matches!(after_mi, Cow::Owned(_))
        || matches!(after_joined_mi, Cow::Owned(_))
        || matches!(after_ast, Cow::Owned(_));
    if changed {
        Cow::Owned(after_ast.into_owned())
    } else {
        Cow::Borrowed(input)
    }
}

/// Run orthographic, punctuation, word and verbal spacing in that order.
pub fn spacing<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let first = orthographic_spacing(input);
    let second = punctuation_spacing(&first);
    let third = word_spacing(&second, morph);
    // The last stage owns its result whenever it changed anything, so hand that
    // buffer back instead of copying it once more.
    if let Cow::Owned(text) = verbal_spacing(&third, morph) {
        return Cow::Owned(text);
    }
    if matches!(
        (&first, &second, &third),
        (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
    ) {
        return Cow::Borrowed(input);
    }
    Cow::Owned(third.into_owned())
}

fn mapped<'a>(input: &'a str, table: &CharMap) -> Cow<'a, str> {
    let mut buf: Option<String> = None;
    for (idx, ch) in input.char_indices() {
        match table.get(&ch) {
            Some(rep) => {
                let b = buf.get_or_insert_with(|| {
                    let mut s = String::with_capacity(input.len() + 8);
                    s.push_str(&input[..idx]);
                    s
                });
                b.push_str(rep);
            }
            None => {
                if let Some(b) = buf.as_mut() {
                    b.push(ch);
                }
            }
        }
    }
    match buf {
        Some(s) => Cow::Owned(s),
        None => Cow::Borrowed(input),
    }
}

fn table(groups: &'static [(&'static str, &'static str)]) -> CharMap {
    let mut map =
        CharMap::with_capacity_and_hasher(groups.len() * 4, BuildHasherDefault::default());
    for (chars, replacement) in groups {
        for ch in chars.chars() {
            map.insert(ch, *replacement);
        }
    }
    map
}

fn replace_at_word_end<'a>(input: &'a str, needle: &str, replacement: &str) -> Cow<'a, str> {
    let mut out: Option<String> = None;
    let mut last = 0;

    for (idx, _) in input.match_indices(needle) {
        let after = idx + needle.len();
        // Punctuation ends a word too: `خانه‌ی.` used to stay unconverted while
        // `خانه‌ی ما` became `خانۀ ما`, so the same word got two spellings
        // depending on what followed it.
        let boundary = after == input.len()
            || input[after..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_whitespace() || ALL_PUNCT.contains(ch));
        if boundary {
            let b = out.get_or_insert_with(|| String::with_capacity(input.len() + 4));
            b.push_str(&input[last..idx]);
            b.push_str(replacement);
            last = after;
        }
    }

    match out {
        Some(mut b) => {
            b.push_str(&input[last..]);
            Cow::Owned(b)
        }
        None => Cow::Borrowed(input),
    }
}

fn ya_ha_yeh_space<'a>(input: &'a str, replacement: &str) -> Cow<'a, str> {
    replace_at_word_end(input, "ه ی", replacement)
}

fn clean_zwnj(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != ZWNJ {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        while i + 1 < chars.len() && chars[i + 1] == ZWNJ {
            i += 1;
        }
        let prev = out.chars().next_back();
        let next = chars.get(i + 1).copied();
        let between_script =
            prev.is_some_and(is_script_or_digit) && next.is_some_and(is_script_or_digit);
        let near_space = prev == Some(' ') || next == Some(' ');
        // A ZWNJ after a non-joining letter is visually neutral and often the
        // only separator in a compound (`آذر‌ماه`, `نماز‌جمعه`); deleting it
        // destroyed lexicon words and fused `آن‌ها‌است` into `آنهااست`.
        if between_script && !near_space {
            out.push(ZWNJ);
        }
        i += 1;
    }

    out
}

fn is_script_or_digit(ch: char) -> bool {
    matches!(
        ch,
        '\u{0600}'..='\u{06FF}'
            | '\u{0750}'..='\u{077F}'
            | '\u{08A0}'..='\u{08FF}'
            | '\u{FB50}'..='\u{FDFF}'
            | '\u{FE70}'..='\u{FEFF}'
            | '0'..='9'
    )
}

fn collapse_horizontal(input: impl IntoIterator<Item = char>) -> String {
    let input = input.into_iter();
    let mut out = String::with_capacity(input.size_hint().0);
    let mut in_space = false;
    for ch in input {
        if is_horizontal(ch) {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

fn strip_space_after_openers(chars: Vec<char>) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len());
    let mut skip_space = false;
    for (i, ch) in chars.into_iter().enumerate() {
        if skip_space && is_horizontal(ch) {
            continue;
        }
        skip_space = OPENER_PUNCT.contains(ch) || (i == 0 && SINGLE_PUNCT.contains(ch));
        out.push(ch);
    }
    out
}

fn strip_space_before_closers(chars: Vec<char>) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len());
    let mut pending_space = false;
    for ch in chars {
        if is_horizontal(ch) {
            pending_space = true;
            continue;
        }
        if pending_space && !CLOSER_PUNCT.contains(ch) && !SINGLE_PUNCT.contains(ch) {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
    }
    out
}

fn insert_space_before_openers(chars: Vec<char>) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len() + 8);
    for ch in chars {
        // A run of openers is one bracket pair (`((ب))`), so it opens with one
        // space, not one per character: `((ب))` became `( (ب))`, which no longer
        // matches the text it came from.
        let after_opener = out.last().is_some_and(|c: &char| OPENER_PUNCT.contains(*c));
        if OPENER_PUNCT.contains(ch)
            && !after_opener
            && out.last().is_some_and(|c: &char| !c.is_whitespace())
        {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// Indices that sit inside a token spacing must not split.
///
/// A URL, a path or a file name is one token: `https://rishe.example/docs` became
/// `https: //rishe. example/docs`, which no reader or parser accepts. Spacing
/// belongs between words, so every index of such a token is marked.
fn unsplittable_tokens(chars: &[char]) -> Vec<bool> {
    let mut protected = vec![false; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let token = &chars[start..i];
        let has_scheme = token
            .windows(3)
            .any(|window| window[0] == ':' && window[1] == '/' && window[2] == '/');
        let has_www = token.len() >= 4
            && token[0] == 'w'
            && token[1] == 'w'
            && token[2] == 'w'
            && token[3] == '.';
        let has_both = token.contains(&':') && token.contains(&'/');
        if has_scheme || has_www || has_both {
            for slot in &mut protected[start..i] {
                *slot = true;
            }
        }
    }
    protected
}

fn is_digit_char(ch: char) -> bool {
    // ASCII, Persian and Arabic-Indic digits. A local range check keeps this
    // module free of a dependency on `maskers`.
    ch.is_ascii_digit() || matches!(ch as u32, 0x06F0..=0x06F9 | 0x0660..=0x0669)
}

fn insert_space_after_singles(chars: Vec<char>) -> Vec<char> {
    // `3.14`, `12:30` and `v1.0.4` read as one value, so a `.`/`:` between two
    // digits is not a word boundary. Digits on both sides is the whole test: a
    // full stop after a number (`سال ۱۴۰۰.`) still gets its space.
    let unsplittable = unsplittable_tokens(&chars);
    let mut out = Vec::with_capacity(chars.len() + 8);
    for (i, ch) in chars.iter().copied().enumerate() {
        out.push(ch);
        // A leading `،`/`…` separates from the word after it like any other. It
        // was exempt at index 0, so `، دنیا` became `،دنیا` — the punctuation was
        // glued to the next word, the opposite of the rule this stage enforces.
        if SINGLE_PUNCT.contains(ch) || CLOSER_PUNCT.contains(ch) {
            if unsplittable.get(i).copied().unwrap_or(false) {
                continue;
            }
            if let Some(&next) = chars.get(i + 1) {
                let inside_number = i
                    .checked_sub(1)
                    .and_then(|j| chars.get(j).copied())
                    .is_some_and(is_digit_char)
                    && is_digit_char(next);
                if !inside_number
                    && !next.is_whitespace()
                    && next != ZWNJ
                    && !SINGLE_PUNCT.contains(next)
                    && !CLOSER_PUNCT.contains(next)
                {
                    out.push(' ');
                }
            }
        }
    }
    out
}

fn finalize_spacing(chars: Vec<char>) -> String {
    // Trim inside the buffer; `collapse(..).trim().to_string()` allocated twice.
    let mut out = collapse_horizontal(chars);
    let leading = out.len() - out.trim_start_matches(is_horizontal).len();
    out.drain(..leading);
    out.truncate(out.trim_end_matches(is_horizontal).len());
    out
}

fn is_horizontal(ch: char) -> bool {
    ch.is_whitespace() && ch != '\n' && ch != '\r'
}

fn word_spans(input: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    for (i, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                spans.push((s, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        spans.push((s, input.len()));
    }
    spans
}

fn split_trailing_punct(token: &str) -> (&str, &str) {
    let mut end = token.len();
    for (i, ch) in token.char_indices().rev() {
        if is_punctuation(ch) {
            end = i;
        } else {
            break;
        }
    }
    (&token[..end], &token[end..])
}

fn is_persian_word(word: &str) -> bool {
    !word.is_empty() && word.chars().all(is_persian_letter)
}

/// Bitmap of `letters`: the hot loops ask about one character at a time, and
/// scanning the 38-character string for each of them is the slow way to do it.
fn letter_bits(letters: &str) -> [bool; 0x3000] {
    let mut bits = [false; 0x3000];
    for ch in letters.chars() {
        if let Some(slot) = bits.get_mut(ch as usize) {
            *slot = true;
        }
    }
    bits
}

fn persian_letter_bits() -> &'static [bool; 0x3000] {
    static BITS: OnceLock<[bool; 0x3000]> = OnceLock::new();
    BITS.get_or_init(|| letter_bits(PERSIAN_LETTERS))
}

fn punct_bits() -> &'static [bool; 0x3000] {
    static BITS: OnceLock<[bool; 0x3000]> = OnceLock::new();
    BITS.get_or_init(|| letter_bits(ALL_PUNCT))
}

fn in_bits(bits: &'static [bool; 0x3000], ch: char) -> bool {
    bits.get(ch as usize).copied().unwrap_or(false)
}

/// Whether `ch` is one of the punctuation marks the normalizer knows about.
#[inline]
pub(crate) fn is_punctuation(ch: char) -> bool {
    in_bits(punct_bits(), ch)
}

/// Whether `ch` is a Persian letter or the zero-width non-joiner.
#[inline]
pub(crate) fn is_persian_letter(ch: char) -> bool {
    ch == ZWNJ || in_bits(persian_letter_bits(), ch)
}

fn join_parts(left: &str, right: &str) -> String {
    format!("{left}{}{right}", preverb_joiner(left))
}

fn join_spans<F>(input: &str, mut try_join: F) -> Cow<'_, str>
where
    F: FnMut(&str, &str) -> Option<String>,
{
    let spans = word_spans(input);
    if spans.len() < 2 {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut prev_end = 0;
    let mut changed = false;
    let mut i = 0;

    while i < spans.len() {
        let (start, end) = spans[i];
        if i + 1 < spans.len() {
            let (next_start, next_end) = spans[i + 1];
            if !input[end..next_start].contains('\n') {
                if let Some(joined) = try_join(&input[start..end], &input[next_start..next_end]) {
                    out.push_str(&input[prev_end..start]);
                    out.push_str(&joined);
                    prev_end = next_end;
                    changed = true;
                    i += 2;
                    continue;
                }
            }
        }
        out.push_str(&input[prev_end..end]);
        prev_end = end;
        i += 1;
    }

    out.push_str(&input[prev_end..]);
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(input)
    }
}

fn try_join_words(prev: &str, next: &str, morph: &MorphologyEngine) -> Option<String> {
    let (next_core, next_punct) = split_trailing_punct(next);
    if !is_persian_word(next_core) || !is_persian_word(prev) {
        return None;
    }

    if PREFIXES.contains(&prev) {
        let candidate = join_parts(prev, next_core);
        if morph.is_known_noun(&candidate) {
            return Some(format!("{candidate}{next_punct}"));
        }
    }

    let candidate = join_parts(prev, next_core);
    if next_core == "تر" {
        return (morph.is_adjective(prev) && Inflector::comparative(prev) == candidate)
            .then(|| format!("{candidate}{next_punct}"));
    }
    if next_core == "ترین" {
        return (morph.is_adjective(prev) && Inflector::superlative(prev) == candidate)
            .then(|| format!("{candidate}{next_punct}"));
    }
    if next_core == "تری" {
        return (morph.is_adjective(prev)
            && format!("{}ی", Inflector::comparative(prev)) == candidate)
            .then(|| format!("{candidate}{next_punct}"));
    }

    if MORPH_SUFFIXES.contains(&next_core) {
        if morph.is_known_noun(prev) {
            return Some(format!("{candidate}{next_punct}"));
        }
    } else if word_suffixes().contains(next_core) && morph.is_known_noun(&candidate) {
        return Some(format!("{candidate}{next_punct}"));
    }

    None
}

fn word_suffixes() -> &'static HashSet<String> {
    WORD_SUFFIXES.get_or_init(|| {
        let mut set = HashSet::new();
        for suffix in WORD_SUFFIXES_BASE {
            set.insert((*suffix).to_string());
            if suffix.ends_with('ا') || suffix.ends_with('آ') {
                set.insert(format!("{suffix}یی"));
            } else if !suffix.ends_with('ی') {
                set.insert(format!("{suffix}ی"));
            }
        }
        for suffix in E_SUFFIXES {
            set.insert((*suffix).to_string());
        }
        set
    })
}

fn sorted_morph_suffixes() -> &'static [&'static str] {
    SORTED_MORPH_SUFFIXES.get_or_init(|| {
        let mut suffixes = MORPH_SUFFIXES.to_vec();
        suffixes.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
        suffixes
    })
}

fn join_attached_morph_suffixes<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let spans = word_spans(input);
    if spans.is_empty() {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut last = 0;
    let mut changed = false;

    for (start, end) in spans {
        let token = &input[start..end];
        let (core, punct) = split_trailing_punct(token);
        if core.contains(ZWNJ) || !is_persian_word(core) || morph.is_known_noun(core) {
            continue;
        }
        for suffix in sorted_morph_suffixes() {
            if core.len() <= suffix.len() || !core.ends_with(suffix) {
                continue;
            }
            let stem = &core[..core.len() - suffix.len()];
            if !is_persian_word(stem) || !morph.is_known_noun(stem) {
                continue;
            }
            // Only a `ه`-initial suffix or a comparative is unambiguous enough to
            // re-spell an unknown word: `معانات`/`معافات` end in `ات` with a known
            // noun stem and used to be split into `معان‌ات`/`معاف‌ات`.
            if !suffix.starts_with('ه') && !matches!(*suffix, "تر" | "تری" | "ترین") {
                continue;
            }
            if matches!(*suffix, "تر" | "تری" | "ترین") && !morph.is_adjective(stem) {
                continue;
            }
            let joined = join_parts(stem, suffix);
            out.push_str(&input[last..start]);
            out.push_str(&joined);
            out.push_str(punct);
            last = end;
            changed = true;
            break;
        }
    }

    if changed {
        out.push_str(&input[last..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(input)
    }
}

fn join_preverb_mi<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let spans = word_spans(input);
    if spans.len() < 2 {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut prev_end = 0;
    let mut changed = false;
    let mut i = 0;

    while i < spans.len() {
        let (start, end) = spans[i];
        let preverb = &input[start..end];

        if SHARED_VERBAL_PREFIXES.contains(&preverb) || matches!(preverb, "به‌در" | "دربر")
        {
            let joiner = preverb_joiner(preverb);

            if i + 2 < spans.len() {
                let (mi_start, mi_end) = spans[i + 1];
                let (stem_start, stem_end) = spans[i + 2];
                let mi = &input[mi_start..mi_end];
                let (stem_core, stem_punct) = split_trailing_punct(&input[stem_start..stem_end]);
                if (mi == "می" || mi == "نمی")
                    && !input[end..mi_start].contains('\n')
                    && !input[mi_end..stem_start].contains('\n')
                    && is_persian_word(stem_core)
                {
                    let form = format!("{preverb}{joiner}{mi}\u{200C}{stem_core}");
                    if is_verb_form(&form, morph) {
                        out.push_str(&input[prev_end..start]);
                        out.push_str(&form);
                        out.push_str(stem_punct);
                        prev_end = stem_end;
                        changed = true;
                        i += 3;
                        continue;
                    }
                }
            }

            if i + 1 < spans.len() {
                let (mi_start, mi_end) = spans[i + 1];
                let (next_core, next_punct) = split_trailing_punct(&input[mi_start..mi_end]);
                if !input[end..mi_start].contains('\n') {
                    if let Some(rest) = next_core
                        .strip_prefix("نمی\u{200C}")
                        .or_else(|| next_core.strip_prefix("می\u{200C}"))
                    {
                        if !rest.is_empty() && is_persian_word(rest) {
                            let form = format!("{preverb}{joiner}{next_core}");
                            if is_verb_form(&form, morph) {
                                out.push_str(&input[prev_end..start]);
                                out.push_str(&form);
                                out.push_str(next_punct);
                                prev_end = mi_end;
                                changed = true;
                                i += 2;
                                continue;
                            }
                        }
                    }
                }
            }
        }

        out.push_str(&input[prev_end..end]);
        prev_end = end;
        i += 1;
    }

    out.push_str(&input[prev_end..]);
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(input)
    }
}

fn preverb_joiner(preverb: &str) -> &'static str {
    if preverb
        .chars()
        .next_back()
        .is_some_and(|c| NON_LEFT_JOINERS.contains(c))
    {
        ""
    } else {
        "\u{200C}"
    }
}

fn join_mi_tokens<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let spans = word_spans(input);
    if spans.is_empty() {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut last = 0;
    let mut changed = false;

    for (start, end) in spans {
        let token = &input[start..end];
        let (core, punct) = split_trailing_punct(token);
        // `میانجی`/`میگساری` are nouns, not `می` + a verb: the whole token is
        // already known, so never respell it (`colloquial.rs` has the same guard).
        if morph.is_known_noun(core) {
            continue;
        }
        let (prefix, rest) = if let Some(rest) = core.strip_prefix("نمی") {
            ("نمی", rest)
        } else if let Some(rest) = core.strip_prefix("می") {
            ("می", rest)
        } else {
            continue;
        };
        if rest.is_empty() || rest.starts_with(ZWNJ) || !is_persian_word(rest) {
            continue;
        }
        let joined = format!("{prefix}\u{200C}{rest}");
        if is_verb_form(&joined, morph) {
            out.push_str(&input[last..start]);
            out.push_str(&joined);
            out.push_str(punct);
            last = end;
            changed = true;
        }
    }

    if changed {
        out.push_str(&input[last..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(input)
    }
}

fn is_verb_form(form: &str, morph: &MorphologyEngine) -> bool {
    if perfect_form_valid(form, morph) {
        return true;
    }

    for prefix in SHARED_VERBAL_PREFIXES
        .iter()
        .copied()
        .chain(["به‌در", "دربر"])
    {
        if let Some(rest) = form.strip_prefix(prefix) {
            let rest = rest.strip_prefix(ZWNJ).unwrap_or(rest);
            if rest.len() < form.len() && is_verb_form(rest, morph) {
                return true;
            }
        }
    }

    for prefix in ["نمی", "می"] {
        if let Some(rest) = form.strip_prefix(prefix) {
            let rest = rest.strip_prefix(ZWNJ).unwrap_or(rest);
            return mi_form_valid(rest, morph);
        }
    }

    conjugated_form_valid(form, morph)
}

fn mi_form_valid(form: &str, morph: &MorphologyEngine) -> bool {
    if morph.is_past_stem(form) {
        return true;
    }
    person_ending_form_valid(form, morph)
}

fn conjugated_form_valid(form: &str, morph: &MorphologyEngine) -> bool {
    if morph.is_known_verb_stem(form) {
        return true;
    }
    person_ending_form_valid(form, morph)
}

fn person_ending_form_valid(form: &str, morph: &MorphologyEngine) -> bool {
    VERB_PERSON_ENDINGS.iter().any(|ending| {
        form.strip_suffix(ending)
            .filter(|stem| !stem.is_empty())
            .is_some_and(|stem| morph.is_known_verb_stem(stem))
    })
}

fn perfect_form_valid(form: &str, morph: &MorphologyEngine) -> bool {
    PERFECT_SUFFIXES.iter().any(|suffix| {
        form.strip_suffix(suffix)
            .and_then(|stem| stem.strip_suffix('ه'))
            .is_some_and(|stem| morph.is_past_stem(stem))
    })
}

fn fix_ast_spacing(input: &str) -> Cow<'_, str> {
    let needle = "ه\u{200C}است";
    if !input.contains(needle) {
        return Cow::Borrowed(input);
    }

    let mut out: Option<String> = None;
    let mut last = 0;
    for (idx, _) in input.match_indices(needle) {
        let after = idx + needle.len();
        let boundary = after == input.len()
            || input[after..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || is_punctuation(c));
        if boundary {
            let b = out.get_or_insert_with(|| String::with_capacity(input.len() + 4));
            b.push_str(&input[last..idx]);
            b.push_str("ه است");
            last = after;
        }
    }

    match out {
        Some(mut b) => {
            b.push_str(&input[last..]);
            Cow::Owned(b)
        }
        None => Cow::Borrowed(input),
    }
}

const ALPHABET_GROUPS: &[(&str, &str)] = &[
    ("ﺁﺂ", "آ"),
    ("أٲٵﺃﺄ", "أ"),
    ("ﭐﭑٳﺇﺈإٱﺍﺎ", "ا"),
    ("ؠٮٻڀݐݒݔݕݖﭒﭕﺏﺒﺐﺑ", "ب"),
    ("ﭖﭗﭘﭙﭚﭛﭜﭝ", "پ"),
    ("ٹٺټٿݓﭞﭟﭠﭡﭦﭨﺕﺘﺖﺗ", "ت"),
    ("ٽݑﺙﺚﺛﺜﭢﭤ", "ث"),
    ("ڃڄﭲﭴﭵﭷﺝﺟﺠﺞ", "ج"),
    ("ڇڿﭺݘﭼﮀﮁݯﭻﭽ", "چ"),
    ("ځڂڅݗݮﺡﺤﺢﺣ", "ح"),
    ("ﺥﺦﺧﺨ", "خ"),
    ("ڈډڊڋڍۮݙݚﮂﮈﺩﺪ", "د"),
    ("ڌﱛﺫﺬڎڏڐﮅﮇ", "ذ"),
    ("ڑڒړڔڕږۯݛﮌﺭﺮ", "ر"),
    ("ڗݫﺯﺰ", "ز"),
    ("ڙﮊﮋ", "ژ"),
    ("ښڛﺱﺴﺲﺳ", "س"),
    ("ڜۺﺵﺸݜݭﺶﺷ", "ش"),
    ("ڝڞﺹﺼﺺﺻ", "ص"),
    ("ۻﺽﻀﺾﺿ", "ض"),
    ("ﻁﻃﻄﻂ", "ط"),
    ("ﻅﻆﻈڟﻇ", "ظ"),
    ("ڠݝݞݟﻉﻊﻋﻌ", "ع"),
    ("ۼﻍﻎﻐﻏ", "غ"),
    ("ڡڢڣڤڥڦݠݡﭪﭫﭬﻑﻒﻓﻔ", "ف"),
    ("ٯڧڨﻕﻗﻖﻘ", "ق"),
    ("كػؼڪګڬڭڮݢݣﮎﮐﯓﻙﻛﮏﮑ", "ک"),
    ("ڰڱڲڳڴﮒﮔﮖﮓﮕ", "گ"),
    ("ڵڶڷڸݪﻝﻠﻞﻟ", "ل"),
    ("۾ݥݦﻡﻢﻣﻤ", "م"),
    ("ڹںڻڼڽݧݨݩﮞﻥﻧﻦﻨ", "ن"),
    ("ﯝٷﯗﯘﺅٶﺆ", "ؤ"),
    ("ﯙﯚﯜﯞﯟۄۅۉۊۋۏﯠﻭﻮ", "و"),
    ("ﮤۂﮥ", "ۀ"),
    ("ھۿہۃەﮦﮧﮨﮩﻩﻫةﻪﻬ", "ه"),
    ("ﮰﮱٸۓﺉﺊﺋﺌ", "ئ"),
    ("ﯷﯹ", "ئی"),
    ("ﯻ", "ئد"),
    ("ﯫ", "ئا"),
    ("ﯭ", "ئه"),
    ("ﯰﯵﯳ", "ئو"),
    ("ؽؾؿىيۍێېۑےﮮﮯﯤﯥﯦﯧﯼﯽﯾﯿﻯﻱﻳﯨﯩﱝ", "ی"),
];

const ARABIC_UNICODE_GROUPS: &[(&str, &str)] = &[
    ("﷽", "بسم الله الرحمن الرحیم"),
    ("﷼", "ریال"),
    ("ﷰﷹ", "صلی"),
    ("ﷲ", "الله"),
    ("ﷳ", "اکبر"),
    ("ﷴ", "محمد"),
    ("ﷵ", "صلعم"),
    ("ﷶ", "رسول"),
    ("ﷷ", "علیه"),
    ("ﷸ", "وسلم"),
    ("ﻵﻶﻷﻸﻹﻺﻻﻼ", "لا"),
];

const DIGIT_GROUPS: &[(&str, &str)] = &[
    ("0٠𝟢𝟬", "۰"),
    ("1١𝟣𝟭⑴⒈⓵①❶𝟙𝟷", "۱"),
    ("2٢𝟤𝟮⑵⒉⓶②❷²𝟐𝟸𝟚", "۲"),
    ("3٣𝟥𝟯⑶⒊⓷③❸³", "۳"),
    ("4٤𝟦𝟰⑷⒋⓸④❹⁴", "۴"),
    ("5٥𝟧𝟱⑸⒌⓹⑤❺⁵", "۵"),
    ("6٦𝟨𝟲⑹⒍⓺⑥❻⁶", "۶"),
    ("7٧𝟩𝟳⑺⒎⓻⑦❼⁷", "۷"),
    ("8٨𝟪𝟴⑻⒏⓼⑧❽⁸۸", "۸"),
    ("9٩𝟫𝟵⑼⒐⓽⑨❾⁹", "۹"),
    ("⑽⒑⓾⑩", "۱۰"),
    ("⑾⒒⑪", "۱۱"),
    ("⑿⒓⑫", "۱۲"),
    ("⒀⒔⑬", "۱۳"),
    ("⒁⒕⑭", "۱۴"),
    ("⒂⒖⑮", "۱۵"),
    ("⒃⒗⑯", "۱۶"),
    ("⒄⒘⑰", "۱۷"),
    ("⒅⒙⑱", "۱۸"),
    ("⒆⒚⑲", "۱۹"),
    ("⒇⒛⑳", "۲۰"),
];

const PUNCTUATION_GROUPS: &[(&str, &str)] = &[
    ("▕❘❙❚▏│", "|"),
    // Em/en dashes and the combining strike only: `ㅡ` (Hangul), `一` (CJK) and
    // `ー` (katakana) are letters of other scripts, not Persian punctuation.
    ("—–̶", "-"),
    ("▁_̲", "_"),
    ("❔?❓؟", "؟"),
    ("❕！", "!"),
    ("⁉", "!؟"),
    ("‼", "!!"),
    ("℅%", "٪"),
    ("÷", "/"),
    ("×", "*"),
    ("：", ":"),
    ("›", ">"),
    ("‹＜", "<"),
    ("《", "«"),
    ("》", "»"),
    ("•", "."),
    ("٬,", "،"),
    (";；", "؛"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_normalizes_presentation_forms() {
        assert_eq!(
            alphabet("ۿدف ما ػمګ بۃ ێڪډيڱڕ إښټ"),
            "هدف ما کمک به یکدیگر است"
        );
    }

    #[test]
    fn arabic_unicode_expands_ligatures() {
        assert_eq!(arabic_unicode("﷽ ﷼ ﷴ"), "بسم الله الرحمن الرحیم ریال محمد");
    }

    #[test]
    fn digits_normalizes_all_digit_sets() {
        assert_eq!(digits("1𝟮3٤٥⓺"), "۱۲۳۴۵۶");
    }

    #[test]
    fn punctuation_maps_symbols() {
        assert_eq!(punctuation("چیست?"), "چیست؟");
        assert_eq!(punctuation("⁉"), "!؟");
        // `❓` is the black partner of `❔`; its table slot held U+FFFD, so it
        // passed through while every other question ornament was rewritten.
        assert_eq!(punctuation("❓"), "؟");
        assert_eq!(punctuation("❔❓?"), "؟؟؟");
        // A replacement character is not punctuation anyone typed on purpose.
        assert_eq!(punctuation("\u{FFFD}"), "\u{FFFD}");
    }

    #[test]
    fn tables_leave_letters_and_marks_alone() {
        // A skin tone modifier, a pharyngeal and a glottal stop are not
        // question marks; dotless i, Armenian sheh and Georgian vini are not
        // digits. Each used to be rewritten into one.
        for ch in [
            '\u{0295}',
            '\u{0294}',
            '\u{1F3FB}',
            '\u{0008}',
            '\u{0097}',
            '\u{009D}',
        ] {
            assert_eq!(punctuation(&ch.to_string()), ch.to_string(), "{ch:?}");
        }
        for ch in ['\u{0131}', '\u{0577}', '\u{10D5}'] {
            assert_eq!(digits(&ch.to_string()), ch.to_string(), "{ch:?}");
        }
        // The real mappings still happen.
        assert_eq!(punctuation("👍🏻؟"), "👍🏻؟");
        assert_eq!(digits("1١①"), "۱۱۱");
    }

    #[test]
    fn repeated_letters_leaves_punctuation_and_digits() {
        // `...` is an ellipsis and `!!!` is emphasis, not a repeated letter.
        assert_eq!(repeated_letters("سلام..."), "سلام...");
        assert_eq!(repeated_letters("!!!"), "!!!");
        assert_eq!(repeated_letters("۱۲۳۳۳"), "۱۲۳۳۳");
        assert_eq!(repeated_letters("خوببب"), "خوبب");
    }

    #[test]
    fn orthographic_spacing_keeps_compound_zwnj() {
        // A ZWNJ after a non-joining letter is the only separator in a compound;
        // deleting it turned shipped words into spellings the lexicon lacks.
        assert_eq!(orthographic_spacing("آذر\u{200C}ماه"), "آذر\u{200C}ماه");
        assert_eq!(orthographic_spacing("نماز\u{200C}جمعه"), "نماز\u{200C}جمعه");
        assert_eq!(
            orthographic_spacing("آن\u{200C}ها\u{200C}است"),
            "آن\u{200C}ها\u{200C}است"
        );
    }

    #[test]
    fn other_scripts_are_not_rewritten() {
        // Hebrew pe, Canadian syllabics, Hangul and CJK lookalikes used to be
        // mapped to Persian letters, digits and punctuation.
        assert_eq!(alphabet("\u{05E4}"), "\u{05E4}");
        assert_eq!(digits("\u{14BF}"), "\u{14BF}");
        for ch in ['\u{3161}', '\u{4E00}', '\u{30FC}'] {
            assert_eq!(punctuation(&ch.to_string()), ch.to_string(), "{ch:?}");
        }
        assert_eq!(punctuation("—"), "-");
    }

    #[test]
    fn bidi_controls_are_invisible() {
        let controls = "\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}\u{2066}\u{2067}\u{2068}\u{2069}";
        assert_eq!(orthographic_spacing(controls), "");
    }

    #[test]
    fn spacing_does_not_respell_nouns_or_split_unknown_words() {
        let morph = MorphologyEngine::new();
        // `میانجی`/`میگساری` are nouns, not `می` + a verb.
        assert_eq!(verbal_spacing("میانجی", &morph), "میانجی");
        assert_eq!(verbal_spacing("میگساری", &morph), "میگساری");
        // `ات` inside an unknown word is not an attachable plural.
        assert_eq!(word_spacing("معانات.", &morph), "معانات.");
        assert_eq!(word_spacing("مشتها.", &morph), "مشت\u{200C}ها.");
    }

    #[test]
    fn spacing_is_idempotent_and_does_not_split_a_bracket_pair() {
        // `خوب؟‌ها` gained a space between `؟` and the ZWNJ on the first pass and
        // lost the ZWNJ on the second, so the stage was not idempotent.
        let once = punctuation_spacing("خوب؟\u{200C}ها");
        assert_eq!(once, "خوب؟\u{200C}ها");
        assert_eq!(punctuation_spacing(once.as_ref()), once.as_ref());
        // A run of openers opens once: `((ب))` was `( (ب))`.
        assert_eq!(punctuation_spacing("((ب))"), "((ب))");
        assert_eq!(punctuation_spacing("سلام(ب)"), "سلام (ب)");
    }

    #[test]
    fn repeated_letters_does_not_trim_the_edges() {
        // Collapsing a run is this stage's job; trimming leading/trailing space is
        // not, and it silently changed text the caller passed through untouched.
        assert_eq!(repeated_letters("  سسسلام  "), "  سسلام  ");
        assert_eq!(repeated_letters("سلامممم"), "سلامم");
        assert_eq!(repeated_letters("سلام"), "سلام");
    }

    #[test]
    fn repeated_letters_collapses_runs() {
        assert_eq!(
            repeated_letters("اینــــجاااا یکــــــ متنــــــ نمونه است."),
            "اینجاا یک متن نمونه است."
        );
    }

    #[test]
    fn ya_styles_round_trip() {
        assert_eq!(ya("خانه‌ی ما", YaStyle::Standard), "خانۀ ما");
        assert_eq!(ya("خانۀ ما", YaStyle::Joda), "خانه‌ی ما");
        // `ه‌ی` is the ending only at a word end: `خانه‌یی` must not become
        // `خانۀی`, which is neither spelling.
        assert_eq!(ya("خانه‌یی", YaStyle::Standard), "خانه‌یی");
        assert_eq!(ya("خانه‌ی", YaStyle::Standard), "خانۀ");
        // Punctuation ends a word as well as a space, so the ending is converted
        // before a full stop too; it used to stay خانه‌ی. while خانه‌ی ما
        // became خانۀ ما.
        assert_eq!(ya("خانه‌ی.", YaStyle::Standard), "خانۀ.");
        assert_eq!(ya("خانه‌ی،", YaStyle::Standard), "خانۀ،");
        assert_eq!(ya("خانۀ.", YaStyle::Joda), "خانه‌ی.");
    }

    #[test]
    fn orthographic_spacing_cleans_invisibles_and_zwnj() {
        assert_eq!(
            orthographic_spacing("\u{200F}سلام\u{200B}  دنیا\u{200C}  خوب\u{200F}"),
            "سلام دنیا خوب"
        );
        assert_eq!(orthographic_spacing("خانهها"), "خانه‌ها");
    }

    #[test]
    fn letter_and_punctuation_bits_match_the_strings() {
        // The bitmaps are built from the constants, so this pins the half-open
        // bound: a character past the last slot must read as false, not panic.
        for ch in PERSIAN_LETTERS.chars() {
            assert!(is_persian_letter(ch), "{ch:?} missing from the bitmap");
        }
        for ch in ALL_PUNCT.chars() {
            assert!(is_punctuation(ch), "{ch:?} missing from the bitmap");
        }
        assert!(is_persian_letter(ZWNJ));
        assert!(!is_persian_letter('x'));
        assert!(!is_punctuation('\u{10FFFF}'));
    }

    #[test]
    fn orthographic_spacing_borrows_clean_text() {
        // The point of the Cow: text that needs no fix must not be copied.
        assert!(matches!(orthographic_spacing("سلام دنیا"), Cow::Borrowed(_)));
        assert!(matches!(orthographic_spacing("سلام  دنیا"), Cow::Owned(_)));
        assert!(matches!(orthographic_spacing(" سلام"), Cow::Owned(_)));
        let morph = MorphologyEngine::new();
        assert!(matches!(spacing("سلام دنیا", &morph), Cow::Borrowed(_)));
    }

    #[test]
    fn punctuation_spacing_fixes_boundaries() {
        assert_eq!(punctuation_spacing("سلام(دنیا)"), "سلام (دنیا)");
        assert_eq!(punctuation_spacing("سلام ، دنیا ."), "سلام، دنیا.");
        assert_eq!(punctuation_spacing("است!نوشته"), "است! نوشته");
        // Punctuation at the very start of the text separated from the word after
        // it too; it used to be exempt, so ، دنیا came back as ،دنیا.
        assert_eq!(punctuation_spacing("، دنیا"), "، دنیا");
        assert_eq!(punctuation_spacing("… و بعد"), "… و بعد");
    }

    #[test]
    fn punctuation_spacing_does_not_split_numbers_or_urls() {
        // A `.`/`:` between digits, or a URL, is not a word boundary. Splitting
        // these turned `3.14` into `3. 14` and a real link into
        // `https: //rishe. example/docs`.
        for text in [
            "3.14",
            "12:30",
            "v1.0.4",
            "۱۴.۵",
            "https://rishe.example/docs",
            "http://a.b/c?d=1",
            "www.example.com",
        ] {
            assert_eq!(punctuation_spacing(text), text, "{text:?} was split");
        }
        // A full stop after a number is still a sentence boundary.
        assert_eq!(punctuation_spacing("۱۴۰۰.سال"), "۱۴۰۰. سال");
        assert_eq!(punctuation_spacing("عدد ۳:نوشته"), "عدد ۳: نوشته");
    }

    #[test]
    fn orthographic_spacing_keeps_a_carriage_return_as_a_line_break() {
        // Dropping `\r` joined the lines: CRLF files and old-Mac files collapsed
        // into one long line.
        assert_eq!(orthographic_spacing("خط اول\rخط دوم"), "خط اول\nخط دوم");
        assert_eq!(orthographic_spacing("خط اول\r\nخط دوم"), "خط اول\nخط دوم");
        assert_eq!(orthographic_spacing("a\r\rb"), "a\n\nb");
        // A lone CR becomes a newline, which the trailing-whitespace trim then
        // removes — the same as a lone `\n` today.
        assert_eq!(orthographic_spacing("\r"), "");
        assert_eq!(orthographic_spacing("\n"), "");
        // A lone `\n` is untouched, and CRLF drops the CR instead of adding a break.
        assert_eq!(orthographic_spacing("a\nb"), "a\nb");
        assert_eq!(orthographic_spacing("a\r\n\r\nb"), "a\n\nb");
    }

    #[test]
    fn word_spacing_joins_known_forms() {
        let morph = MorphologyEngine::new();
        assert_eq!(word_spacing("بی سواد", &morph), "بی‌سواد");
        assert_eq!(word_spacing("کتاب ها", &morph), "کتاب‌ها");
        assert_eq!(word_spacing("خانه ام", &morph), "خانه‌ام");
        assert_eq!(word_spacing("بزرگ تر", &morph), "بزرگ‌تر");
        assert_eq!(word_spacing("کتاب تر", &morph), "کتاب تر");
    }

    #[test]
    fn verbal_spacing_joins_verb_forms() {
        let morph = MorphologyEngine::new();
        assert_eq!(verbal_spacing("می روم", &morph), "می‌روم");
        assert_eq!(verbal_spacing("میروم", &morph), "می‌روم");
        assert_eq!(verbal_spacing("می رفت", &morph), "می‌رفت");
        assert_eq!(verbal_spacing("میجست", &morph), "می‌جست");
        assert_eq!(verbal_spacing("رفته ام", &morph), "رفته‌ام");
        assert_eq!(verbal_spacing("بر می دارم", &morph), "برمی‌دارم");
        assert_eq!(verbal_spacing("پس می‌افتیم", &morph), "پس‌می‌افتیم");
        assert_eq!(verbal_spacing("پس می‌افتیم.", &morph), "پس‌می‌افتیم.");
        assert_eq!(verbal_spacing("می روم.", &morph), "می‌روم.");
        assert_eq!(verbal_spacing("رفته‌است", &morph), "رفته است");
        assert_eq!(verbal_spacing("می است", &morph), "می است");
    }

    #[test]
    fn spacing_composes_operators() {
        let morph = MorphologyEngine::new();
        assert_eq!(spacing("سلام(دنیا) ! می روم", &morph), "سلام (دنیا)! می‌روم");
    }

    #[test]
    fn op_names_resolve() {
        assert_eq!(NormalizeOp::from_name("digits"), Some(NormalizeOp::Digits));
        assert_eq!(
            NormalizeOp::from_name("ya_joda"),
            Some(NormalizeOp::Ya(YaStyle::Joda))
        );
        assert_eq!(NormalizeOp::from_name("nope"), None);
    }
}
