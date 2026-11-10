use std::collections::HashSet;
use std::sync::OnceLock;

const STOPWORDS_TEXT: &str = include_str!("../data/stopwords.txt");

fn set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        STOPWORDS_TEXT
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    })
}

/// Whether the word is in the bundled stopword list.
pub fn is_stopword(word: &str) -> bool {
    set().contains(word)
}

/// Drop stopword tokens, keeping the order of the rest.
pub fn remove_stopwords(tokens: Vec<String>) -> Vec<String> {
    tokens
        .into_iter()
        .filter(|token| !is_stopword(token))
        .collect()
}

/// Every stopword, in unspecified order.
pub fn stopwords() -> Vec<String> {
    set().iter().map(|word| (*word).to_string()).collect()
}
