use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const ZWNJ: char = '\u{200C}';

/// Per-character keyboard-typo probabilities; a fixed `seed` makes the output reproducible.
#[derive(Debug, Clone, Copy)]
pub struct KeyboardNoiseOptions {
    /// Chance to replace a character with a keyboard neighbour.
    pub substitution_prob: f64,
    /// Chance to insert a neighbouring character before a character.
    pub insertion_prob: f64,
    /// Chance to drop a character.
    pub deletion_prob: f64,
    /// Chance to double a character.
    pub repeat_prob: f64,
    /// Chance to emit the shifted-row form of a character.
    pub shifted_prob: f64,
    /// Also shift letters, not only digits and punctuation.
    pub shift_letters: bool,
    /// RNG seed; `None` seeds from the clock.
    pub seed: Option<u64>,
}

impl Default for KeyboardNoiseOptions {
    fn default() -> Self {
        Self {
            substitution_prob: 0.01,
            insertion_prob: 0.01,
            deletion_prob: 0.01,
            repeat_prob: 0.01,
            shifted_prob: 0.005,
            shift_letters: false,
            seed: None,
        }
    }
}

/// Per-character OCR confusion probabilities; a fixed `seed` makes the output reproducible.
#[derive(Debug, Clone, Copy)]
pub struct OcrNoiseOptions {
    /// Chance to replace a character with a visually similar one.
    pub substitution_prob: f64,
    /// Chance to drop a character.
    pub deletion_prob: f64,
    /// Chance to double a character.
    pub repeat_prob: f64,
    /// RNG seed; `None` seeds from the clock.
    pub seed: Option<u64>,
}

impl Default for OcrNoiseOptions {
    fn default() -> Self {
        Self {
            substitution_prob: 0.02,
            deletion_prob: 0.005,
            repeat_prob: 0.005,
            seed: None,
        }
    }
}

/// ZWNJ and space corruption probabilities; a fixed `seed` makes the output reproducible.
#[derive(Debug, Clone, Copy)]
pub struct WhitespaceNoiseOptions {
    /// Chance to delete a ZWNJ.
    pub zwnj_deletion_prob: f64,
    /// Chance to replace a ZWNJ with a space.
    pub zwnj_to_space_prob: f64,
    /// Chance to delete a space.
    pub space_deletion_prob: f64,
    /// Chance to replace a space with a ZWNJ.
    pub space_to_zwnj_prob: f64,
    /// RNG seed; `None` seeds from the clock.
    pub seed: Option<u64>,
}

impl Default for WhitespaceNoiseOptions {
    fn default() -> Self {
        Self {
            zwnj_deletion_prob: 0.3,
            zwnj_to_space_prob: 0.1,
            space_deletion_prob: 0.1,
            space_to_zwnj_prob: 0.1,
            seed: None,
        }
    }
}

/// Corrupt characters with keyboard-neighbour typos; characters without a neighbour are kept.
pub fn keyboard_noise(text: &str, options: KeyboardNoiseOptions) -> String {
    let mut rng = Rng::new(options.seed.unwrap_or_else(now_seed));
    let mut out = String::with_capacity(text.len());

    for ch in text.chars() {
        let Some(neighbors) = keyboard_neighbors().get(&ch) else {
            out.push(ch);
            continue;
        };

        if rng.chance(options.deletion_prob) {
            continue;
        }
        if rng.chance(options.insertion_prob) && !neighbors.is_empty() {
            out.push(neighbors[rng.range(neighbors.len())]);
            out.push(ch);
            continue;
        }
        if rng.chance(options.repeat_prob) {
            out.push(ch);
            out.push(ch);
            continue;
        }
        if rng.chance(options.substitution_prob) && !neighbors.is_empty() {
            out.push(neighbors[rng.range(neighbors.len())]);
            continue;
        }
        if (options.shift_letters || !ch.is_alphabetic()) && rng.chance(options.shifted_prob) {
            if let Some(shifted) = shifted_char(ch) {
                out.push(shifted);
                continue;
            }
        }
        out.push(ch);
    }

    out
}

/// Replace Persian letters with OCR confusions; other characters are kept.
pub fn ocr_noise(text: &str, options: OcrNoiseOptions) -> String {
    let mut rng = Rng::new(options.seed.unwrap_or_else(now_seed));
    let mut out = String::with_capacity(text.len());

    for ch in text.chars() {
        let Some(confusions) = ocr_confusions(ch) else {
            out.push(ch);
            continue;
        };

        if rng.chance(options.deletion_prob) {
            continue;
        }
        if rng.chance(options.repeat_prob) {
            out.push(ch);
            out.push(ch);
            continue;
        }
        if rng.chance(options.substitution_prob) {
            out.push(confusions[rng.range(confusions.len())]);
            continue;
        }
        out.push(ch);
    }

    out
}

/// Corrupt ZWNJ and space separators as configured; other characters are kept.
pub fn whitespace_noise(text: &str, options: WhitespaceNoiseOptions) -> String {
    let mut rng = Rng::new(options.seed.unwrap_or_else(now_seed));
    let mut out = String::with_capacity(text.len());

    for ch in text.chars() {
        if ch == ZWNJ {
            if rng.chance(options.zwnj_deletion_prob) {
                continue;
            }
            if rng.chance(options.zwnj_to_space_prob) {
                out.push(' ');
                continue;
            }
            out.push(ch);
        } else if ch == ' ' {
            if rng.chance(options.space_deletion_prob) {
                continue;
            }
            if rng.chance(options.space_to_zwnj_prob) {
                out.push(ZWNJ);
                continue;
            }
            out.push(ch);
        } else {
            out.push(ch);
        }
    }

    out
}

const KEYBOARD_ROWS: [&str; 4] = ["1234567890-=", "ضصثقفغعهخحجچپ", "شسیبلاتنمکگ", "ظطزرذدئو./"];

// The shifted row is what `shift` produces on each key, so `9` must give `(` and
// `0` must give `)`. The pair was swapped, and `keyboard_noise("90")` emitted `)(`.
const SHIFTED_ROWS: [&str; 4] = ["!@#$%^&*()_+", "ًٌٍ﷼،؛,][\\}{|", "َُِّۀآـ«»:\"", "ةيژؤأإء<>؟"];

/// Persian digits sit on the same keys as the ASCII ones, in this order.
const PERSIAN_DIGIT_ROW: &str = "۱۲۳۴۵۶۷۸۹۰";

static KEYBOARD_NEIGHBORS: OnceLock<HashMap<char, Vec<char>>> = OnceLock::new();

fn keyboard_neighbors() -> &'static HashMap<char, Vec<char>> {
    KEYBOARD_NEIGHBORS.get_or_init(|| {
        let mut map = HashMap::new();
        for (r, row) in KEYBOARD_ROWS.iter().enumerate() {
            let chars: Vec<char> = row.chars().collect();
            for (i, &ch) in chars.iter().enumerate() {
                let mut neighbors = Vec::new();
                if i > 0 {
                    neighbors.push(chars[i - 1]);
                }
                if i + 1 < chars.len() {
                    neighbors.push(chars[i + 1]);
                }
                for dr in [-1isize, 1] {
                    let nr = r as isize + dr;
                    if nr >= 0 && (nr as usize) < KEYBOARD_ROWS.len() {
                        let adjacent: Vec<char> = KEYBOARD_ROWS[nr as usize].chars().collect();
                        neighbors.push(adjacent[i.min(adjacent.len() - 1)]);
                    }
                }
                neighbors.retain(|&n| n != ch);
                neighbors.sort_unstable();
                neighbors.dedup();
                map.insert(ch, neighbors);
            }
        }

        // Persian digits have no row of their own: they sit on the same keys as
        // the ASCII digits, so they take the same neighbours in their own script.
        // Without this, `keyboard_noise` never touched `۱۲۳` at all.
        let persian: Vec<char> = PERSIAN_DIGIT_ROW.chars().collect();
        let ascii: Vec<char> = KEYBOARD_ROWS[0].chars().take(10).collect();
        for (ascii_digit, persian_digit) in ascii.iter().zip(&persian) {
            let neighbors = map
                .get(ascii_digit)
                .map(|neighbors| {
                    neighbors
                        .iter()
                        .filter_map(|n| {
                            ascii
                                .iter()
                                .position(|c| c == n)
                                .map(|index| persian[index])
                        })
                        .collect::<Vec<char>>()
                })
                .unwrap_or_default();
            map.insert(*persian_digit, neighbors);
        }
        // `ژ` shares a key with `ز`, and has no row entry of its own, so it was
        // the one letter the masker never corrupted.
        map.insert('ژ', vec!['ز']);
        map.entry('ز').or_default().push('ژ');
        map
    })
}

fn shifted_char(ch: char) -> Option<char> {
    // Persian digits shift to the symbol on the same key, like the ASCII digit
    // does; they had no entry at all and so were never shifted.
    let ch = PERSIAN_DIGIT_ROW
        .chars()
        .position(|digit| digit == ch)
        .and_then(|index| KEYBOARD_ROWS[0].chars().nth(index))
        .unwrap_or(ch);
    for (r, row) in KEYBOARD_ROWS.iter().enumerate() {
        if let Some(index) = row.chars().position(|c| c == ch) {
            return SHIFTED_ROWS[r].chars().nth(index);
        }
    }
    None
}

fn ocr_confusions(ch: char) -> Option<&'static [char]> {
    Some(match ch {
        'ب' => &['پ', 'ت', 'ث'],
        'پ' => &['ب', 'ت'],
        'ت' => &['ب', 'پ', 'ث'],
        'ث' => &['ت', 'ش'],
        'ج' => &['چ', 'ح', 'خ'],
        'ح' => &['ج', 'خ'],
        'خ' => &['ح'],
        'س' => &['ش', 'ص'],
        'ش' => &['س', 'ث'],
        'ص' => &['س', 'ض'],
        'ض' => &['ص'],
        'ط' => &['ظ'],
        'ظ' => &['ط'],
        'ق' => &['ف'],
        'ف' => &['ق'],
        'ع' => &['غ'],
        'غ' => &['ع', 'خ', 'ف'],
        'د' => &['ذ'],
        'ر' => &['ز'],
        'و' => &['ن'],
        'ک' => &['گ', 'ك'],
        'گ' => &['ک', 'ك'],
        'ه' => &['ة'],
        'ی' => &['ئ'],
        'ئ' => &['ی'],
        // Persian digits were confused with the letters they resemble, which
        // contradicts the documented "only Persian letters are replaced" and
        // turned `۱۰` into `ل۰` or `۱.`. Digits are left alone.
        _ => return None,
    })
}

fn now_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // `seed.max(1)` silently made `seed=0` identical to `seed=1`. Mixing the
        // seed instead keeps every seed distinct and keeps xorshift off zero.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn chance(&mut self, probability: f64) -> bool {
        self.next_f64() < probability
    }

    fn range(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_noise_is_deterministic_and_bounded() {
        let options = KeyboardNoiseOptions {
            deletion_prob: 1.0,
            substitution_prob: 0.0,
            insertion_prob: 0.0,
            repeat_prob: 0.0,
            shifted_prob: 0.0,
            shift_letters: false,
            seed: Some(42),
        };
        assert_eq!(keyboard_noise("سلام abc", options), " abc");
        assert_eq!(
            keyboard_noise("سلام abc", options),
            keyboard_noise("سلام abc", options)
        );

        let repeat = KeyboardNoiseOptions {
            repeat_prob: 1.0,
            deletion_prob: 0.0,
            ..options
        };
        assert_eq!(keyboard_noise("کتاب", repeat), "ککتتاابب");
    }

    #[test]
    fn keyboard_shift_maps_non_letters_by_default() {
        let options = KeyboardNoiseOptions {
            shifted_prob: 1.0,
            deletion_prob: 0.0,
            substitution_prob: 0.0,
            insertion_prob: 0.0,
            repeat_prob: 0.0,
            shift_letters: false,
            seed: Some(1),
        };
        assert_eq!(keyboard_noise("12-", options), "!@_");
        assert_eq!(keyboard_noise("کتاب", options), "کتاب");
        // The shifted row had `9` and `0` swapped, so this gave `)(`.
        assert_eq!(keyboard_noise("90", options), "()");
        // Persian digits shift like their ASCII neighbours on the same keys.
        assert_eq!(keyboard_noise("۹۰", options), "()");
    }

    #[test]
    fn keyboard_noise_reaches_persian_digits_and_zhe() {
        let options = KeyboardNoiseOptions {
            deletion_prob: 1.0,
            substitution_prob: 0.0,
            insertion_prob: 0.0,
            repeat_prob: 0.0,
            shifted_prob: 0.0,
            shift_letters: false,
            seed: Some(3),
        };
        // Neither had a keyboard entry, so both passed through untouched.
        assert_eq!(keyboard_noise("۱۲۳ ژاله", options), " ");
    }

    #[test]
    fn ocr_noise_leaves_digits_alone() {
        let options = OcrNoiseOptions {
            substitution_prob: 1.0,
            deletion_prob: 0.0,
            repeat_prob: 0.0,
            seed: Some(5),
        };
        // The doc says only Persian letters are confused; `۱۰` used to become
        // `ل۰` or `۱.`.
        assert_eq!(ocr_noise("۱۰ ۲۰", options), "۱۰ ۲۰");
        let output = ocr_noise("بپ", options);
        assert_eq!(output.chars().count(), 2);
        assert!(output.chars().all(|ch| "پتثب".contains(ch)));
    }

    #[test]
    fn different_seeds_give_different_output() {
        let options = KeyboardNoiseOptions {
            deletion_prob: 0.0,
            substitution_prob: 0.3,
            insertion_prob: 0.0,
            repeat_prob: 0.0,
            shifted_prob: 0.0,
            shift_letters: false,
            seed: Some(0),
        };
        // `seed=0` was clamped to `1`, so both seeds produced the same text.
        let zero = keyboard_noise(&"کتاب ".repeat(20), options);
        let one = keyboard_noise(
            &"کتاب ".repeat(20),
            KeyboardNoiseOptions {
                seed: Some(1),
                ..options
            },
        );
        assert_ne!(zero, one);
        assert_eq!(zero, keyboard_noise(&"کتاب ".repeat(20), options));
    }

    #[test]
    fn ocr_noise_respects_operations() {
        let substitution = OcrNoiseOptions {
            substitution_prob: 1.0,
            deletion_prob: 0.0,
            repeat_prob: 0.0,
            seed: Some(7),
        };
        let output = ocr_noise("بپ", substitution);
        assert_eq!(output.chars().count(), 2);
        assert!(output.chars().all(|ch| "پتثب".contains(ch)));
        assert_eq!(ocr_noise("بپ", substitution), ocr_noise("بپ", substitution));

        let delete = OcrNoiseOptions {
            deletion_prob: 1.0,
            substitution_prob: 0.0,
            repeat_prob: 0.0,
            seed: Some(7),
        };
        assert_eq!(ocr_noise("بپ x", delete), " x");
    }

    #[test]
    fn whitespace_noise_respects_operations() {
        let to_zwnj = WhitespaceNoiseOptions {
            zwnj_deletion_prob: 0.0,
            zwnj_to_space_prob: 0.0,
            space_deletion_prob: 0.0,
            space_to_zwnj_prob: 1.0,
            seed: Some(1),
        };
        assert_eq!(whitespace_noise("a b c", to_zwnj), "a\u{200C}b\u{200C}c");

        let delete = WhitespaceNoiseOptions {
            zwnj_deletion_prob: 1.0,
            zwnj_to_space_prob: 0.0,
            space_deletion_prob: 1.0,
            space_to_zwnj_prob: 0.0,
            seed: Some(1),
        };
        assert_eq!(whitespace_noise("a b\u{200C}c", delete), "abc");
    }
}
