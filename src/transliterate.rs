const ZWNJ: char = '\u{200C}';

/// Map Persian letters, ZWNJ and digits to Tajik Cyrillic; unmapped characters are kept.
pub fn to_tajik(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &ch) in chars.iter().enumerate() {
        if ch == ZWNJ {
            continue;
        }
        if let Some(digit) = persian_digit(ch) {
            out.push(digit);
            continue;
        }
        // A ZWNJ ends a word for these tests: it joins a stem to a suffix
        // (`خانه‌ها`), so the `ه` before it is stem-final and softens, while the
        // old test counted the ZWNJ as a letter and kept it hard (`хонҳҳо`).
        let start =
            index == 0 || chars[index - 1] == ZWNJ || !is_persian_word_char(chars[index - 1]);
        let end = index + 1 == chars.len()
            || chars[index + 1] == ZWNJ
            || !is_persian_word_char(chars[index + 1]);
        let mapped = match ch {
            '\u{0622}' => "о",
            '\u{0627}' => {
                if start {
                    "а"
                } else {
                    "о"
                }
            }
            '\u{0623}' | '\u{0625}' | '\u{0671}' => "а",
            '\u{0628}' => "б",
            '\u{067E}' => "п",
            '\u{062A}' | '\u{0637}' => "т",
            '\u{062B}' | '\u{0635}' | '\u{0633}' => "с",
            '\u{062C}' => "ҷ",
            '\u{0686}' => "ч",
            // `ح` is a consonant everywhere; only `ه` softens to a vowel at the
            // end of a word (`خانه` → `хона`), so `صبح` keeps its `ҳ`.
            '\u{062D}' => "ҳ",
            // `ه` softens to a vowel at the end of a word (`خانه` → `хона`) but
            // stays a consonant when it follows a vowel letter (`ماه` → `моҳ`).
            '\u{0647}' => {
                if end && !follows_vowel_letter(&chars, index) {
                    "а"
                } else {
                    "ҳ"
                }
            }
            '\u{062E}' => "х",
            '\u{062F}' => "д",
            '\u{0630}' | '\u{0632}' | '\u{0636}' | '\u{0638}' => "з",
            '\u{0631}' => "р",
            '\u{0698}' => "ж",
            '\u{0634}' => "ш",
            '\u{0639}' => "ъ",
            '\u{063A}' => "ғ",
            '\u{0641}' => "ф",
            '\u{0642}' => "қ",
            '\u{06A9}' => "к",
            '\u{06AF}' => "г",
            '\u{0644}' => "л",
            '\u{0645}' => "м",
            '\u{0646}' => "н",
            '\u{0648}' | '\u{0624}' => {
                if end {
                    "у"
                } else {
                    "в"
                }
            }
            '\u{06CC}' | '\u{064A}' | '\u{0649}' => {
                if end {
                    "ӣ"
                } else {
                    "й"
                }
            }
            '\u{0626}' => "й",
            // `ۀ` is `ه` plus the ezafe hamza: `خانۀ من` is `خانه` + ezafe, so the
            // `ه` softens and the ezafe is written `и` (`хонаи мн`).
            '\u{06C0}' => "аи",
            '\u{0629}' => "ҳ",
            _ => {
                out.push(ch);
                continue;
            }
        };
        out.push_str(mapped);
    }
    out
}

/// Map Tajik Cyrillic letters and digits back to Persian; unmapped characters are kept.
pub fn to_persian(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &ch) in chars.iter().enumerate() {
        if let Some(digit) = utf8_digit(ch) {
            out.push(digit);
            continue;
        }
        let start = index == 0 || !is_tajik_word_char(chars[index - 1]);
        let end = index + 1 == chars.len() || !is_tajik_word_char(chars[index + 1]);
        let mapped = match ch {
            'а' | 'А' => {
                if end {
                    "ه"
                } else {
                    "ا"
                }
            }
            'б' | 'Б' => "ب",
            'в' | 'В' => "و",
            'г' | 'Г' => "گ",
            'ғ' | 'Ғ' => "غ",
            'д' | 'Д' => "د",
            'е' | 'Е' | 'ҳ' | 'Ҳ' => "ه",
            'ё' | 'Ё' | 'ю' | 'Ю' => "یو",
            'ж' | 'Ж' => "ژ",
            'з' | 'З' => "ز",
            'и' | 'И' | 'ӣ' | 'Ӣ' | 'й' | 'Й' => "ی",
            'к' | 'К' => "ک",
            'қ' | 'Қ' => "ق",
            'л' | 'Л' => "ل",
            'м' | 'М' => "م",
            'н' | 'Н' => "ن",
            'о' | 'О' => {
                if start {
                    "آ"
                } else {
                    "ا"
                }
            }
            'п' | 'П' => "پ",
            'р' | 'Р' => "ر",
            'с' | 'С' => "س",
            'т' | 'Т' => "ت",
            'у' | 'У' | 'ӯ' | 'Ӯ' => "و",
            'ф' | 'Ф' => "ف",
            'х' | 'Х' => "خ",
            'ч' | 'Ч' => "چ",
            'ҷ' | 'Ҷ' => "ج",
            'ш' | 'Ш' => "ش",
            'ъ' | 'Ъ' => "ع",
            'э' | 'Э' => "ا",
            'я' | 'Я' => "یا",
            _ => {
                out.push(ch);
                continue;
            }
        };
        out.push_str(mapped);
    }
    out
}

pub(crate) fn persian_digit(ch: char) -> Option<char> {
    match ch {
        '\u{06F0}'..='\u{06F9}' => char::from_u32(ch as u32 - 0x06F0 + 0x30),
        '\u{0660}'..='\u{0669}' => char::from_u32(ch as u32 - 0x0660 + 0x30),
        _ => None,
    }
}

pub(crate) fn utf8_digit(ch: char) -> Option<char> {
    if ch.is_ascii_digit() {
        char::from_u32(ch as u32 - 0x30 + 0x06F0)
    } else {
        None
    }
}

fn is_persian_word_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0621}'..='\u{063A}'
            | '\u{0641}'..='\u{064A}'
            | '\u{0660}'..='\u{0669}'
            | '\u{067E}'
            | '\u{0686}'
            | '\u{0698}'
            | '\u{06A9}'
            | '\u{06AF}'
            | '\u{06CC}'
            | '\u{06C0}'
            | '\u{06F0}'..='\u{06F9}'
            | '\u{200C}'
    )
}

/// Whether the nearest non-ZWNJ character before `index` is a vowel letter that
/// keeps a following `ه` consonantal: `خواه` → `хоҳ`, but `خانه` → `хона`.
fn follows_vowel_letter(chars: &[char], index: usize) -> bool {
    matches!(
        previous_word_char(chars, index),
        Some('\u{0627}' | '\u{0622}' | '\u{0648}' | '\u{06CC}' | '\u{064A}' | '\u{0649}')
    )
}

/// Nearest non-ZWNJ character before `index`, if any.
fn previous_word_char(chars: &[char], index: usize) -> Option<char> {
    chars[..index].iter().rev().copied().find(|&ch| ch != ZWNJ)
}

fn is_tajik_word_char(ch: char) -> bool {
    ch.is_alphabetic() || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_persian_to_tajik() {
        assert_eq!(to_tajik("آب"), "об");
        assert_eq!(to_tajik("خانه"), "хона");
        assert_eq!(to_tajik("کتاب‌ها"), "ктобҳо");
        assert_eq!(to_tajik("ایران"), "айрон");
        assert_eq!(to_tajik("سال ۱۴۰۴"), "сол 1404");
        assert_eq!(to_tajik("سلام!"), "слом!");
        // `ح` stays a consonant at a word end; `ه` still softens to a vowel.
        assert_eq!(to_tajik("صبح"), "сбҳ");
        assert_eq!(to_tajik("خانه"), "хона");
    }

    #[test]
    fn zwnj_ends_a_word_for_the_softening_rules() {
        // The ZWNJ was counted as a letter, so a stem-final `ه` stayed hard
        // (`خانه‌ها` -> `хонҳҳо`) and `ۀ` came out as a bare `ҳ`.
        assert_eq!(to_tajik("خانه‌ها"), "хонаҳо");
        assert_eq!(to_tajik("خانۀ من"), "хонаи мн");
    }

    #[test]
    fn a_he_after_a_vowel_letter_stays_a_consonant() {
        // `ماه` has no ZWNJ and no ezafe; softening every final `ه` gave `моа`.
        assert_eq!(to_tajik("ماه"), "моҳ");
        assert_eq!(to_tajik("خان"), "хон");
        // After a consonant the `ه` is the final vowel, as before.
        assert_eq!(to_tajik("ره"), "ра");
    }

    #[test]
    fn maps_tajik_to_persian() {
        assert_eq!(to_persian("об"), "آب");
        assert_eq!(to_persian("хона"), "خانه");
        assert_eq!(to_persian("сол"), "سال");
        assert_eq!(to_persian("сол 1404"), "سال ۱۴۰۴");
        assert_eq!(to_persian("ду"), "دو");
    }

    #[test]
    fn preserves_unmapped_characters() {
        assert_eq!(to_tajik("سلام abc!"), "слом abc!");
        assert_eq!(to_persian("привет"), "پریوهت");
    }
}
