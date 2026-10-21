#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Word,
    Number,
    Punctuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

pub struct Tokenizer;

impl Tokenizer {
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
                    if next_ch.is_alphanumeric() {
                        end_idx = next_idx + next_ch.len_utf8();
                        chars.next();
                    } else if next_ch == '\u{200C}' {
                        chars.next();
                        if let Some(&(_, after_zwnj)) = chars.peek() {
                            if after_zwnj.is_alphanumeric() {
                                end_idx = next_idx + next_ch.len_utf8();
                                continue;
                            }
                        }
                        break;
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
                    if next_ch.is_numeric() {
                        end_idx = next_idx + next_ch.len_utf8();
                        chars.next();
                    } else if matches!(next_ch, '.' | ',' | '٫') {
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
