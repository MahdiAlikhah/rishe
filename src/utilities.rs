use crate::normalize_ops::PERSIAN_LETTERS;
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

pub fn number_to_words(number: i64) -> String {
    if number < 0 {
        return format!("منفی {}", unsigned_to_words(number.unsigned_abs()));
    }
    unsigned_to_words(number as u64)
}

pub fn number_to_words_text(text: &str) -> Result<String, String> {
    let normalized: String = text
        .trim()
        .chars()
        .map(|ch| match ch {
            '\u{06F0}'..='\u{06F9}' => char::from_u32(ch as u32 - 0x06F0 + 0x30).unwrap(),
            '\u{0660}'..='\u{0669}' => char::from_u32(ch as u32 - 0x0660 + 0x30).unwrap(),
            _ => ch,
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
        let decimal_value: u64 = decimal
            .parse()
            .map_err(|_| format!("number out of range: {text}"))?;
        words = format!("{words} ممیز {}", unsigned_to_words(decimal_value));
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
            if remainder % 10 > 0 {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanwordMatch {
    pub word: String,
    pub alternatives: Vec<String>,
    pub start: usize,
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

pub fn persianize_suggest(text: &str) -> Vec<LoanwordMatch> {
    persianize_matches(text)
        .into_iter()
        .map(|(word, alternatives, start, end)| LoanwordMatch {
            word: word.to_string(),
            alternatives: alternatives.iter().map(|alt| (*alt).to_string()).collect(),
            start,
            end,
        })
        .collect()
}

fn persianize_matches(text: &str) -> Vec<(&'static str, &'static [&'static str], usize, usize)> {
    let mut matches = Vec::new();
    for found in loanword_matcher().find_iter(text) {
        if !word_boundary(text, found.start(), found.end()) {
            continue;
        }
        let (word, alternatives) = LOANWORDS[found.pattern().as_usize()];
        let start = text[..found.start()].chars().count();
        let end = text[..found.end()].chars().count();
        matches.push((word, alternatives, start, end));
    }
    matches
}

fn word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start]
        .chars()
        .next_back()
        .map_or(true, |ch| !is_word_char(ch));
    let after = text[end..]
        .chars()
        .next()
        .map_or(true, |ch| !is_word_char(ch));
    before && after
}

fn is_word_char(ch: char) -> bool {
    PERSIAN_LETTERS.contains(ch) || ch == '\u{200C}' || ch.is_ascii_alphanumeric()
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
        assert_eq!(number_to_words_text("-0.5").unwrap(), "منفی صفر ممیز پنج");
        assert!(number_to_words_text("12a").is_err());
        assert!(number_to_words_text("1.").is_err());
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
    }
}
