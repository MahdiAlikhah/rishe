const PERSIAN_ABBREVIATIONS: &[&str] = &[
    "ه\u{200C}ش",
    "ه.ش",
    "ه\u{200C}ق",
    "ه.ق",
    "ق.م",
    "ص",
    "صص",
    "ج",
    "رک",
    "ر.ک",
    "د",
    "م",
    "ج.ا",
];

pub struct SentenceSplitter;

impl SentenceSplitter {
    #[inline]
    pub fn split(text: &str) -> Vec<&str> {
        let mut sentences = Vec::new();
        let mut sent_start = 0;
        let mut chars = text.char_indices().peekable();

        while let Some((idx, ch)) = chars.next() {
            if ch == '\n' {
                let slice = text[sent_start..idx].trim();
                if !slice.is_empty() {
                    sentences.push(slice);
                }
                sent_start = idx + ch.len_utf8();
                continue;
            }

            if matches!(ch, '.' | '؟' | '?' | '!' | '۔') {
                if ch == '.' && Self::is_decimal_dot(text, idx) {
                    continue;
                }

                let mut end_punct_idx = idx + ch.len_utf8();
                let mut is_ellipsis = false;
                while let Some(&(next_idx, next_ch)) = chars.peek() {
                    if matches!(next_ch, '.' | '؟' | '?' | '!' | '۔') {
                        if ch == '.' && next_ch == '.' {
                            is_ellipsis = true;
                        }
                        end_punct_idx = next_idx + next_ch.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }

                if ch == '.' && !is_ellipsis && Self::is_abbreviation_dot(text, sent_start, idx) {
                    continue;
                }

                while let Some(&(next_idx, next_ch)) = chars.peek() {
                    if matches!(next_ch, '»' | ')' | ']' | '}' | '"') {
                        end_punct_idx = next_idx + next_ch.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }

                if chars
                    .peek()
                    .map_or(true, |&(_, next_ch)| next_ch.is_whitespace())
                {
                    let slice = text[sent_start..end_punct_idx].trim();
                    if !slice.is_empty() {
                        sentences.push(slice);
                    }
                    sent_start = end_punct_idx;
                }
            }
        }

        if sent_start < text.len() {
            let slice = text[sent_start..].trim();
            if !slice.is_empty() {
                sentences.push(slice);
            }
        }
        sentences
    }

    fn is_decimal_dot(text: &str, dot_idx: usize) -> bool {
        let prev_num = text[..dot_idx]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_numeric());
        let next_num = text[dot_idx + 1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_numeric());
        prev_num && next_num
    }

    fn is_abbreviation_dot(text: &str, sent_start: usize, dot_idx: usize) -> bool {
        let last_word = text[sent_start..dot_idx]
            .rsplit(|c: char| c.is_whitespace() || matches!(c, '«' | '(' | '[' | '،'))
            .next()
            .unwrap_or("");
        if last_word.is_empty() {
            return false;
        }
        if PERSIAN_ABBREVIATIONS.contains(&last_word) {
            return true;
        }
        let count = last_word.chars().count();
        (count == 1 && last_word.chars().next().unwrap().is_alphabetic())
            || (last_word.contains('.') && count <= 5)
    }
}
