use crate::detokenizer::Detokenizer;
use crate::stopwords;
use crate::tokenizer::{TokenKind, Tokenizer};
use fst::Map;
use std::collections::{HashMap, HashSet};

static WORD_FREQ_FST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/word_freq.fst"));

const ALPHABET: &[char] = &[
    'آ', 'ا', 'ب', 'پ', 'ت', 'ث', 'ج', 'چ', 'ح', 'خ', 'د', 'ذ', 'ر', 'ز', 'ژ', 'س', 'ش', 'ص', 'ض',
    'ط', 'ظ', 'ع', 'غ', 'ف', 'ق', 'ک', 'گ', 'ل', 'م', 'ن', 'و', 'ه', 'ی', 'ء', 'أ', 'إ', 'ؤ', 'ئ',
    'ۀ',
];

/// Frequency-ranked spelling suggestions backed by the bundled word-frequency FST.
pub struct SpellChecker {
    freq: Map<&'static [u8]>,
}

impl SpellChecker {
    /// Build the checker from the FST bundled at compile time.
    pub fn new() -> Self {
        Self {
            freq: Map::new(WORD_FREQ_FST_BYTES).expect("Corrupted word_freq.fst"),
        }
    }

    /// Whether the word is in the frequency dictionary.
    pub fn contains(&self, word: &str) -> bool {
        self.freq.contains_key(word)
    }

    /// Up to `max` candidates: the known word first, then single-edit neighbours by
    /// descending frequency, ties by text.
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        // The edit set grows with the square of the word; no real word is this long.
        if max == 0 || word.is_empty() || word.chars().count() > 64 {
            return Vec::new();
        }

        let mut out = Vec::new();
        if self.contains(word) {
            out.push(word.to_string());
            if out.len() == max {
                return out;
            }
        }

        let chars: Vec<char> = word.chars().collect();
        let mut scored: Vec<(u64, String)> = Vec::new();
        for candidate in edits(&chars) {
            if candidate == word {
                continue;
            }
            if let Some(freq) = self.freq.get(candidate.as_str()) {
                scored.push((freq, candidate));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        for (_, candidate) in scored {
            if out.len() == max {
                break;
            }
            out.push(candidate);
        }
        out
    }

    /// Replace unknown words of three or more characters with their best suggestion
    /// and detokenize the result.
    pub fn correct(&self, text: &str) -> String {
        let mut out = Vec::new();
        // A repeated misspelling costs the same edit search every time; remember it per call.
        let mut corrections: HashMap<&str, String> = HashMap::new();
        for token in Tokenizer::tokenize(text) {
            let keep = token.kind != TokenKind::Word
                || token.text.chars().count() < 3
                || stopwords::is_stopword(token.text)
                || self.contains(token.text)
                // Edits only add or swap Persian letters, so a token with none of
                // them can never match a dictionary word.
                || !token.text.chars().any(|ch| ALPHABET.contains(&ch));
            if keep {
                out.push(token.text.to_string());
                continue;
            }
            if let Some(best) = corrections.get(token.text) {
                out.push(best.clone());
                continue;
            }
            let best = self
                .suggest(token.text, 1)
                .into_iter()
                .next()
                .unwrap_or_else(|| token.text.to_string());
            corrections.insert(token.text, best.clone());
            out.push(best);
        }
        Detokenizer::detokenize(&out)
    }
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn edits(word: &[char]) -> HashSet<String> {
    let mut out = HashSet::new();
    let n = word.len();

    for i in 0..=n {
        for &letter in ALPHABET {
            let mut chars = word.to_vec();
            chars.insert(i, letter);
            out.insert(chars.into_iter().collect());
        }

        if i == n {
            continue;
        }

        let mut chars = word.to_vec();
        chars.remove(i);
        out.insert(chars.into_iter().collect());

        for &letter in ALPHABET {
            let mut chars = word.to_vec();
            chars[i] = letter;
            out.insert(chars.into_iter().collect());
        }

        if i + 1 < n {
            let mut chars = word.to_vec();
            chars.swap(i, i + 1);
            out.insert(chars.into_iter().collect());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_very_long_word_gets_no_suggestions() {
        assert!(SpellChecker::new().suggest(&"ک".repeat(1000), 5).is_empty());
    }

    #[test]
    fn a_repeated_unknown_word_is_corrected_everywhere() {
        let checker = SpellChecker::new();
        assert_eq!(checker.correct("سسلام بر سسلام"), "سلام بر سلام");
    }

    #[test]
    fn words_without_persian_letters_are_left_alone() {
        let checker = SpellChecker::new();
        // Latin words never reach the edit search: nothing Persian can come out.
        assert_eq!(checker.correct("googel hello"), "googel hello");
        assert_eq!(checker.correct("سسلام googel"), "سلام googel");
    }
}
