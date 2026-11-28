/// Lexical class of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Letter-initial token; may contain a ZWNJ.
    Word,
    /// Digit sequence, optionally with decimal separators.
    Number,
    /// Any other single character.
    Punctuation,
}

/// A token with its byte offsets into the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    /// Token text.
    pub text: &'a str,
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset one past the last character.
    pub end: usize,
    /// Lexical class.
    pub kind: TokenKind,
}

/// Unicode combining marks, which belong to the word before them.
///
/// `std` exposes no general-category access, so this lists the blocks that can
/// follow a Persian or Latin letter in real text. Without it a mark ended the
/// word: `tokenize("cafe\u{0301}")` gave `["cafe", "\u{0301}"]`, and decomposed
/// text from PDFs and macOS hits this constantly.
fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F   // Combining Diacritical Marks
            | 0x0483..=0x0489
            | 0x0591..=0x05C7 // Hebrew niqqud
            | 0x0610..=0x061A
            | 0x064B..=0x065F // Arabic harakat
            | 0x0670
            | 0x06D6..=0x06DC
            | 0x06DF..=0x06E4
            | 0x06E7..=0x06E8
            | 0x06EA..=0x06ED
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

/// Byte-offset tokenizer.
pub struct Tokenizer;

impl Tokenizer {
    /// Split into words, numbers and punctuation with byte spans; whitespace is skipped.
    #[inline]
    pub fn tokenize<'a>(input: &'a str) -> Vec<Token<'a>> {
        let mut tokens = Vec::with_capacity(input.len() / 6 + 1);
        let mut chars = input.char_indices().peekable();

        while let Some(&(start_idx, ch)) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
                continue;
            }

            if ch.is_alphabetic() {
                let mut end_idx = start_idx + ch.len_utf8();
                chars.next();

                while let Some(&(next_idx, next_ch)) = chars.peek() {
                    // A mark continues the word rather than ending it.
                    if next_ch.is_alphanumeric() || is_combining_mark(next_ch) {
                        end_idx = next_idx + next_ch.len_utf8();
                        chars.next();
                    } else if next_ch == '\u{200C}' {
                        // A ZWNJ belongs to the word on both sides of it, and a
                        // trailing one stays on its word: dropping it made
                        // `tokenize` -> `detokenize` lose the character.
                        while let Some(&(z_idx, z_ch)) = chars.peek() {
                            if z_ch != '\u{200C}' {
                                break;
                            }
                            end_idx = z_idx + z_ch.len_utf8();
                            chars.next();
                        }
                        if !chars
                            .peek()
                            .is_some_and(|&(_, after)| after.is_alphanumeric())
                        {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                tokens.push(Token {
                    text: &input[start_idx..end_idx],
                    start: start_idx,
                    end: end_idx,
                    kind: TokenKind::Word,
                });
            } else if ch.is_numeric() {
                let mut end_idx = start_idx + ch.len_utf8();
                chars.next();

                while let Some(&(next_idx, next_ch)) = chars.peek() {
                    if next_ch.is_numeric() || is_combining_mark(next_ch) {
                        end_idx = next_idx + next_ch.len_utf8();
                        chars.next();
                    } else if matches!(next_ch, '.' | ',' | '\u{066B}' | '\u{066C}') {
                        let mut lookahead = chars.clone();
                        lookahead.next();
                        if lookahead.peek().is_some_and(|&(_, c)| c.is_numeric()) {
                            end_idx = next_idx + next_ch.len_utf8();
                            chars.next();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                tokens.push(Token {
                    text: &input[start_idx..end_idx],
                    start: start_idx,
                    end: end_idx,
                    kind: TokenKind::Number,
                });
            } else {
                let end_idx = start_idx + ch.len_utf8();
                chars.next();
                tokens.push(Token {
                    text: &input[start_idx..end_idx],
                    start: start_idx,
                    end: end_idx,
                    kind: TokenKind::Punctuation,
                });
            }
        }

        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(input: &str) -> Vec<&str> {
        Tokenizer::tokenize(input)
            .into_iter()
            .map(|token| token.text)
            .collect()
    }

    #[test]
    fn keeps_a_zwnj_on_its_word() {
        // Dropping the ZWNJ made `tokenize` -> `detokenize` lossy.
        assert_eq!(texts("کتاب\u{200C}"), ["کتاب\u{200C}"]);
        assert_eq!(texts("\u{200C}کتاب"), ["\u{200C}", "کتاب"]);
        assert_eq!(
            texts("می\u{200C}\u{200C}خواهم"),
            ["می\u{200C}\u{200C}خواهم"]
        );
        assert_eq!(texts("a\u{200C}b"), ["a\u{200C}b"]);
        assert_eq!(texts("می\u{200C}خواهم"), ["می\u{200C}خواهم"]);
    }

    #[test]
    fn splits_words_numbers_and_punctuation() {
        assert_eq!(texts("سلام دنیا"), ["سلام", "دنیا"]);
        assert_eq!(texts("۱۲۳، ۴۵۶"), ["۱۲۳", "،", "۴۵۶"]);
        assert_eq!(texts("کتاب."), ["کتاب", "."]);
        assert_eq!(texts("۳.۱۴"), ["۳.۱۴"]);
    }

    #[test]
    fn spans_cover_the_original_bytes() {
        let input = "سلام😊 دنیا\u{00A0}۱۲۳ (خوب)";
        for token in Tokenizer::tokenize(input) {
            assert_eq!(&input[token.start..token.end], token.text);
        }
    }

    #[test]
    fn a_combining_mark_stays_in_its_word() {
        // A mark is neither alphabetic nor numeric, so it ended the word and came
        // out as `Punctuation`: decomposed text from a PDF or macOS split every
        // such word in two, and the token count, lemmas and detokenized text all
        // went wrong with it.
        assert_eq!(texts("cafe\u{0301}"), ["cafe\u{0301}"]);
        assert_eq!(texts("a\u{0301}b"), ["a\u{0301}b"]);
        assert_eq!(texts("ک\u{0323}تاب"), ["ک\u{0323}تاب"]);
        assert_eq!(texts("مُحَمَّد"), ["مُحَمَّد"]);
        // A number keeps its marks too.
        assert_eq!(texts("1\u{0301}"), ["1\u{0301}"]);
        // The span still covers the bytes.
        let input = "a\u{0301}b";
        let token = Tokenizer::tokenize(input)[0];
        assert_eq!(&input[token.start..token.end], input);
    }
}
