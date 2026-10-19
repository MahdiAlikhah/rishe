pub struct Detokenizer;

impl Detokenizer {
    #[inline]
    pub fn detokenize<S: AsRef<str>>(tokens: &[S]) -> String {
        if tokens.is_empty() {
            return String::new();
        }
        let cap: usize = tokens.iter().map(|t| t.as_ref().len() + 1).sum();
        let mut out = String::with_capacity(cap);
        let mut no_space_before_next = true;

        for tok_ref in tokens {
            let tok = tok_ref.as_ref();
            if tok.is_empty() {
                continue;
            }
            if matches!(tok, "«" | "(" | "[" | "{") {
                if !no_space_before_next {
                    out.push(' ');
                }
                out.push_str(tok);
                no_space_before_next = true;
            } else if matches!(
                tok,
                "." | "،" | "؛" | ":" | "؟" | "?" | "!" | ")" | "]" | "}" | "»" | "%" | "٪"
            ) || tok.chars().all(|c| matches!(c, '.' | '؟' | '?' | '!'))
            {
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
