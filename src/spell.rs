use crate::detokenizer::Detokenizer;
use crate::morphology::MorphologyEngine;
use crate::stopwords;
use crate::tokenizer::{TokenKind, Tokenizer};
use fst::Map;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Shared engine for the "the morphology already reads this word" guard.
fn morphology() -> &'static MorphologyEngine {
    static ENGINE: OnceLock<MorphologyEngine> = OnceLock::new();
    ENGINE.get_or_init(MorphologyEngine::new)
}

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
        // Only dictionary hits are stored, so the thousands of candidates that
        // miss the FST cost one probe and no allocation.
        for_each_edit(&chars, |candidate| {
            if candidate == word {
                return;
            }
            if let Some(freq) = self.freq.get(candidate) {
                // A frequency of zero marks a junk row in the bundled list; it is
                // not evidence that the candidate is the word the user meant.
                if freq > 0 && !scored.iter().any(|(_, seen)| seen == candidate) {
                    scored.push((freq, candidate.to_string()));
                }
            }
        });
        // `correct` asks for one suggestion, so one max beats sorting the whole
        // edit neighbourhood; any other count keeps the full order.
        if max - out.len() == 1 {
            if let Some((_, best)) = scored
                .iter()
                .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            {
                out.push(best.clone());
            }
            return out;
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
    ///
    /// A word the morphology can analyse is never rewritten: `زدم` is a form of
    /// `زدن`, and treating it as a misspelling turned it into `دم`.
    pub fn correct(&self, text: &str) -> String {
        let mut out = Vec::new();
        let morphology = morphology();
        // A repeated misspelling costs the same edit search every time; remember it per call.
        let mut corrections: HashMap<&str, String> = HashMap::new();
        for token in Tokenizer::tokenize(text) {
            let keep = token.kind != TokenKind::Word
                || token.text.chars().count() < 3
                || stopwords::is_stopword(token.text)
                || self.contains(token.text)
                || !token.text.chars().any(|ch| ALPHABET.contains(&ch))
                // Last, and only for a real candidate: a word the morphology can
                // read is a form of a known word, not a misspelling.
                || morphology.lemmatize(token.text).as_ref() != token.text;
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

/// Visit every one-edit variant of `word`, reusing one character buffer so the
/// thousands of candidates cost no allocations. The caller filters by dictionary
/// hit, which is why nothing is collected or deduplicated here.
fn for_each_edit(word: &[char], mut visit: impl FnMut(&str)) {
    let n = word.len();
    let mut scratch: Vec<char> = Vec::with_capacity(n + 1);
    let mut buffer = String::with_capacity(n * 4 + 4);
    let visit = &mut visit;

    for i in 0..=n {
        for &letter in ALPHABET {
            scratch.clear();
            scratch.extend_from_slice(&word[..i]);
            scratch.push(letter);
            scratch.extend_from_slice(&word[i..]);
            emit(&scratch, &mut buffer, visit);
        }

        if i == n {
            continue;
        }

        scratch.clear();
        scratch.extend_from_slice(&word[..i]);
        scratch.extend_from_slice(&word[i + 1..]);
        emit(&scratch, &mut buffer, visit);

        for &letter in ALPHABET {
            scratch.clear();
            scratch.extend_from_slice(word);
            scratch[i] = letter;
            emit(&scratch, &mut buffer, visit);
        }

        if i + 1 < n {
            scratch.clear();
            scratch.extend_from_slice(word);
            scratch.swap(i, i + 1);
            emit(&scratch, &mut buffer, visit);
        }
    }
}

fn emit(scratch: &[char], buffer: &mut String, visit: &mut dyn FnMut(&str)) {
    buffer.clear();
    buffer.extend(scratch);
    visit(buffer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_edit_of_the_old_set_is_visited() {
        // The naive rebuild of the previous `edits()`; if the sets agree, the
        // buffer-reusing version lost no candidate and invented none.
        let word: Vec<char> = "کتاب".chars().collect();
        let mut expected = std::collections::BTreeSet::new();
        let n = word.len();
        for i in 0..=n {
            for &letter in ALPHABET {
                let mut chars = word.clone();
                chars.insert(i, letter);
                expected.insert(chars.into_iter().collect::<String>());
            }
            if i == n {
                continue;
            }
            let mut chars = word.clone();
            chars.remove(i);
            expected.insert(chars.into_iter().collect::<String>());
            for &letter in ALPHABET {
                let mut chars = word.clone();
                chars[i] = letter;
                expected.insert(chars.into_iter().collect::<String>());
            }
            if i + 1 < n {
                let mut chars = word.clone();
                chars.swap(i, i + 1);
                expected.insert(chars.into_iter().collect::<String>());
            }
        }

        let mut visited = std::collections::BTreeSet::new();
        for_each_edit(&word, |candidate| {
            visited.insert(candidate.to_string());
        });
        assert_eq!(visited, expected);
    }

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
    fn known_inflections_are_not_corrected() {
        let checker = SpellChecker::new();
        // `زدم` is a form of `زدن`, not a misspelling of `دم`; the morphology
        // guard now keeps every word it can analyse.
        assert_eq!(checker.correct("زنگ زدم"), "زنگ زدم");
        assert_eq!(checker.correct("در را بستم"), "در را بستم");
        // A real misspelling is still fixed.
        assert_eq!(checker.correct("سسلام"), "سلام");
    }

    #[test]
    fn words_without_persian_letters_are_left_alone() {
        let checker = SpellChecker::new();
        // Latin words never reach the edit search: nothing Persian can come out.
        assert_eq!(checker.correct("googel hello"), "googel hello");
        assert_eq!(checker.correct("سسلام googel"), "سلام googel");
    }
}
