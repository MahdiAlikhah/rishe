const ZWNJ: &str = "\u{200C}";

pub struct Inflector;

impl Inflector {
    pub fn comparative(adj: &str) -> String {
        match irregular(adj) {
            Some((comparative, _)) => comparative.to_string(),
            None => join(adj, "تر"),
        }
    }

    pub fn superlative(adj: &str) -> String {
        match irregular(adj) {
            Some((_, superlative)) => superlative.to_string(),
            None => join(adj, "ترین"),
        }
    }

    pub fn plural(noun: &str) -> String {
        join(noun, "ها")
    }

    pub fn possessive(noun: &str, person: usize) -> Option<String> {
        let suffixes = ["م", "ت", "ش", "مان", "تان", "شان"];
        suffixes.get(person)?;

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
        "مه" => Some(("مهتر", "مهترین")),
        "به" => Some(("بهتر", "بهترین")),
        _ => None,
    }
}

fn join(word: &str, suffix: &str) -> String {
    if word.chars().last().is_some_and(|c| "اأآودذرزژ".contains(c)) {
        format!("{word}{suffix}")
    } else {
        format!("{word}{ZWNJ}{suffix}")
    }
}
