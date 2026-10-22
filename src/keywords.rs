use crate::sbd::SentenceSplitter;
use crate::stopwords;
use crate::tokenizer::{TokenKind, Tokenizer};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

const MIN_RAKE_WORD_CHARS: usize = 3;
const MIN_TEXTRANK_WORD_CHARS: usize = 2;

pub fn rake(text: &str, top_n: usize, max_length: usize) -> Vec<String> {
    if top_n == 0 {
        return Vec::new();
    }
    let max_length = max_length.max(1);
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

pub fn textrank(text: &str, top_n: usize, max_length: usize, window: usize) -> Vec<String> {
    if top_n == 0 {
        return Vec::new();
    }
    let max_length = max_length.max(1);
    let sentences = textrank_sentences(text);
    let graph = build_graph(&sentences, window);
    let scores = pagerank(&graph);

    let mut candidates: Vec<(String, f64)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for words in &sentences {
        let mut i = 0;
        while i < words.len() {
            if !scores.contains_key(&words[i]) {
                i += 1;
                continue;
            }
            let mut phrase = vec![words[i].as_str()];
            let mut j = i + 1;
            while j < words.len() && j - i < max_length && scores.contains_key(&words[j]) {
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
            i = (i + 1).max(j);
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
            .filter(|word| {
                word.chars().count() >= MIN_TEXTRANK_WORD_CHARS && !stopwords::is_stopword(word)
            })
            .collect();
        if !words.is_empty() {
            sentences.push(words);
        }
    }
    sentences
}

fn build_graph(
    sentences: &[Vec<String>],
    window: usize,
) -> BTreeMap<String, BTreeMap<String, f64>> {
    let mut graph: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for words in sentences {
        for i in 0..words.len() {
            let start = i.saturating_sub(window);
            let end = (i + window + 1).min(words.len());
            for j in start..end {
                if i == j {
                    continue;
                }
                *graph
                    .entry(words[i].clone())
                    .or_default()
                    .entry(words[j].clone())
                    .or_insert(0.0) += 1.0;
                *graph
                    .entry(words[j].clone())
                    .or_default()
                    .entry(words[i].clone())
                    .or_insert(0.0) += 1.0;
            }
        }
    }
    graph
}

fn pagerank(graph: &BTreeMap<String, BTreeMap<String, f64>>) -> HashMap<String, f64> {
    if graph.is_empty() {
        return HashMap::new();
    }
    let damping = 0.85;
    let iterations = 30;
    let initial = 1.0 / graph.len() as f64;
    let mut scores: HashMap<String, f64> =
        graph.keys().map(|node| (node.clone(), initial)).collect();

    for _ in 0..iterations {
        let mut next = HashMap::with_capacity(graph.len());
        for (node, neighbors) in graph {
            let mut rank = 0.0;
            for (neighbor, weight) in neighbors {
                let out_weight: f64 = graph
                    .get(neighbor)
                    .map(|edges| edges.values().sum())
                    .unwrap_or(1.0)
                    .max(1.0);
                rank += (weight / out_weight) * scores.get(neighbor).copied().unwrap_or(0.0);
            }
            next.insert(node.clone(), (1.0 - damping) + damping * rank);
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
    fn empty_and_zero_top_n() {
        assert!(rake("", 5, 3).is_empty());
        assert!(textrank("", 5, 3, 2).is_empty());
        assert!(rake(TEXT, 0, 3).is_empty());
        assert!(textrank(TEXT, 0, 3, 2).is_empty());
    }
}
