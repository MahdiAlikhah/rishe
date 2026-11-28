use crate::normalize_ops::is_persian_letter;
use crate::transliterate::persian_digit;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use std::sync::OnceLock;

const UNITS: [&str; 20] = [
    "صفر",
    "یک",
    "دو",
    "سه",
    "چهار",
    "پنج",
    "شش",
    "هفت",
    "هشت",
    "نه",
    "ده",
    "یازده",
    "دوازده",
    "سیزده",
    "چهارده",
    "پانزده",
    "شانزده",
    "هفده",
    "هجده",
    "نوزده",
];
const TENS: [&str; 10] = [
    "",
    "",
    "بیست",
    "سی",
    "چهل",
    "پنجاه",
    "شصت",
    "هفتاد",
    "هشتاد",
    "نود",
];
const HUNDREDS: [&str; 10] = [
    "",
    "صد",
    "دویست",
    "سیصد",
    "چهارصد",
    "پانصد",
    "ششصد",
    "هفتصد",
    "هشتصد",
    "نهصد",
];
const SCALES: [&str; 7] = [
    "",
    "هزار",
    "میلیون",
    "میلیارد",
    "تریلیون",
    "کوادریلیون",
    "کوینتیلیون",
];

/// Render an integer in Persian words, prefixing `منفی` for negative values.
pub fn number_to_words(number: i64) -> String {
    if number < 0 {
        return format!("منفی {}", unsigned_to_words(number.unsigned_abs()));
    }
    unsigned_to_words(number as u64)
}

/// Render a Persian or ASCII digit string in Persian words, using `ممیز` for
/// decimals; errors on malformed or out-of-range input.
pub fn number_to_words_text(text: &str) -> Result<String, String> {
    // Same separators the tokenizer accepts: `٫` is the decimal point and `٬`
    // groups thousands. Left alone they made `۱٫۵` unparseable.
    let normalized: String = text
        .trim()
        .chars()
        .filter_map(|ch| match ch {
            '\u{066B}' => Some('.'),
            '\u{066C}' | ',' => None,
            _ => persian_digit(ch).or(Some(ch)),
        })
        .collect();

    let (negative, body) = match normalized.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, normalized.as_str()),
    };
    if body.is_empty() {
        return Err(format!("invalid numeric input: {text}"));
    }

    let mut parts = body.splitn(2, '.');
    let integer = parts.next().unwrap();
    let decimal = parts.next();

    if integer.is_empty() || !integer.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("invalid numeric input: {text}"));
    }
    let integer_value: u64 = integer
        .parse()
        .map_err(|_| format!("number out of range: {text}"))?;

    let mut words = unsigned_to_words(integer_value);
    if let Some(decimal) = decimal {
        if decimal.is_empty() || !decimal.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(format!("invalid numeric input: {text}"));
        }
        // Leading zeros are words of their own: `3.05` is "سه ممیز صفر پنج",
        // not "سه ممیز پنج". Parsing the stripped part also keeps a run of
        // zeros from overflowing the value.
        let stripped = decimal.trim_start_matches('0');
        let mut decimal_words: Vec<String> =
            vec![UNITS[0].to_string(); decimal.len() - stripped.len()];
        if !stripped.is_empty() {
            let decimal_value: u64 = stripped
                .parse()
                .map_err(|_| format!("number out of range: {text}"))?;
            decimal_words.push(unsigned_to_words(decimal_value));
        }
        words = format!("{words} ممیز {}", decimal_words.join(" "));
    }
    if negative {
        words = format!("منفی {words}");
    }
    Ok(words)
}

fn unsigned_to_words(mut number: u64) -> String {
    if number == 0 {
        return UNITS[0].to_string();
    }

    let mut chunks = Vec::new();
    let mut scale = 0;
    while number > 0 {
        let chunk = (number % 1000) as u32;
        if chunk > 0 {
            let words = chunk_to_words(chunk);
            if SCALES[scale].is_empty() {
                chunks.push(words);
            } else {
                chunks.push(format!("{words} {}", SCALES[scale]));
            }
        }
        number /= 1000;
        scale += 1;
    }

    chunks.reverse();
    chunks.join(" و ")
}

fn chunk_to_words(number: u32) -> String {
    let mut parts = Vec::new();
    let hundreds = (number / 100) as usize;
    let remainder = (number % 100) as usize;

    if hundreds > 0 {
        parts.push(HUNDREDS[hundreds].to_string());
    }
    if remainder > 0 {
        if remainder < 20 {
            parts.push(UNITS[remainder].to_string());
        } else {
            parts.push(TENS[remainder / 10].to_string());
            if !remainder.is_multiple_of(10) {
                parts.push(UNITS[remainder % 10].to_string());
            }
        }
    }

    parts.join(" و ")
}

const LOANWORDS: &[(&str, &[&str])] = &[
    ("کامپیوتر", &["رایانه"]),
    ("لپ‌تاپ", &["رایانهٔ همراه"]),
    ("موبایل", &["تلفن همراه"]),
    ("ایمیل", &["رایانامه"]),
    ("سایت", &["تارنما"]),
    ("وب‌سایت", &["تارنما"]),
    ("وب سایت", &["تارنما"]),
    ("دانلود", &["بارگیری"]),
    ("آپلود", &["بارگذاری"]),
    ("فایل", &["پرونده"]),
    ("دیتا", &["داده"]),
    ("دیتابیس", &["پایگاه داده"]),
    ("سرور", &["کارساز"]),
    ("آنلاین", &["برخط"]),
    ("آفلاین", &["برون‌خط"]),
    ("سیستم", &["سامانه"]),
    ("پروژه", &["طرح"]),
    ("پروسه", &["فرایند"]),
    ("استراتژی", &["راهبرد"]),
    ("تکنولوژی", &["فناوری"]),
    ("تکنیک", &["فن"]),
    ("پروتکل", &["قرارداد"]),
    ("مانیتور", &["نمایشگر"]),
    ("کیبورد", &["صفحه‌کلید"]),
    ("پرینتر", &["چاپگر"]),
    ("اسکنر", &["پویشگر"]),
    ("پرینت", &["چاپ"]),
    ("اسکن", &["پویش"]),
    ("کپی", &["رونوشت"]),
    ("فکس", &["دورنگار"]),
    ("پروفایل", &["نمایه"]),
    ("اکانت", &["حساب"]),
    ("لاگین", &["ورود"]),
    ("پسورد", &["گذرواژه"]),
    ("یوزرنیم", &["نام کاربری"]),
    ("آپدیت", &["به‌روزرسانی"]),
    ("کنفرانس", &["همایش"]),
    ("سمینار", &["همایش"]),
    ("میتینگ", &["جلسه"]),
    ("تیم", &["گروه"]),
    ("کمپین", &["کارزار"]),
    ("برند", &["نشان تجاری"]),
    ("مارکتینگ", &["بازاریابی"]),
    ("بیزینس", &["کسب‌وکار"]),
    ("استارتاپ", &["کسب‌وکار نوپا"]),
    ("فریلنسر", &["آزادکار"]),
    ("ایده", &["اندیشه"]),
    ("ایدئولوژی", &["اندیشه‌شناسی"]),
    ("استادیوم", &["ورزشگاه"]),
    ("اورژانس", &["فوریت"]),
    ("هتل", &["مهمان‌سرا"]),
    ("پارکینگ", &["جایگاه خودرو"]),
    ("دربی", &["شهرآورد"]),
    ("فینال", &["پایانی"]),
    ("کاپ", &["جام"]),
];

/// A loanword found by [`persianize_suggest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanwordMatch {
    /// The matched loanword.
    pub word: String,
    /// Preferred Persian replacements, best first.
    pub alternatives: Vec<String>,
    /// Character offset of the match start.
    pub start: usize,
    /// Character offset one past the match end.
    pub end: usize,
}

static LOANWORD_MATCHER: OnceLock<AhoCorasick> = OnceLock::new();

fn loanword_matcher() -> &'static AhoCorasick {
    LOANWORD_MATCHER.get_or_init(|| {
        AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(LOANWORDS.iter().map(|(word, _)| *word))
            .expect("valid Aho-Corasick patterns")
    })
}

/// Replace whole-word loanwords with their preferred Persian equivalent.
pub fn persianize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;

    for found in loanword_matcher().find_iter(text) {
        if !word_boundary(text, found.start(), found.end()) {
            continue;
        }
        out.push_str(&text[last..found.start()]);
        out.push_str(LOANWORDS[found.pattern().as_usize()].1[0]);
        last = found.end();
    }
    out.push_str(&text[last..]);
    out
}

/// All whole-word loanwords with their alternatives and character spans.
pub fn persianize_suggest(text: &str) -> Vec<LoanwordMatch> {
    let mut matches = Vec::new();
    // Matches come left to right, so count characters once instead of per match.
    let mut last_byte = 0;
    let mut last_char = 0;
    for found in loanword_matcher().find_iter(text) {
        let start = last_char + text[last_byte..found.start()].chars().count();
        let end = start + text[found.start()..found.end()].chars().count();
        last_byte = found.end();
        last_char = end;
        if !word_boundary(text, found.start(), found.end()) {
            continue;
        }
        let (word, alternatives) = LOANWORDS[found.pattern().as_usize()];
        matches.push(LoanwordMatch {
            word: word.to_string(),
            alternatives: alternatives.iter().map(|alt| (*alt).to_string()).collect(),
            start,
            end,
        });
    }
    matches
}

/// Whether `text[start..end]` is a standalone word: neither neighbour continues it.
///
/// One predicate for the whole crate. `maskers` used to keep its own identical
/// copy; two copies of a boundary rule drift, and these two had already been
/// described differently in their comments while deciding the same inputs the
/// same way. A word ends at a character that is neither a Persian letter nor
/// ASCII alphanumeric, so `استx` is not the stopword `است` and `کامپیوتر2` is not
/// the offensive `کامپیوتر`.
pub(crate) fn word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_word_char(ch));
    let after = text[end..]
        .chars()
        .next()
        .is_none_or(|ch| !is_word_char(ch));
    before && after
}

fn is_word_char(ch: char) -> bool {
    // The four Arabic-coded letters `mask_non_persian` keeps, because `normalize`
    // maps them to Persian: without them a stopword's tail was left dangling and
    // `mask("آنهاي")` returned `"ي"`.
    is_persian_letter(ch) || matches!(ch, 'ي' | 'ك' | 'ى' | 'ة') || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_integers() {
        assert_eq!(number_to_words(0), "صفر");
        assert_eq!(number_to_words(1), "یک");
        assert_eq!(number_to_words(19), "نوزده");
        assert_eq!(number_to_words(21), "بیست و یک");
        assert_eq!(number_to_words(100), "صد");
        assert_eq!(number_to_words(101), "صد و یک");
        assert_eq!(number_to_words(999), "نهصد و نود و نه");
        assert_eq!(number_to_words(1000), "یک هزار");
        assert_eq!(number_to_words(1234), "یک هزار و دویست و سی و چهار");
        assert_eq!(number_to_words(1_000_000), "یک میلیون");
        assert_eq!(number_to_words(-5), "منفی پنج");
    }

    #[test]
    fn converts_numeric_text() {
        assert_eq!(
            number_to_words_text("۱۴۰۴").unwrap(),
            "یک هزار و چهارصد و چهار"
        );
        assert_eq!(number_to_words_text("3.14").unwrap(), "سه ممیز چهارده");
        // A leading zero in the fraction is a word: `3.05` and `3.5` differ.
        assert_eq!(number_to_words_text("3.05").unwrap(), "سه ممیز صفر پنج");
        assert_eq!(number_to_words_text("3.5").unwrap(), "سه ممیز پنج");
        assert_eq!(number_to_words_text("3.00").unwrap(), "سه ممیز صفر صفر");
        assert_eq!(number_to_words_text("-0.5").unwrap(), "منفی صفر ممیز پنج");
        assert!(number_to_words_text("12a").is_err());
        assert!(number_to_words_text("1.").is_err());
    }

    #[test]
    fn arabic_number_separators_are_accepted() {
        // `٬` groups thousands and `٫` is the decimal point; leaving them in the
        // string made `۱٫۵` unparseable and `۱۲٬۰۰۰` three tokens.
        assert_eq!(number_to_words_text("۱۲٬۰۰۰").unwrap(), "دوازده هزار");
        assert_eq!(number_to_words_text("۱٫۵").unwrap(), "یک ممیز پنج");
        assert_eq!(number_to_words_text("۱,۵").unwrap(), "پانزده");
    }

    #[test]
    fn persianizes_known_loanwords() {
        assert_eq!(persianize("فایل را کامپیوتر کن"), "پرونده را رایانه کن");
        assert_eq!(persianize("کامپیوتری"), "کامپیوتری");
        let matches = persianize_suggest("فایل و کامپیوتر");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].word, "فایل");
        assert_eq!(matches[0].alternatives, ["پرونده"]);
        assert_eq!((matches[0].start, matches[0].end), (0, 4));

        // Spans are character offsets, not bytes, and stay right after a skipped match.
        let spaced = persianize_suggest("«فایل» و کامپیوتر");
        let spans: Vec<(usize, usize)> = spaced.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(spans, [(1, 5), (9, 17)]);
        assert_eq!(spaced[1].word, "کامپیوتر");
    }
    #[test]
    fn a_word_ends_at_an_arabic_coded_persian_letter() {
        // `is_word_char` used only `PERSIAN_LETTERS`, which has no `\u{064A}`
        // (`\u{064A}` is written here the way the crate spells it) and friends,
        // though `mask_non_persian` keeps exactly those because `normalize` maps
        // them. So a stopword followed by one was masked and left its tail behind.
        for (text, tail) in [
            ("\u{0622}\u{0646}\u{0647}\u{0627}\u{064A}", "\u{064A}"),
            ("\u{0627}\u{0633}\u{062A}\u{064A}", "\u{064A}"),
            (
                "\u{0645}\u{0646} \u{0627}\u{0632}\u{064A} \u{06AF}\u{0641}\u{062A}\u{0645}",
                "\u{064A}",
            ),
        ] {
            assert!(
                !word_boundary(text, 0, text.len() - tail.len()),
                "{text:?}: the trailing letter must count as part of the word"
            );
        }
        // The same letters in a normal word are not a boundary either.
        assert!(!word_boundary(
            "\u{0643}\u{062A}\u{0627}\u{0628}\u{064A}",
            0,
            8
        ));
        // An ASCII letter still ends a word.
        assert!(word_boundary("\u{0633}\u{0644}\u{0627}\u{0645} x", 0, 8));
    }
}
