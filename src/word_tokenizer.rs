use crate::maskers::{email_spans, is_emoji_base, is_regional};
use crate::morphology::MorphologyEngine;

const VS16: char = '\u{FE0F}';

#[derive(Debug, Clone, Copy, Default)]
pub struct WordTokenizerOptions {
    pub join_verb_parts: bool,
    pub join_abbreviations: bool,
    pub separate_emoji: bool,
    pub replace_links: bool,
    pub replace_ids: bool,
    pub replace_emails: bool,
    pub replace_numbers: bool,
    pub replace_hashtags: bool,
}

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
        let ch = chars[i];
        if is_regional(ch) && i + 1 < chars.len() && is_regional(chars[i + 1]) {
            out.push(ch);
            out.push(chars[i + 1]);
            out.push(' ');
            i += 2;
            continue;
        }
        if is_emoji_base(ch) {
            out.push(ch);
            i += 1;
            if i < chars.len() && is_skin_tone(chars[i]) {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == VS16 {
                out.push(chars[i]);
                i += 1;
            }
            out.push(' ');
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn is_skin_tone(ch: char) -> bool {
    matches!(ch as u32, 0x1F3FB..=0x1F3FF)
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

fn replace_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let prefix = ["https://", "http://", "ftp://", "www."]
            .into_iter()
            .find(|candidate| rest.starts_with(candidate));
        if let Some(prefix) = prefix {
            let previous_ok = i == 0
                || text[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| !ch.is_alphanumeric());
            if previous_ok {
                let mut end = i + prefix.len();
                for (offset, ch) in rest[prefix.len()..].char_indices() {
                    if ch.is_whitespace() {
                        break;
                    }
                    end = i + prefix.len() + offset + ch.len_utf8();
                }
                out.push_str(" LINK ");
                i = end;
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn replace_ids(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '@' {
            let previous_ok =
                i == 0 || !(chars[i - 1].is_alphanumeric() || matches!(chars[i - 1], '_' | '.'));
            let mut end = i + 1;
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            if previous_ok && end > i + 1 {
                out.push_str(" ID ");
                i = end;
                continue;
            }
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
        if chars[i] == '#' {
            let mut end = i + 1;
            while end < chars.len() && !chars[end].is_whitespace() {
                end += 1;
            }
            if end > i + 1 {
                out.push_str(" TAG ");
                for &ch in &chars[i + 1..end] {
                    out.push(if ch == '_' { ' ' } else { ch });
                }
                out.push(' ');
                i = end;
                continue;
            }
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
                    decimal = true;
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

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

const ABBREVIATIONS: &[&str] = &["ع", "ص", "س", "ره", "قدس", "ط", "ق", "م", "د", "هـ"];

fn join_abbreviations(tokens: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if i + 2 < tokens.len()
            && tokens[i] == "("
            && tokens[i + 2] == ")"
            && ABBREVIATIONS.contains(&tokens[i + 1].as_str())
        {
            out.push(format!("({})", tokens[i + 1]));
            i += 3;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
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
        let join = BEFORE_VERBS.contains(&token.as_str())
            || (result
                .last()
                .is_some_and(|last| AFTER_VERBS.contains(&last.as_str()))
                && is_participle(&token, morph));
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

fn is_participle(token: &str, morph: &MorphologyEngine) -> bool {
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
    fn preprocesses_tokenizer_options() {
        assert_eq!(separate_emoji("سلام😊دنیا"), "سلام😊 دنیا");
        assert_eq!(replace_emails("info@a.co"), " EMAIL ");
        assert_eq!(replace_links("به https://a.b برو"), "به  LINK  برو");
        assert_eq!(replace_ids("@user سلام"), " ID  سلام");
        assert_eq!(replace_hashtags("#خبر_فوری سلام"), " TAG خبر فوری  سلام");
        assert_eq!(replace_numbers("سال ۱۴۰۴ و 3.14"), "سال  NUM4  و  NUMF ");
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
