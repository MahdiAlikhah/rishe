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

        if noun.ends_with('ی') {
            // A `ی`-final word takes a ZWNJ before the suffix like `ه` does;
            // without it `ایرانی` + `م` reads as "we are Iranian".
            let suffix = if person < 3 {
                ["ام", "ات", "اش"][person]
            } else {
                suffixes[person]
            };
            return Some(format!("{noun}{ZWNJ}{suffix}"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn possessive_keeps_the_he_and_ye_final_letters_distinct() {
        // `ی`-final words take a ZWNJ like `ه`-final ones; without it `ایرانی` +
        // `م` reads as "we are Iranian".
        assert_eq!(Inflector::possessive("ایرانی", 0), Some("ایرانی‌ام".into()));
        assert_eq!(Inflector::possessive("ایرانی", 2), Some("ایرانی‌اش".into()));
        assert_eq!(Inflector::possessive("خانه", 0), Some("خانه‌ام".into()));
        assert_eq!(Inflector::possessive("خانه", 3), Some("خانه‌مان".into()));
        // Vowel-final nouns take the linking `ی`.
        assert_eq!(Inflector::possessive("دنیا", 0), Some("دنیایم".into()));
        assert_eq!(Inflector::possessive("بازو", 2), Some("بازویش".into()));
        // Consonant-final nouns take the bare suffix.
        assert_eq!(Inflector::possessive("کتاب", 0), Some("کتابم".into()));
        assert_eq!(Inflector::possessive("کتاب", 6), None);
        assert_eq!(Inflector::possessive("", 0), Some(String::new()));
    }

    #[test]
    fn comparisons_and_plurals() {
        assert_eq!(Inflector::comparative("بزرگ"), "بزرگ‌تر");
        assert_eq!(Inflector::superlative("بزرگ"), "بزرگ‌ترین");
        assert_eq!(Inflector::comparative("خوب"), "بهتر");
        assert_eq!(Inflector::superlative("خوب"), "بهترین");
        // `ها` follows `ا`/`و`/`د`/`ر`/`ز`/`ژ` without a ZWNJ.
        assert_eq!(Inflector::plural("کتاب"), "کتاب‌ها");
        assert_eq!(Inflector::plural("دنیا"), "دنیاها");
        assert_eq!(Inflector::plural(""), "");
    }
}
