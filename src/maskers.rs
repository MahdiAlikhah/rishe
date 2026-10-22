use crate::normalize_ops::{ALL_PUNCT, PERSIAN_LETTERS};
use crate::stopwords;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use std::sync::OnceLock;

const ZWNJ: char = '\u{200C}';
const PERSIAN_DIGITS: &str = "۰۱۲۳۴۵۶۷۸۹";
const ARABIC_DIGITS: &str = "٠١٢٣٤٥٦٧٨٩";
const DIACRITICS: &str = "ًٌٍَُِّْ";
const VS16: char = '\u{FE0F}';
const ZWJ: char = '\u{200D}';
const OFFENSIVE_WORDS: &str = include_str!("../data/offensive_words.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskOp {
    Url,
    Email,
    Mention,
    Hashtag,
    Emoji,
    Html,
    NonPersian,
    Stopwords,
    Offensive,
    Digits,
    Diacritics,
    Punctuation,
}

impl MaskOp {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "url" | "urls" => Self::Url,
            "email" | "emails" => Self::Email,
            "mention" | "mentions" => Self::Mention,
            "hashtag" | "hashtags" => Self::Hashtag,
            "emoji" | "emojis" => Self::Emoji,
            "html" | "html_tag" | "html_tags" => Self::Html,
            "non_persian" | "non_persian_letters" => Self::NonPersian,
            "stopword" | "stopwords" => Self::Stopwords,
            "offensive" | "offensive_words" => Self::Offensive,
            "digit" | "digits" => Self::Digits,
            "diacritic" | "diacritics" => Self::Diacritics,
            "punctuation" | "punctuations" => Self::Punctuation,
            _ => return None,
        })
    }

    pub fn default_mask(self) -> &'static str {
        match self {
            Self::Url => "<URL>",
            Self::Email => "<EMAIL>",
            Self::Mention | Self::Hashtag | Self::Html => " ",
            _ => "",
        }
    }

    pub fn apply(self, text: &str, mask: Option<&str>) -> String {
        let mask = mask.unwrap_or_else(|| self.default_mask());
        match self {
            Self::Url => mask_urls(text, mask),
            Self::Email => mask_emails(text, mask),
            Self::Mention => mask_sigil(text, '@', mask),
            Self::Hashtag => mask_sigil(text, '#', mask),
            Self::Emoji => mask_emojis(text, mask),
            Self::Html => mask_html(text, mask),
            Self::NonPersian => mask_non_persian(text, mask, false, false),
            Self::Stopwords => mask_stopwords(text, mask),
            Self::Offensive => mask_offensive(text, mask),
            Self::Digits => mask_digits(text, mask),
            Self::Diacritics => mask_diacritics(text, mask),
            Self::Punctuation => mask_punctuation(text, mask),
        }
    }
}

pub fn mask_urls(text: &str, mask: &str) -> String {
    if !text.contains("http") {
        return text.trim().to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < text.len() {
        if text[i..].starts_with("http://") || text[i..].starts_with("https://") {
            let mut end = text.len();
            for (offset, ch) in text[i..].char_indices() {
                if ch.is_whitespace() {
                    end = i + offset;
                    break;
                }
            }
            out.push_str(mask);
            i = end;
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    out.trim().to_string()
}

pub fn mask_emails(text: &str, mask: &str) -> String {
    if !text.contains('@') {
        return text.trim().to_string();
    }

    let spans = email_spans(text);
    if spans.is_empty() {
        return text.trim().to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end) in spans {
        out.push_str(&text[last..start]);
        out.push_str(mask);
        last = end;
    }
    out.push_str(&text[last..]);
    out.trim().to_string()
}

pub(crate) fn email_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'@' {
            let mut start = i;
            while start > 0 && is_email_local(bytes[start - 1] as char) {
                start -= 1;
            }
            while start < i && !text.is_char_boundary(start) {
                start += 1;
            }

            let mut domain_end = i + 1;
            while domain_end < bytes.len() && is_email_domain(bytes[domain_end] as char) {
                domain_end += 1;
            }

            let mut match_end = None;
            for dot in (i + 1..domain_end).rev() {
                if bytes[dot] != b'.' || dot == i + 1 {
                    continue;
                }
                let mut run_end = dot + 1;
                while run_end < domain_end && bytes[run_end].is_ascii_alphabetic() {
                    run_end += 1;
                }
                if run_end - (dot + 1) >= 2 {
                    match_end = Some(run_end);
                    break;
                }
            }

            if start < i {
                if let Some(end) = match_end {
                    spans.push((start, end));
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    spans
}

fn is_email_local(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-')
}

fn is_email_domain(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-')
}

pub fn mask_sigil(text: &str, sigil: char, mask: &str) -> String {
    if !text.contains(sigil) {
        return text.trim().to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == sigil {
            while chars.peek().is_some_and(|next| !next.is_whitespace()) {
                chars.next();
            }
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }

    out.trim().to_string()
}

pub fn mask_emojis(text: &str, mask: &str) -> String {
    if !text.chars().any(is_emoji_base) {
        return text.trim().to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if is_regional(ch) && i + 1 < chars.len() && is_regional(chars[i + 1]) {
            out.push_str(mask);
            i += 2;
            continue;
        }

        if is_emoji_base(ch) {
            out.push_str(mask);
            i += 1;
            if i < chars.len() && is_skin_tone(chars[i]) {
                i += 1;
            }
            if i < chars.len() && chars[i] == VS16 {
                i += 1;
            }
            while i + 1 < chars.len() && chars[i] == ZWJ && is_emoji_base(chars[i + 1]) {
                i += 2;
                if i < chars.len() && is_skin_tone(chars[i]) {
                    i += 1;
                }
                if i < chars.len() && chars[i] == VS16 {
                    i += 1;
                }
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    out.trim().to_string()
}

pub(crate) fn is_emoji_base(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2600..=0x26FF
            | 0x2700..=0x27BF
            | 0x1F000..=0x1F02F
            | 0x1F0A0..=0x1F0FF
            | 0x1F100..=0x1F1FF
            | 0x1F200..=0x1F2FF
            | 0x1F300..=0x1F5FF
            | 0x1F600..=0x1F64F
            | 0x1F680..=0x1F6FF
            | 0x1F700..=0x1F77F
            | 0x1F780..=0x1F7FF
            | 0x1F800..=0x1F8FF
            | 0x1F900..=0x1F9FF
            | 0x1FA00..=0x1FA6F
            | 0x1FA70..=0x1FAFF
    )
}

pub(crate) fn is_regional(ch: char) -> bool {
    matches!(ch as u32, 0x1F1E6..=0x1F1FF)
}

fn is_skin_tone(ch: char) -> bool {
    matches!(ch as u32, 0x1F3FB..=0x1F3FF)
}

pub fn mask_html(text: &str, mask: &str) -> String {
    if !text.contains('<') && !text.contains('&') {
        return text.trim().to_string();
    }

    let unescaped = html_unescape(text);
    let mut out = String::with_capacity(unescaped.len());
    let mut i = 0;

    while i < unescaped.len() {
        if unescaped.as_bytes()[i] == b'<' {
            if let Some(close) = unescaped[i..].find('>') {
                if close > 1 {
                    out.push_str(mask);
                    i += close + 1;
                    continue;
                }
            }
        }
        let ch = unescaped[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out.trim().to_string()
}

fn html_unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < text.len() {
        if text.as_bytes()[i] == b'&' {
            if let Some(semi) = text[i..].find(';').filter(|semi| *semi <= 32) {
                if let Some(replacement) = decode_entity(&text[i + 1..i + semi]) {
                    out.push_str(&replacement);
                    i += semi + 1;
                    continue;
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

// ponytail: common HTML entities and numeric references only; add the full
// HTML5 table if real input needs it.
fn decode_entity(entity: &str) -> Option<String> {
    let named = match entity {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }

    if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        return u32::from_str_radix(hex, 16)
            .ok()
            .and_then(char::from_u32)
            .map(|ch| ch.to_string());
    }
    if let Some(decimal) = entity.strip_prefix('#') {
        return decimal
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|ch| ch.to_string());
    }
    None
}

pub fn mask_non_persian(
    text: &str,
    mask: &str,
    keep_english: bool,
    keep_diacritics: bool,
) -> String {
    let mut out = String::with_capacity(text.len());

    for ch in text.chars() {
        let allowed = PERSIAN_LETTERS.contains(ch)
            || ch == ZWNJ
            || ch == ' '
            || PERSIAN_DIGITS.contains(ch)
            || ALL_PUNCT.contains(ch)
            || (keep_diacritics && DIACRITICS.contains(ch))
            || (keep_english && (ch.is_ascii_alphanumeric() || ch.is_ascii_punctuation()));

        if allowed {
            out.push(ch);
        } else {
            out.push_str(mask);
        }
    }

    out.trim().to_string()
}

pub fn mask_stopwords(text: &str, mask: &str) -> String {
    replace_words(text, stopword_matcher(), mask)
}

pub fn mask_offensive(text: &str, mask: &str) -> String {
    replace_words(text, offensive_matcher(), mask)
}

static STOPWORD_MATCHER: OnceLock<AhoCorasick> = OnceLock::new();
static OFFENSIVE_MATCHER: OnceLock<AhoCorasick> = OnceLock::new();

fn stopword_matcher() -> &'static AhoCorasick {
    STOPWORD_MATCHER.get_or_init(|| build_matcher(stopwords::stopwords()))
}

fn offensive_matcher() -> &'static AhoCorasick {
    OFFENSIVE_MATCHER.get_or_init(|| {
        build_matcher(
            OFFENSIVE_WORDS
                .lines()
                .map(str::trim)
                .filter(|word| !word.is_empty())
                .map(str::to_owned),
        )
    })
}

fn build_matcher<I, P>(patterns: I) -> AhoCorasick
where
    I: IntoIterator<Item = P>,
    P: AsRef<[u8]>,
{
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(patterns)
        .expect("valid Aho-Corasick patterns")
}

fn replace_words(text: &str, matcher: &AhoCorasick, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;

    for found in matcher.find_iter(text) {
        if !word_boundary(text, found.start(), found.end()) {
            continue;
        }
        out.push_str(&text[last..found.start()]);
        out.push_str(mask);
        last = found.end();
    }
    out.push_str(&text[last..]);
    out.trim().to_string()
}

fn word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start]
        .chars()
        .next_back()
        .map_or(true, |ch| !is_persian_letter(ch));
    let after = text[end..]
        .chars()
        .next()
        .map_or(true, |ch| !is_persian_letter(ch));
    before && after
}

fn is_persian_letter(ch: char) -> bool {
    PERSIAN_LETTERS.contains(ch) || ch == ZWNJ
}

pub fn mask_digits(text: &str, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_digit() || PERSIAN_DIGITS.contains(ch) || ARABIC_DIGITS.contains(ch) {
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

pub fn mask_diacritics(text: &str, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if DIACRITICS.contains(ch) {
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn mask_punctuation(text: &str, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ALL_PUNCT.contains(ch) || ch.is_ascii_punctuation() {
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_links_and_contacts() {
        assert_eq!(
            MaskOp::Url.apply("برای اطلاعات بیشتر به https://shekar.io مراجعه کنید.", None),
            "برای اطلاعات بیشتر به <URL> مراجعه کنید."
        );
        assert_eq!(
            MaskOp::Email.apply("برای تماس با ما به info@shekar.io ایمیل بزنید.", None),
            "برای تماس با ما به <EMAIL> ایمیل بزنید."
        );
        assert_eq!(
            mask_emails("mail info@a.co, ok", "<EMAIL>"),
            "mail <EMAIL>, ok"
        );
        assert_eq!(
            MaskOp::Url.apply("x https://a.b y", Some("[MASK]")),
            "x [MASK] y"
        );
    }

    #[test]
    fn masks_social_tokens() {
        assert_eq!(
            MaskOp::Mention.apply("سلام @user! چطوری؟", None),
            "سلام   چطوری؟"
        );
        assert_eq!(MaskOp::Hashtag.apply("#سلام #خوش_آمدید", None), "");
    }

    #[test]
    fn masks_emoji_sequences() {
        assert_eq!(
            MaskOp::Emoji.apply("درود بر شما😊!🌟", None),
            "درود بر شما!"
        );
        assert_eq!(MaskOp::Emoji.apply("family 👨‍👩‍👧‍👦", None), "family");
        assert_eq!(MaskOp::Emoji.apply("flag 🇮🇷", None), "flag");
    }

    #[test]
    fn masks_html_tags_and_entities() {
        assert_eq!(
            MaskOp::Html.apply("<p>این یک <strong>متن</strong> نمونه است.</p>", None),
            "این یک  متن  نمونه است."
        );
        assert_eq!(mask_html("a &amp; b", ""), "a & b");
        assert_eq!(mask_html("&#1587;&#x644;&#x627;&#1605;", ""), "سلام");
    }

    #[test]
    fn masks_non_persian_letters() {
        assert_eq!(MaskOp::NonPersian.apply("سلام hello 123!", None), "سلام  !");
        assert_eq!(
            mask_non_persian("سلام hello 123!", "", true, false),
            "سلام hello 123!"
        );
    }

    #[test]
    fn masks_stopwords_and_offensive_words() {
        assert_eq!(MaskOp::Stopwords.apply("این کتاب است", None), "کتاب");
        assert_eq!(mask_stopwords("کتاب و دفتر", ""), "کتاب  دفتر");
        assert_eq!(MaskOp::Offensive.apply("این بی‌شرف است", None), "این  است");
    }

    #[test]
    fn masks_digits_diacritics_punctuation() {
        assert_eq!(
            MaskOp::Digits.apply("این متن 1234 شامل اعداد است.", None),
            "این متن  شامل اعداد است."
        );
        assert_eq!(
            MaskOp::Diacritics.apply("کُجا نِشانِ قَدَم ناتَمام خواهَد ماند؟", None),
            "کجا نشان قدم ناتمام خواهد ماند؟"
        );
        assert_eq!(
            MaskOp::Punctuation.apply("دریغ است ایران که ویران شود!", None),
            "دریغ است ایران که ویران شود"
        );
    }

    #[test]
    fn op_names_resolve() {
        assert_eq!(MaskOp::from_name("url"), Some(MaskOp::Url));
        assert_eq!(MaskOp::from_name("stopwords"), Some(MaskOp::Stopwords));
        assert_eq!(MaskOp::from_name("nope"), None);
    }
}
