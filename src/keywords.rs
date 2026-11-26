use crate::sbd::SentenceSplitter;
use crate::stopwords;
use crate::tokenizer::{TokenKind, Tokenizer};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

const MIN_RAKE_WORD_CHARS: usize = 3;
const MIN_TEXTRANK_WORD_CHARS: usize = 2;

/// Whether a token can be part of a TextRank phrase.
fn is_keyword_word(word: &str) -> bool {
    word.chars().count() >= MIN_TEXTRANK_WORD_CHARS && !stopwords::is_stopword(word)
}

/// Top `top_n` RAKE phrases of at most `max_length` words, ordered by word degree score.
///
/// A `top_n` of 0 asks for no phrases and a `max_length` of 0 allows no words, so
/// both return nothing. `max_length` used to be clamped to 1, which answered a
/// request for "at most 0 words" with one-word phrases.
pub fn rake(text: &str, top_n: usize, max_length: usize) -> Vec<String> {
    if top_n == 0 || max_length == 0 {
        return Vec::new();
    }
    let phrases = rake_phrases(text);

    let mut frequency: HashMap<&str, usize> = HashMap::new();
    let mut degree: HashMap<&str, usize> = HashMap::new();
    for phrase in &phrases {
        for word in phrase {
            *frequency.entry(word.as_str()).or_default() += 1;
            *degree.entry(word.as_str()).or_default() += phrase.len();
        }
    }
    let scores: HashMap<&str, f64> = frequency
        .iter()
        .map(|(word, count)| {
            (
                *word,
                degree.get(word).copied().unwrap_or(0) as f64 / *count as f64,
            )
        })
        .collect();

    let mut candidates: Vec<(String, f64)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for phrase in &phrases {
        for n in 1..=max_length.min(phrase.len()) {
            for window in phrase.windows(n) {
                let candidate = window.join(" ");
                let score: f64 = window
                    .iter()
                    .map(|word| scores.get(word.as_str()).copied().unwrap_or(0.0))
                    .sum();
                if let Some(&index) = positions.get(&candidate) {
                    candidates[index].1 = score;
                } else {
                    positions.insert(candidate.clone(), candidates.len());
                    candidates.push((candidate, score));
                }
            }
        }
    }

    candidates.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
    candidates
        .into_iter()
        .take(top_n)
        .map(|(candidate, _)| candidate)
        .collect()
}

fn rake_phrases(text: &str) -> Vec<Vec<String>> {
    let mut phrases = Vec::new();
    for sentence in SentenceSplitter::split(text) {
        let mut current = Vec::new();
        for token in Tokenizer::tokenize(sentence) {
            let boundary = token.kind != TokenKind::Word
                || token.text.chars().count() < MIN_RAKE_WORD_CHARS
                || stopwords::is_stopword(token.text);
            if boundary {
                if !current.is_empty() {
                    phrases.push(std::mem::take(&mut current));
                }
            } else {
                current.push(token.text.to_string());
            }
        }
        if !current.is_empty() {
            phrases.push(current);
        }
    }
    phrases
}

/// Top `top_n` TextRank phrases of at most `max_length` words over a
/// `window`-word co-occurrence graph. A 0 for either means no phrases.
pub fn textrank(text: &str, top_n: usize, max_length: usize, window: usize) -> Vec<String> {
    if top_n == 0 || max_length == 0 {
        return Vec::new();
    }
    let sentences = textrank_sentences(text);
    let graph = build_graph(&sentences, window);
    let scores = pagerank(&graph);

    let mut candidates: Vec<(String, f64)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    // Phrases come from the sentence as written: `stopwords::is_stopword` words
    // are boundaries, not deletions, so two words either side of one never join.
    for sentence in SentenceSplitter::split(text) {
        let words: Vec<String> = Tokenizer::tokenize(sentence)
            .into_iter()
            .filter(|token| token.kind == TokenKind::Word)
            .map(|token| token.text.to_string())
            .collect();
        let mut i = 0;
        while i < words.len() {
            if !is_keyword_word(&words[i]) || !scores.contains_key(words[i].as_str()) {
                i += 1;
                continue;
            }
            let mut phrase = vec![words[i].as_str()];
            let mut j = i + 1;
            while j < words.len()
                && phrase.len() < max_length
                && is_keyword_word(&words[j])
                && scores.contains_key(words[j].as_str())
            {
                phrase.push(words[j].as_str());
                j += 1;
            }
            let candidate = phrase.join(" ");
            let average = phrase
                .iter()
                .map(|word| scores.get(*word).copied().unwrap_or(0.0))
                .sum::<f64>()
                / phrase.len() as f64;
            let score = average * (1.0 + (phrase.len() as f64).ln());
            if let Some(&index) = positions.get(&candidate) {
                candidates[index].1 = candidates[index].1.max(score);
            } else {
                positions.insert(candidate.clone(), candidates.len());
                candidates.push((candidate, score));
            }
            i += 1;
        }
    }

    candidates.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
    candidates
        .into_iter()
        .take(top_n)
        .map(|(candidate, _)| candidate)
        .collect()
}

fn textrank_sentences(text: &str) -> Vec<Vec<String>> {
    let mut sentences = Vec::new();
    for sentence in SentenceSplitter::split(text) {
        let words: Vec<String> = Tokenizer::tokenize(sentence)
            .into_iter()
            .filter(|token| token.kind == TokenKind::Word)
            .map(|token| token.text.to_string())
            .filter(|word| is_keyword_word(word))
            .collect();
        if !words.is_empty() {
            sentences.push(words);
        }
    }
    sentences
}

/// The graph never outlives `sentences`, so its keys borrow the words and a
/// co-occurrence pair costs no allocation.
fn build_graph<'a>(
    sentences: &'a [Vec<String>],
    window: usize,
) -> BTreeMap<&'a str, BTreeMap<&'a str, f64>> {
    let mut graph: BTreeMap<&'a str, BTreeMap<&'a str, f64>> = BTreeMap::new();
    for words in sentences {
        for i in 0..words.len() {
            // Every word is a node, even with no neighbour in the window: a word
            // only entered the graph by pairing, so a content word alone in its
            // sentence was never a node, and the candidate loop then rejected it
            // as unknown. `textrank("کتاب خوب است")` returned nothing while
            // `rake` on the same text returned `کتاب`.
            graph.entry(words[i].as_str()).or_default();
            let start = i.saturating_sub(window);
            let end = i.saturating_add(window).saturating_add(1).min(words.len());
            for j in start..end {
                if i == j {
                    continue;
                }
                let (first, second) = (words[i].as_str(), words[j].as_str());
                *graph.entry(first).or_default().entry(second).or_insert(0.0) += 1.0;
                *graph.entry(second).or_default().entry(first).or_insert(0.0) += 1.0;
            }
        }
    }
    graph
}

/// PageRank over the co-occurrence graph. The scores borrow the graph's keys, so
/// the 30 passes copy no words; iterating the `BTreeMap` keeps the summation
/// order (and so the result) identical to the owned-key version.
fn pagerank<'a>(graph: &'a BTreeMap<&'a str, BTreeMap<&'a str, f64>>) -> HashMap<&'a str, f64> {
    if graph.is_empty() {
        return HashMap::new();
    }
    let damping = 0.85;
    let iterations = 30;
    let initial = 1.0 / graph.len() as f64;
    let mut scores: HashMap<&str, f64> = graph.keys().map(|node| (*node, initial)).collect();
    // Link totals are fixed, so sum them once instead of once per pass per link.
    let out_weights: HashMap<&str, f64> = graph
        .iter()
        .map(|(node, edges)| (*node, edges.values().sum::<f64>().max(1.0)))
        .collect();

    for _ in 0..iterations {
        let mut next = HashMap::with_capacity(graph.len());
        for (node, neighbors) in graph {
            let mut rank = 0.0;
            for (neighbor, weight) in neighbors {
                let out_weight = out_weights.get(*neighbor).copied().unwrap_or(1.0);
                rank += (weight / out_weight) * scores.get(*neighbor).copied().unwrap_or(0.0);
            }
            next.insert(*node, (1.0 - damping) + damping * rank);
        }
        scores = next;
    }
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "پردازش زبان طبیعی شاخه‌ای از هوش مصنوعی است. \
                        یادگیری ماشین زیرشاخه‌ای از هوش مصنوعی است.";

    #[test]
    fn rake_extracts_phrases_without_stopwords() {
        let keywords = rake(TEXT, 5, 3);
        assert!(!keywords.is_empty());
        assert!(keywords.iter().any(|kw| kw == "پردازش زبان طبیعی"));
        assert!(keywords.iter().all(|kw| kw != "از" && kw != "است"));
        assert_eq!(keywords, rake(TEXT, 5, 3));
    }

    #[test]
    fn textrank_extracts_phrases_without_stopwords() {
        let keywords = textrank(TEXT, 5, 3, 2);
        assert!(!keywords.is_empty());
        assert!(keywords.iter().any(|kw| kw == "هوش مصنوعی"));
        assert!(keywords.iter().all(|kw| kw != "از" && kw != "است"));
        assert_eq!(keywords, textrank(TEXT, 5, 3, 2));
    }

    #[test]
    fn pagerank_ranks_the_hub_highest() {
        // a-b, b-c. Hand-computed with damping 0.85 over 30 passes.
        let mut graph: BTreeMap<&str, BTreeMap<&str, f64>> = BTreeMap::new();
        for (from, to) in [("a", "b"), ("b", "a"), ("b", "c"), ("c", "b")] {
            graph.entry(from).or_default().insert(to, 1.0);
        }
        let scores = pagerank(&graph);
        assert!((scores["a"] - 0.7669361095464207).abs() < 1e-12);
        assert!((scores["b"] - 1.4508662617175787).abs() < 1e-12);
    }

    #[test]
    fn textrank_phrases_come_from_adjacent_words() {
        // A stopword is a phrase boundary, not a deletion: `کتاب` and `خانه` are
        // not neighbours, and the old fixed-window scan joined them anyway.
        let phrases = textrank("کتاب خوب بود و خانه بزرگ", 5, 3, 2);
        assert!(
            !phrases.iter().any(|phrase| phrase == "کتاب خانه"),
            "{phrases:?}"
        );
        // Both halves of the real phrase must be candidates.
        let phrases = textrank("ایران هوش مصنوعی", 6, 2, 2);
        assert!(
            phrases.iter().any(|phrase| phrase == "هوش مصنوعی"),
            "{phrases:?}"
        );
        assert!(phrases.iter().all(|phrase| phrase.split(' ').count() <= 2));
    }

    #[test]
    fn empty_and_zero_top_n() {
        assert!(rake("", 5, 3).is_empty());
        assert!(textrank("", 5, 3, 2).is_empty());
        assert!(rake(TEXT, 0, 3).is_empty());
        assert!(textrank(TEXT, 0, 3, 2).is_empty());
    }

    #[test]
    fn a_zero_max_length_asks_for_no_words() {
        // It was clamped to 1, so "phrases of at most 0 words" answered with
        // one-word phrases — a wrong answer rather than none.
        assert!(rake(TEXT, 5, 0).is_empty());
        assert!(textrank(TEXT, 5, 0, 2).is_empty());
    }
}
