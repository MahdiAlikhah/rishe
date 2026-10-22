const ZWNJ: char = '\u{200C}';

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
        let start = index == 0 || !is_persian_word_char(chars[index - 1]);
        let end = index + 1 == chars.len() || !is_persian_word_char(chars[index + 1]);
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
            '\u{062D}' | '\u{0647}' => {
                if end {
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
            '\u{0629}' | '\u{06C0}' => "ҳ",
            _ => {
                out.push(ch);
                continue;
            }
        };
        out.push_str(mapped);
    }
    out
}

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

fn persian_digit(ch: char) -> Option<char> {
    match ch {
        '\u{06F0}'..='\u{06F9}' => char::from_u32(ch as u32 - 0x06F0 + 0x30),
        '\u{0660}'..='\u{0669}' => char::from_u32(ch as u32 - 0x0660 + 0x30),
        _ => None,
    }
}

fn utf8_digit(ch: char) -> Option<char> {
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
