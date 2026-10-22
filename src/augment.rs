use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const ZWNJ: char = '\u{200C}';

#[derive(Debug, Clone, Copy)]
pub struct KeyboardNoiseOptions {
    pub substitution_prob: f64,
    pub insertion_prob: f64,
    pub deletion_prob: f64,
    pub repeat_prob: f64,
    pub shifted_prob: f64,
    pub shift_letters: bool,
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

#[derive(Debug, Clone, Copy)]
pub struct OcrNoiseOptions {
    pub substitution_prob: f64,
    pub deletion_prob: f64,
    pub repeat_prob: f64,
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

#[derive(Debug, Clone, Copy)]
pub struct WhitespaceNoiseOptions {
    pub zwnj_deletion_prob: f64,
    pub zwnj_to_space_prob: f64,
    pub space_deletion_prob: f64,
    pub space_to_zwnj_prob: f64,
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

const SHIFTED_ROWS: [&str; 4] = ["!@#$%^&*)(_+", "ًٌٍ﷼،؛,][\\}{|", "َُِّۀآـ«»:\"", "ةيژؤأإء<>؟"];

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
        map
    })
}

fn shifted_char(ch: char) -> Option<char> {
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
        '۱' => &['ل'],
        '۰' => &['.'],
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
        Self(seed.max(1))
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
