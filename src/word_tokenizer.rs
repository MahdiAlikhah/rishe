use std::collections::VecDeque;

use crate::maskers::{email_spans, emoji_end};
use crate::morphology::{MorphologyEngine, NON_PARTICIPLE_NOUNS, PARTICIPLE_NOUNS};

/// Switches for [`crate::Pipeline::tokenize`]; all default to `false`.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordTokenizerOptions {
    /// Join auxiliary verbs to the preceding participle with `_`.
    pub join_verb_parts: bool,
    /// Join `( X )` abbreviations into `(X)`.
    pub join_abbreviations: bool,
    /// Add spaces around emoji so they tokenize separately.
    pub separate_emoji: bool,
    /// Replace links with the ` LINK ` placeholder.
    pub replace_links: bool,
    /// Replace `@mentions` with the ` ID ` placeholder.
    pub replace_ids: bool,
    /// Replace email addresses with the ` EMAIL ` placeholder.
    pub replace_emails: bool,
    /// Replace numbers with a ` NUMn ` or ` NUMF ` placeholder.
    pub replace_numbers: bool,
    /// Replace hashtags with ` TAG `, turning `_` inside the tag into spaces.
    pub replace_hashtags: bool,
}

/// Apply the rewrites that must run before tokenizing.
pub fn apply_pre(text: &str, options: &WordTokenizerOptions) -> String {
    let mut current = text.to_string();
    if options.separate_emoji {
        current = separate_emoji(&current);
    }
    if options.replace_emails {
        current = replace_emails(&current);
    }
    if options.replace_links {
        current = replace_links(&current);
    }
    if options.replace_ids {
        current = replace_ids(&current);
    }
    if options.replace_hashtags {
        current = replace_hashtags(&current);
    }
    current
}

/// Apply the rewrites that must run after tokenizing.
pub fn postprocess(
    tokens: Vec<String>,
    options: &WordTokenizerOptions,
    morph: &MorphologyEngine,
) -> Vec<String> {
    let mut tokens = tokens;
    if options.join_verb_parts {
        tokens = join_verb_parts(tokens, morph);
    }
    if options.join_abbreviations {
        tokens = join_abbreviations(tokens);
    }
    tokens
}

fn separate_emoji(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;

    while i < chars.len() {
        if let Some(end) = emoji_end(&chars, i) {
            out.extend(&chars[i..end]);
            out.push(' ');
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn replace_emails(text: &str) -> String {
    let spans = email_spans(text);
    if spans.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end) in spans {
        out.push_str(&text[last..start]);
        out.push_str(" EMAIL ");
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

/// Prefixes that begin a URL.
const URL_PREFIXES: [&str; 4] = ["https://", "http://", "ftp://", "www."];

/// Punctuation that ends the sentence rather than the URL.
const URL_TRAILING: [char; 8] = [')', '،', '.', '»', ',', ';', '!', '?'];

/// The end byte index of the URL starting at `start`, or `None` when no URL
/// starts there.
///
/// Shared by [`replace_links`] and the `url` masker so the two cannot disagree,
/// and so both agree that `(https://a.com)` is a URL followed by `)`.
pub(crate) fn url_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    let prefix_len = URL_PREFIXES
        .iter()
        .find(|prefix| rest.starts_with(**prefix))
        .map(|prefix| prefix.len())?;
    // A URL does not start inside a word.
    if start > 0
        && text[..start]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
    {
        return None;
    }
    let mut end = text.len();
    for (offset, ch) in rest.char_indices() {
        if ch.is_whitespace() {
            end = start + offset;
            break;
        }
    }
    // Trailing punctuation belongs to the sentence around the URL.
    while let Some(ch) = text[..end].chars().next_back() {
        if !URL_TRAILING.contains(&ch) {
            break;
        }
        end -= ch.len_utf8();
    }
    // Trimming must not leave nothing but the prefix, as in a bare `www.`.
    (end > start + prefix_len).then_some(end)
}

fn replace_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(end) = url_end(text, i) {
            out.push_str(" LINK ");
            i = end;
            continue;
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// End of the `sigil` token starting at `i`, or `None` when the sigil is part of
/// a word or has no body. The maskers and the tokenizer share this scan and pass
/// their own boundary rules, so `@name` and `#tag` cannot drift apart again.
pub(crate) fn sigil_end(
    chars: &[char],
    i: usize,
    sigil: char,
    previous: fn(char) -> bool,
    body: fn(char) -> bool,
) -> Option<usize> {
    if chars[i] != sigil || (i > 0 && !previous(chars[i - 1])) {
        return None;
    }
    let mut end = i + 1;
    while end < chars.len() && body(chars[end]) {
        end += 1;
    }
    (end > i + 1).then_some(end)
}

fn replace_ids(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        let previous = |ch: char| !(ch.is_alphanumeric() || matches!(ch, '_' | '.'));
        if let Some(end) = sigil_end(&chars, i, '@', previous, |ch| {
            ch.is_alphanumeric() || ch == '_'
        }) {
            out.push_str(" ID ");
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn replace_hashtags(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        let previous = |ch: char| !(ch.is_alphanumeric() || matches!(ch, '_' | '.'));
        if let Some(end) = sigil_end(&chars, i, '#', previous, |ch| !ch.is_whitespace()) {
            out.push_str(" TAG ");
            for &ch in &chars[i + 1..end] {
                out.push(if ch == '_' { ' ' } else { ch });
            }
            out.push(' ');
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub(crate) fn replace_numbers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        if is_digit(chars[i]) {
            let start = i;
            let mut end = i;
            let mut decimal = false;
            while end < chars.len() {
                if is_digit(chars[end]) {
                    end += 1;
                } else if is_number_separator(chars[end])
                    && end + 1 < chars.len()
                    && is_digit(chars[end + 1])
                {
                    // Only a decimal point makes this a fraction; a thousands
                    // separator keeps it an integer, so `۱٬۰۰۰` is NUM4.
                    decimal = decimal || is_decimal_separator(chars[end]);
                    end += 1;
                } else {
                    break;
                }
            }
            let previous_ok = start == 0 || !is_word_char(chars[start - 1]);
            let next_ok = end == chars.len() || !is_word_char(chars[end]);
            if previous_ok && next_ok {
                if decimal {
                    out.push_str(" NUMF ");
                } else {
                    let digits = chars[start..end].iter().filter(|ch| is_digit(**ch)).count();
                    out.push_str(&format!(" NUM{digits} "));
                }
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
        || ('\u{06F0}'..='\u{06F9}').contains(&ch)
        || ('\u{0660}'..='\u{0669}').contains(&ch)
}

fn is_number_separator(ch: char) -> bool {
    matches!(ch, '.' | '\u{066B}' | '\u{066C}' | ',')
}

/// The separators that mark a decimal point: ASCII `.` and Arabic `٫`. `,` and
/// `٬` group thousands and must not turn `۱٬۰۰۰` into a fraction.
fn is_decimal_separator(ch: char) -> bool {
    matches!(ch, '.' | '\u{066B}')
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

const ABBREVIATIONS: &[&str] = &["ع", "ص", "س", "ره", "قدس", "ط", "ق", "م", "د", "هـ"];

fn join_abbreviations(tokens: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    // A queue rather than index-and-clone: the tokens are already owned here.
    let mut rest: VecDeque<String> = tokens.into();
    while let Some(token) = rest.pop_front() {
        if token == "("
            && rest.len() >= 2
            && rest[1] == ")"
            && ABBREVIATIONS.contains(&rest[0].as_str())
        {
            out.push(format!("({})", rest[0]));
            rest.pop_front();
            rest.pop_front();
            continue;
        }
        out.push(token);
    }
    out
}

const BEFORE_VERBS: &[&str] = &[
    "خواهم",
    "خواهی",
    "خواهد",
    "خواهیم",
    "خواهید",
    "خواهند",
    "نخواهم",
    "نخواهی",
    "نخواهد",
    "نخواهیم",
    "نخواهید",
    "نخواهند",
];

const AFTER_VERBS: &[&str] = &[
    "ام",
    "ای",
    "است",
    "ایم",
    "اید",
    "اند",
    "بودم",
    "بودی",
    "بود",
    "بودیم",
    "بودید",
    "بودند",
    "باشم",
    "باشی",
    "باشد",
    "باشیم",
    "باشید",
    "باشند",
    "شده_ام",
    "شده_ای",
    "شده_است",
    "شده_ایم",
    "شده_اید",
    "شده_اند",
    "شده_بودم",
    "شده_بودی",
    "شده_بود",
    "شده_بودیم",
    "شده_بودید",
    "شده_بودند",
    "شده_باشم",
    "شده_باشی",
    "شده_باشد",
    "شده_باشیم",
    "شده_باشید",
    "شده_باشند",
    "نشده_ام",
    "نشده_ای",
    "نشده_است",
    "نشده_ایم",
    "نشده_اید",
    "نشده_اند",
    "نشده_بودم",
    "نشده_بودی",
    "نشده_بود",
    "نشده_بودیم",
    "نشده_بودید",
    "نشده_بودند",
    "نشده_باشم",
    "نشده_باشی",
    "نشده_باشد",
    "نشده_باشیم",
    "نشده_باشید",
    "نشده_باشند",
    "شوم",
    "شوی",
    "شود",
    "شویم",
    "شوید",
    "شوند",
    "شدم",
    "شدی",
    "شد",
    "شدیم",
    "شدید",
    "شدند",
    "نشوم",
    "نشوی",
    "نشود",
    "نشویم",
    "نشوید",
    "نشوند",
    "نشدم",
    "نشدی",
    "نشد",
    "نشدیم",
    "نشدید",
    "نشدند",
    "می‌شوم",
    "می‌شوی",
    "می‌شود",
    "می‌شویم",
    "می‌شوید",
    "می‌شوند",
    "می‌شدم",
    "می‌شدی",
    "می‌شد",
    "می‌شدیم",
    "می‌شدید",
    "می‌شدند",
    "نمی‌شوم",
    "نمی‌شوی",
    "نمی‌شود",
    "نمی‌شویم",
    "نمی‌شوید",
    "نمی‌شوند",
    "نمی‌شدم",
    "نمی‌شدی",
    "نمی‌شد",
    "نمی‌شدیم",
    "نمی‌شدید",
    "نمی‌شدند",
    "خواهم_شد",
    "خواهی_شد",
    "خواهد_شد",
    "خواهیم_شد",
    "خواهید_شد",
    "خواهند_شد",
    "نخواهم_شد",
    "نخواهی_شد",
    "نخواهد_شد",
    "نخواهیم_شد",
    "نخواهید_شد",
    "نخواهند_شد",
];

fn join_verb_parts(tokens: Vec<String>, morph: &MorphologyEngine) -> Vec<String> {
    if tokens.len() <= 1 {
        return tokens;
    }
    let mut result: Vec<String> = Vec::with_capacity(tokens.len());
    for token in tokens.into_iter().rev() {
        let join = !result.is_empty()
            && ((BEFORE_VERBS.contains(&token.as_str())
                && result.last().is_some_and(|last| is_verb_after(last, morph)))
                || (result
                    .last()
                    .is_some_and(|last| AFTER_VERBS.contains(&last.as_str()))
                    && is_participle(&token, morph)));
        if join {
            let last = result.last_mut().unwrap();
            *last = format!("{token}_{last}");
        } else {
            result.push(token);
        }
    }
    result.reverse();
    result
}

/// A future auxiliary (`خواهد`) joins the verb it governs, not whatever token
/// happens to follow it, so `خواهد .` and `خواهم که` stay apart.
fn is_verb_after(token: &str, morph: &MorphologyEngine) -> bool {
    morph.is_past_stem(token) || is_participle(token, morph)
}

fn is_participle(token: &str, morph: &MorphologyEngine) -> bool {
    // `خنده`, `دانه`, `هسته` end in ه over a verb stem but are nouns, not the
    // participles of خندیدن/دانستن/بودن; the lemmatizer already knows that, so
    // the join rule has to use the same stoplist or `خنده است` becomes one token.
    // Words on the stoplist that are also real participles (`گفته`, `مرده`) keep
    // joining, which is what `گفته شده است` needs.
    if NON_PARTICIPLE_NOUNS.contains(&token) && !PARTICIPLE_NOUNS.contains(&token) {
        return false;
    }
    if let Some(stem) = token.strip_suffix('ه') {
        if !stem.is_empty() && (morph.is_known_verb_stem(stem) || morph.is_past_stem(stem)) {
            return true;
        }
    }
    if let Some(rest) = token.strip_prefix('ن') {
        if let Some(stem) = rest.strip_suffix('ه') {
            if !stem.is_empty() && (morph.is_known_verb_stem(stem) || morph.is_past_stem(stem)) {
                return true;
            }
        }
    }
    let lemma = morph.lemmatize(token);
    lemma.as_ref() != token && morph.present_stem(lemma.as_ref()).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> WordTokenizerOptions {
        WordTokenizerOptions::default()
    }

    #[test]
    fn joins_multi_part_verbs() {
        let morph = MorphologyEngine::new();
        assert_eq!(
            join_verb_parts(vec!["رفته".to_string(), "است".to_string()], &morph),
            ["رفته_است"]
        );
        assert_eq!(
            join_verb_parts(
                vec!["گفته".to_string(), "شده".to_string(), "است".to_string()],
                &morph
            ),
            ["گفته_شده_است"]
        );
        assert_eq!(
            join_verb_parts(vec!["خواهد".to_string(), "رفت".to_string()], &morph),
            ["خواهد_رفت"]
        );
        assert_eq!(
            join_verb_parts(vec!["خسته".to_string(), "شدید".to_string()], &morph),
            ["خسته_شدید"]
        );
    }

    #[test]
    fn joins_future_auxiliary_to_its_verb_only() {
        let morph = MorphologyEngine::new();
        let tok =
            |text: &str| join_verb_parts(text.split(' ').map(str::to_string).collect(), &morph);
        assert_eq!(tok("خواهم رفت"), ["خواهم_رفت"]);
        // A bare auxiliary joined anything that followed it.
        assert_eq!(tok("او خواهد ."), ["او", "خواهد", "."]);
        assert_eq!(tok("خواهم که بروم"), ["خواهم", "که", "بروم"]);
        assert_eq!(tok("خواهد شد"), ["خواهد_شد"]);
    }

    #[test]
    fn a_trailing_verb_prefix_is_not_joined() {
        let morph = MorphologyEngine::new();
        assert_eq!(
            join_verb_parts(vec!["رفته".to_string(), "خواهد".to_string()], &morph),
            ["رفته", "خواهد"]
        );
    }

    #[test]
    fn preprocesses_tokenizer_options() {
        assert_eq!(separate_emoji("سلام😊دنیا"), "سلام😊 دنیا");
        // A ZWJ joins the parts into one emoji, so the family stays one token
        // exactly as the `emoji` masker keeps it.
        assert_eq!(
            separate_emoji("x\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}y"),
            "x\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} y"
        );
        assert_eq!(replace_emails("info@a.co"), " EMAIL ");
        assert_eq!(replace_links("به https://a.b برو"), "به  LINK  برو");
        // The same span rule the `url` masker uses.
        assert_eq!(replace_links("(https://a.com) بعد"), "( LINK ) بعد");
        assert_eq!(replace_links("www.a.com"), " LINK ");
        assert_eq!(replace_links("ftp://a.com"), " LINK ");
        assert_eq!(replace_links("abchttps://a.com"), "abchttps://a.com");
        assert_eq!(replace_ids("@user سلام"), " ID  سلام");
        assert_eq!(replace_hashtags("#خبر_فوری سلام"), " TAG خبر فوری  سلام");
        // A `#` inside a word is not a tag, matching how `@` is handled.
        assert_eq!(replace_hashtags("abc#def"), "abc#def");
        assert_eq!(replace_ids("abc@def"), "abc@def");
        assert_eq!(replace_numbers("سال ۱۴۰۴ و 3.14"), "سال  NUM4  و  NUMF ");
        // A thousands separator is not a decimal point: four digits, not a
        // fraction. `٬` (U+066C) and `,` group; `.` and `٫` (U+066B) divide.
        assert_eq!(replace_numbers("۱٬۰۰۰ تومان"), " NUM4  تومان");
        assert_eq!(replace_numbers("1,000"), " NUM4 ");
        assert_eq!(replace_numbers("۳٫۱۴"), " NUMF ");
        assert_eq!(replace_numbers("1,234.5"), " NUMF ");
    }

    #[test]
    fn joins_parenthesized_abbreviations() {
        assert_eq!(
            join_abbreviations(vec!["(".into(), "ع".into(), ")".into()]),
            ["(ع)"]
        );
        assert_eq!(
            join_abbreviations(vec!["(".into(), "کتاب".into(), ")".into()]),
            ["(", "کتاب", ")"]
        );
    }

    #[test]
    fn defaults_are_off() {
        let options = options();
        assert!(!options.join_verb_parts);
        assert!(!options.separate_emoji);
        assert!(!options.replace_links);
    }
}
