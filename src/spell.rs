use crate::detokenizer::Detokenizer;
use crate::stopwords;
use crate::tokenizer::{TokenKind, Tokenizer};
use fst::Map;
use std::collections::HashSet;

static WORD_FREQ_FST_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/word_freq.fst"));

const ALPHABET: &[char] = &[
    'آ', 'ا', 'ب', 'پ', 'ت', 'ث', 'ج', 'چ', 'ح', 'خ', 'د', 'ذ', 'ر', 'ز', 'ژ', 'س', 'ش', 'ص', 'ض',
    'ط', 'ظ', 'ع', 'غ', 'ف', 'ق', 'ک', 'گ', 'ل', 'م', 'ن', 'و', 'ه', 'ی', 'ء', 'أ', 'إ', 'ؤ', 'ئ',
    'ۀ',
];

pub struct SpellChecker {
    freq: Map<&'static [u8]>,
}

impl SpellChecker {
    pub fn new() -> Self {
        Self {
            freq: Map::new(WORD_FREQ_FST_BYTES).expect("Corrupted word_freq.fst"),
        }
    }

    pub fn contains(&self, word: &str) -> bool {
        self.freq.contains_key(word)
    }

    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        if max == 0 {
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

    pub fn correct(&self, text: &str) -> String {
        let mut out = Vec::new();
        for token in Tokenizer::tokenize(text) {
            let keep = token.kind != TokenKind::Word
                || token.text.chars().count() < 3
                || stopwords::is_stopword(token.text)
                || self.contains(token.text);
            if keep {
                out.push(token.text.to_string());
                continue;
            }
            match self.suggest(token.text, 1).into_iter().next() {
                Some(best) => out.push(best),
                None => out.push(token.text.to_string()),
            }
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
