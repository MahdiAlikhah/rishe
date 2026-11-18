/// Joins tokens back into text, spacing punctuation the way Persian is written.
pub struct Detokenizer;

impl Detokenizer {
    /// Join with single spaces, but no space before closing punctuation and none after
    /// opening brackets; empty tokens are skipped.
    ///
    /// ASCII `,`/`;` close like `،`. The ASCII quote is ambiguous, so it toggles:
    /// the odd occurrence opens and the even one closes.
    #[inline]
    pub fn detokenize<S: AsRef<str>>(tokens: &[S]) -> String {
        let cap: usize = tokens.iter().map(|t| t.as_ref().len() + 1).sum();
        let mut out = String::with_capacity(cap);
        let mut no_space_before_next = true;
        let mut quotes = 0usize;

        for tok_ref in tokens {
            let tok = tok_ref.as_ref();
            if tok.is_empty() {
                continue;
            }
            let quote = tok == "\"";
            if quote {
                quotes += 1;
            }
            let quote_closes = quote && quotes.is_multiple_of(2);
            let opens =
                matches!(tok, "«" | "(" | "[" | "{" | "\u{200C}") || (quote && !quote_closes);
            let closes = quote_closes
                || (!quote
                    && matches!(
                        tok,
                        "." | "،"
                            | ","
                            | "؛"
                            | ";"
                            | ":"
                            | "؟"
                            | "?"
                            | "!"
                            | ")"
                            | "]"
                            | "}"
                            | "»"
                            | "%"
                            | "٪"
                    ));
            if opens {
                if !no_space_before_next {
                    out.push(' ');
                }
                out.push_str(tok);
                no_space_before_next = true;
            } else if closes || tok.chars().all(|c| matches!(c, '.' | '؟' | '?' | '!')) {
                out.push_str(tok);
                no_space_before_next = false;
            } else {
                if !no_space_before_next {
                    out.push(' ');
                }
                out.push_str(tok);
                no_space_before_next = false;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detokenize(tokens: &[&str]) -> String {
        Detokenizer::detokenize(tokens)
    }

    #[test]
    fn spaces_persian_punctuation_the_way_it_is_written() {
        assert_eq!(detokenize(&["من", "،", "تو"]), "من، تو");
        assert_eq!(
            detokenize(&["او", "گفت", ":", "«", "سلام", "»", "!"]),
            "او گفت: «سلام»!"
        );
        assert_eq!(detokenize(&["۱۲۳", "،", "۴۵۶"]), "۱۲۳، ۴۵۶");
        assert_eq!(detokenize(&["صبر", "...", "بعد"]), "صبر... بعد");
        assert_eq!(detokenize(&["(", "کتاب", ")"]), "(کتاب)");
    }

    #[test]
    fn ascii_punctuation_round_trips_through_the_tokenizer() {
        // `,` and `;` are the ASCII spellings of `،` and `؛`; the quote is
        // ambiguous, so it alternates open/close.
        assert_eq!(detokenize(&["a", ",", "b"]), "a, b");
        assert_eq!(detokenize(&["a", ";", "b"]), "a; b");
        assert_eq!(
            detokenize(&["او", "گفت", "\"", "سلام", "\""]),
            "او گفت \"سلام\""
        );
        assert_eq!(detokenize(&["کتاب", ".", "من", "،", "تو"]), "کتاب. من، تو");
    }

    #[test]
    fn tokens_round_trip_through_tokenize() {
        use crate::tokenizer::Tokenizer;
        for text in [
            "کتاب.",
            "من، تو",
            "او گفت: «سلام»!",
            "۱۲۳، ۴۵۶",
            "می\u{200C}خواهم",
            "کتاب\u{200C}",
            "a, b",
        ] {
            let tokens: Vec<&str> = Tokenizer::tokenize(text).iter().map(|t| t.text).collect();
            assert_eq!(detokenize(&tokens), text, "{text:?}");
        }
    }

    #[test]
    fn empty_tokens_are_skipped() {
        assert_eq!(detokenize(&["", "کتاب", "", ""]), "کتاب");
        assert_eq!(detokenize(&[]), "");
    }
}
