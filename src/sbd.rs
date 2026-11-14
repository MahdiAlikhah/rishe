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

/// Rule-based sentence boundary detection.
pub struct SentenceSplitter;

impl SentenceSplitter {
    /// Split on `.`, `؟`, `?`, `!`, `۔` and newlines, ignoring decimal dots and
    /// common abbreviations; slices are trimmed.
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
                    .is_none_or(|&(_, next_ch)| next_ch.is_whitespace())
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
        // The longest abbreviation is three characters and a word longer than five
        // fails the checks below, so only the last six characters can matter.
        let mut start = dot_idx;
        for (count, (idx, ch)) in text[sent_start..dot_idx].char_indices().rev().enumerate() {
            if ch.is_whitespace() || matches!(ch, '«' | '(' | '[' | '،') {
                start = sent_start + idx + ch.len_utf8();
                break;
            }
            if count == 5 {
                return false;
            }
            start = sent_start + idx;
        }
        let last_word = &text[start..dot_idx];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_sentence_punctuation() {
        assert_eq!(
            SentenceSplitter::split("یک. دو! سه؟"),
            ["یک.", "دو!", "سه؟"]
        );
        assert_eq!(SentenceSplitter::split("سلام۔ بعد"), ["سلام۔", "بعد"]);
    }

    #[test]
    fn newlines_always_split() {
        assert_eq!(SentenceSplitter::split("یک\nدو"), ["یک", "دو"]);
        assert_eq!(SentenceSplitter::split("یک\n\nدو"), ["یک", "دو"]);
    }

    #[test]
    fn decimal_dots_do_not_split() {
        assert_eq!(SentenceSplitter::split("عدد ۳.۱۴ است."), ["عدد ۳.۱۴ است."]);
    }

    #[test]
    fn abbreviations_do_not_split() {
        assert_eq!(SentenceSplitter::split("د. احمدی آمد."), ["د. احمدی آمد."]);
        assert_eq!(
            SentenceSplitter::split("سال ۱۴۰۲ ه.ش. بود."),
            ["سال ۱۴۰۲ ه.ش. بود."]
        );
    }

    #[test]
    fn a_dotted_word_of_five_characters_is_not_a_sentence_end() {
        assert_eq!(SentenceSplitter::split("ab.cd. بعد"), ["ab.cd. بعد"]);
    }

    #[test]
    fn a_dotted_word_of_six_characters_is_a_sentence_end() {
        assert_eq!(SentenceSplitter::split("ab.cde. بعد"), ["ab.cde.", "بعد"]);
        assert_eq!(SentenceSplitter::split("abcdef. بعد"), ["abcdef.", "بعد"]);
    }

    #[test]
    fn ellipsis_and_closing_brackets_stay_in_the_sentence() {
        assert_eq!(SentenceSplitter::split("صبر... بعد"), ["صبر...", "بعد"]);
        assert_eq!(SentenceSplitter::split("«سلام.» بعد"), ["«سلام.»", "بعد"]);
    }

    #[test]
    fn a_long_space_free_run_does_not_split() {
        // The pathological input for the old repeated back-scan: no whitespace anywhere.
        let text = "کتاب.خانه.مدرسه.بزرگ.".repeat(5_000);
        assert_eq!(SentenceSplitter::split(&text), [text.trim()]);
    }
}
