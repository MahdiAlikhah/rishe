const ZWNJ: &str = "\u{200C}";

/// Persian adjective and noun inflection.
pub struct Inflector;

impl Inflector {
    /// Comparative form: `بزرگ` -> `بزرگ‌تر`, with irregular adjectives mapped to their
    /// listed form.
    pub fn comparative(adj: &str) -> String {
        match irregular(adj) {
            Some((comparative, _)) => comparative.to_string(),
            None => join(adj, "تر"),
        }
    }

    /// Superlative form: `بزرگ` -> `بزرگ‌ترین`, with irregular adjectives mapped to their
    /// listed form.
    pub fn superlative(adj: &str) -> String {
        match irregular(adj) {
            Some((_, superlative)) => superlative.to_string(),
            None => join(adj, "ترین"),
        }
    }

    /// Plural with `ها`, joined without ZWNJ after `ا`/`و`/`د`/`ر`/`ز`/`ژ`.
    pub fn plural(noun: &str) -> String {
        join(noun, "ها")
    }

    /// Possessive for person `0..=5` (1st/2nd/3rd singular, then plural), or `None`
    /// when out of range.
    pub fn possessive(noun: &str, person: usize) -> Option<String> {
        let suffixes = ["م", "ت", "ش", "مان", "تان", "شان"];
        suffixes.get(person)?;
        if noun.is_empty() {
            return Some(String::new());
        }

        if noun.ends_with(['ا', 'آ', 'و']) {
            // Vowel-final nouns take a linking ی before the suffix: دنیا -> دنیایم.
            return Some(format!("{noun}ی{}", suffixes[person]));
        }

        if noun.ends_with('ه') {
            if person < 3 {
                let suffix = ["ام", "ات", "اش"][person];
                return Some(format!("{noun}{ZWNJ}{suffix}"));
            }
            return Some(format!("{noun}{ZWNJ}{}", suffixes[person]));
        }

        Some(format!("{noun}{}", suffixes[person]))
    }
}

fn irregular(adj: &str) -> Option<(&'static str, &'static str)> {
    match adj {
        "خوب" => Some(("بهتر", "بهترین")),
        "کم" => Some(("کمتر", "کمترین")),
        "بیش" => Some(("بیشتر", "بیشترین")),
        _ => None,
    }
}

fn join(word: &str, suffix: &str) -> String {
    if word.is_empty() {
        return String::new();
    }
    if word.chars().last().is_some_and(|c| "اأآودذرزژ".contains(c)) {
        format!("{word}{suffix}")
    } else {
        format!("{word}{ZWNJ}{suffix}")
    }
}
