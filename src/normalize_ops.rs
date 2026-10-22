use crate::inflector::Inflector;
use crate::morphology::MorphologyEngine;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const ZWNJ: char = '\u{200C}';

pub(crate) const PERSIAN_LETTERS: &str = "آابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهیءؤۀأئ";
const NON_LEFT_JOINERS: &str = "دۀذاأآورژز";
const SINGLE_PUNCT: &str = "…،:.؟!؛";
const OPENER_PUNCT: &str = ">{[(«";
const CLOSER_PUNCT: &str = "<}])»";
pub(crate) const ALL_PUNCT: &str = "…،:.؟!؛>{[(«<}])»-٪@/#";
const INVISIBLE: &str =
    "\u{200B}\u{200D}\u{200E}\u{200F}\u{2066}\u{2067}\u{202A}\u{202B}\u{202D}\u{FEFF}";

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

const VERBAL_PREFIXES: &[&str] = &[
    "بر",
    "باز",
    "در",
    "فرو",
    "وا",
    "ور",
    "فرا",
    "به‌در",
    "دربر",
    "پس",
    "پیش",
];

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
    "نشین",
    "گزین",
    "ساز",
    "کنان",
    "دار",
    "بند",
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
    "ساز",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YaStyle {
    Standard,
    Joda,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizeOp {
    Alphabet,
    ArabicUnicode,
    Ya(YaStyle),
    Digits,
    Punctuation,
    RepeatedLetters,
    Spacing,
    OrthographicSpacing,
    PunctuationSpacing,
    WordSpacing,
    VerbalSpacing,
}

impl NormalizeOp {
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

static ALPHABET_TABLE: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
static ARABIC_UNICODE_TABLE: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
static DIGIT_TABLE: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
static PUNCTUATION_TABLE: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
static WORD_SUFFIXES: OnceLock<HashSet<String>> = OnceLock::new();
static SORTED_MORPH_SUFFIXES: OnceLock<Vec<&'static str>> = OnceLock::new();

pub fn alphabet(input: &str) -> Cow<'_, str> {
    mapped(input, ALPHABET_TABLE.get_or_init(|| table(ALPHABET_GROUPS)))
}

pub fn arabic_unicode(input: &str) -> Cow<'_, str> {
    mapped(
        input,
        ARABIC_UNICODE_TABLE.get_or_init(|| table(ARABIC_UNICODE_GROUPS)),
    )
}

pub fn digits(input: &str) -> Cow<'_, str> {
    mapped(input, DIGIT_TABLE.get_or_init(|| table(DIGIT_GROUPS)))
}

pub fn punctuation(input: &str) -> Cow<'_, str> {
    mapped(
        input,
        PUNCTUATION_TABLE.get_or_init(|| table(PUNCTUATION_GROUPS)),
    )
}

pub fn ya(input: &str, style: YaStyle) -> Cow<'_, str> {
    match style {
        YaStyle::Standard => {
            if input.contains("ه\u{200C}ی") {
                let replaced = input.replace("ه\u{200C}ی", "ۀ");
                Cow::Owned(ya_ha_yeh_space(&replaced, "ۀ").into_owned())
            } else {
                ya_ha_yeh_space(input, "ۀ")
            }
        }
        YaStyle::Joda => {
            if input.contains('ۀ') {
                let replaced = input.replace('ۀ', "ه\u{200C}ی");
                Cow::Owned(ya_ha_yeh_space(&replaced, "ه\u{200C}ی").into_owned())
            } else {
                ya_ha_yeh_space(input, "ه\u{200C}ی")
            }
        }
    }
}

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
        if run > 2 && !ch.is_whitespace() && !ch.is_numeric() {
            changed = true;
            continue;
        }
        out.push(ch);
    }

    let trimmed = out.trim();
    if !changed && trimmed.len() == input.len() {
        return Cow::Borrowed(input);
    }
    Cow::Owned(trimmed.to_string())
}

pub fn orthographic_spacing(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    let mut laid_out = String::with_capacity(input.len());
    let mut last_horizontal = false;
    for ch in input.chars() {
        if INVISIBLE.contains(ch) || ch == '\r' {
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

    let with_ha = laid_out.replace("هها", "ه\u{200C}ها");
    let cleaned_zwnj = clean_zwnj(&with_ha);
    let collapsed = collapse_horizontal(&cleaned_zwnj);
    Cow::Owned(collapsed.trim().to_string())
}

pub fn punctuation_spacing(input: &str) -> Cow<'_, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    let chars: Vec<char> = input.chars().collect();
    let chars = strip_space_after_openers(chars);
    let chars = strip_space_before_closers(chars);
    let chars = insert_space_before_openers(chars);
    let chars = insert_space_after_singles(chars);
    Cow::Owned(finalize_spacing(chars))
}

pub fn word_spacing<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let joined = join_spans(input, |prev, next| try_join_words(prev, next, morph));
    let with_suffixes = join_attached_morph_suffixes(&joined, morph);
    match with_suffixes {
        Cow::Borrowed(_) if matches!(joined, Cow::Borrowed(_)) => Cow::Borrowed(input),
        other => Cow::Owned(other.into_owned()),
    }
}

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

pub fn spacing<'a>(input: &'a str, morph: &MorphologyEngine) -> Cow<'a, str> {
    let s = orthographic_spacing(input);
    let s = punctuation_spacing(&s);
    let s = word_spacing(&s, morph);
    let s = verbal_spacing(&s, morph);
    Cow::Owned(s.into_owned())
}

fn mapped<'a>(input: &'a str, table: &HashMap<char, &'static str>) -> Cow<'a, str> {
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

fn table(groups: &'static [(&'static str, &'static str)]) -> HashMap<char, &'static str> {
    let mut map = HashMap::with_capacity(groups.len() * 4);
    for (chars, replacement) in groups {
        for ch in chars.chars() {
            map.insert(ch, *replacement);
        }
    }
    map
}

fn ya_ha_yeh_space<'a>(input: &'a str, replacement: &str) -> Cow<'a, str> {
    let needle = "ه ی";
    let mut out: Option<String> = None;
    let mut last = 0;

    for (idx, _) in input.match_indices(needle) {
        let after = idx + needle.len();
        let boundary = after == input.len()
            || input[after..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace);
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
        let after_non_joiner = prev.is_some_and(|c| NON_LEFT_JOINERS.contains(c));
        let near_space = prev == Some(' ') || next == Some(' ');
        if between_script && !after_non_joiner && !near_space {
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

fn collapse_horizontal(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_space = false;
    for ch in input.chars() {
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
        if OPENER_PUNCT.contains(ch) && out.last().is_some_and(|c: &char| !c.is_whitespace()) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn insert_space_after_singles(chars: Vec<char>) -> Vec<char> {
    let mut out = Vec::with_capacity(chars.len() + 8);
    for (i, ch) in chars.iter().copied().enumerate() {
        out.push(ch);
        let single_at_start = i == 0 && SINGLE_PUNCT.contains(ch);
        if !single_at_start && (SINGLE_PUNCT.contains(ch) || CLOSER_PUNCT.contains(ch)) {
            if let Some(&next) = chars.get(i + 1) {
                if !next.is_whitespace()
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
    let mut out = String::with_capacity(chars.len());
    let mut in_space = false;
    for ch in chars {
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
    out.trim_matches(is_horizontal).to_string()
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
        if ALL_PUNCT.contains(ch) {
            end = i;
        } else {
            break;
        }
    }
    (&token[..end], &token[end..])
}

fn is_persian_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| PERSIAN_LETTERS.contains(c) || c == ZWNJ)
}

fn join_parts(left: &str, right: &str) -> String {
    if left
        .chars()
        .next_back()
        .is_some_and(|c| NON_LEFT_JOINERS.contains(c))
    {
        format!("{left}{right}")
    } else {
        format!("{left}{ZWNJ}{right}")
    }
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

        if VERBAL_PREFIXES.contains(&preverb) {
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

    for prefix in VERBAL_PREFIXES {
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
    VERB_PERSON_ENDINGS.iter().any(|ending| {
        form.strip_suffix(ending)
            .filter(|stem| !stem.is_empty())
            .is_some_and(|stem| morph.is_known_verb_stem(stem))
    })
}

fn conjugated_form_valid(form: &str, morph: &MorphologyEngine) -> bool {
    if morph.is_known_verb_stem(form) {
        return true;
    }
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
                .is_some_and(|c| c.is_whitespace() || ALL_PUNCT.contains(c));
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
    ("أٲٵ", "أ"),
    ("ﭐﭑٳﺇﺈإٱ", "ا"),
    ("ؠٮٻڀݐݒݔݕݖﭒﭕﺏﺒ", "ب"),
    ("ﭖﭗﭘﭙﭚﭛﭜﭝ", "پ"),
    ("ٹٺټٿݓﭞﭟﭠﭡﭦﭨﺕﺘ", "ت"),
    ("ٽݑﺙﺚﺛﺜﭢﭤ", "ث"),
    ("ڃڄﭲﭴﭵﭷﺝﺟﺠ", "ج"),
    ("ڇڿﭺݘﭼﮀﮁݯ", "چ"),
    ("ځڂڅݗݮﺡﺤ", "ح"),
    ("ﺥﺦﺧ", "خ"),
    ("ڈډڊڋڍۮݙݚﮂﮈﺩ", "د"),
    ("ڌﱛﺫﺬڎڏڐﮅﮇ", "ذ"),
    ("ڑڒړڔڕږۯݛﮌﺭ", "ر"),
    ("ڗݫﺯﺰ", "ز"),
    ("ڙﮊﮋ", "ژ"),
    ("ښڛﺱﺴ", "س"),
    ("ڜۺﺵﺸݜݭ", "ش"),
    ("ڝڞﺹﺼ", "ص"),
    ("ۻﺽﻀ", "ض"),
    ("ﻁﻃﻄ", "ط"),
    ("ﻅﻆﻈڟ", "ظ"),
    ("ڠݝݞݟﻉﻊﻋ", "ع"),
    ("ۼﻍﻎﻐ", "غ"),
    ("ڡڢڣڤڥڦݠݡﭪﭫﭬﻑﻒﻓ", "ف"),
    ("ٯڧڨﻕﻗ", "ق"),
    ("كػؼڪګڬڭڮݢݣﮎﮐﯓﻙﻛ", "ک"),
    ("ڰڱڲڳڴﮒﮔﮖ", "گ"),
    ("ڵڶڷڸݪﻝﻠ", "ل"),
    ("۾ݥݦﻡﻢﻣ", "م"),
    ("ڹںڻڼڽݧݨݩﮞﻥﻧ", "ن"),
    ("ﯝٷﯗﯘﺅٶ", "ؤ"),
    ("ﯙﯚﯜﯞﯟۄۅۉۊۋۏﯠﻭפ", "و"),
    ("ﮤۂ", "ۀ"),
    ("ھۿہۃەﮦﮧﮨﮩﻩﻫة", "ه"),
    ("ﮰﮱٸۓ", "ئ"),
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
    ("1١𝟣𝟭⑴⒈⓵①❶𝟙𝟷ı", "۱"),
    ("2٢𝟤𝟮⑵⒉⓶②❷²𝟐𝟸𝟚ᒿշ", "۲"),
    ("3٣𝟥𝟯⑶⒊⓷③❸³ვ", "۳"),
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
    ("ㅡ一—–ー̶", "-"),
    ("▁_̲", "_"),
    ("❔?�؟ʕʔ🏻\u{0008}\u{0097}\u{009D}", "؟"),
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
    fn punctuation_spacing_fixes_boundaries() {
        assert_eq!(punctuation_spacing("سلام(دنیا)"), "سلام (دنیا)");
        assert_eq!(punctuation_spacing("سلام ، دنیا ."), "سلام، دنیا.");
        assert_eq!(punctuation_spacing("است!نوشته"), "است! نوشته");
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
