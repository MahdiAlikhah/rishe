use crate::morphology::MorphologyEngine;

const EZAFE_PREPS: &[&str] = &[
    "روی",
    "زیر",
    "بالای",
    "پایین",
    "کنار",
    "پشت",
    "جلوی",
    "داخل",
    "توی",
    "نزد",
    "مانند",
    "مثل",
    "شبیه",
    "طبق",
];
const NON_EZAFE_HEADS: &[&str] = &[
    "از", "به", "با", "در", "بر", "تا", "برای", "را", "و", "یا", "اما", "ولی", "چون", "اگر", "که",
    "این", "آن", "هر", "هیچ",
];
const PRONOUNS: &[&str] = &["من", "تو", "او", "ما", "شما", "آن\u{200C}ها", "ایشان", "وی"];
const NUMERAL_HEADS: &[&str] = &[
    "سال",
    "ماه",
    "روز",
    "ساعت",
    "شماره",
    "صفحه",
    "فصل",
    "بخش",
    "مدل",
    "نسخه",
];

pub struct RuleBasedEzafe;

impl RuleBasedEzafe {
    pub fn predict(tokens: &[String], morph: &MorphologyEngine) -> Vec<bool> {
        let n = tokens.len();
        let mut ezafe = vec![false; n];
        if n < 2 {
            return ezafe;
        }

        for i in 0..(n - 1) {
            let curr = tokens[i].as_str();
            let next = tokens[i + 1].as_str();

            if curr.chars().all(|c| !c.is_alphabetic())
                || next.chars().all(|c| !c.is_alphabetic() && !c.is_numeric())
            {
                continue;
            }
            if NON_EZAFE_HEADS.contains(&curr)
                || NON_EZAFE_HEADS.contains(&next)
                || PRONOUNS.contains(&curr)
            {
                continue;
            }
            if matches!(next, "است" | "نیست" | "بود" | "شد")
                || next.starts_with("می\u{200C}")
                || next.starts_with("نمی\u{200C}")
            {
                continue;
            }
            if !morph.is_known_noun(curr)
                && (curr.ends_with('ش')
                    || curr.ends_with('م')
                    || curr.ends_with('ت')
                    || curr.ends_with("مان"))
            {
                continue;
            }
            if EZAFE_PREPS.contains(&curr) {
                ezafe[i] = true;
                continue;
            }
            if next.chars().next().is_some_and(|c| c.is_numeric()) && NUMERAL_HEADS.contains(&curr)
            {
                ezafe[i] = true;
                continue;
            }
            if i + 2 < n
                && (tokens[i + 2].starts_with("می\u{200C}")
                    || matches!(tokens[i + 2].as_str(), "کرد" | "شد"))
            {
                continue;
            }
            if morph.is_known_noun(curr) || curr.ends_with("\u{200C}های") {
                ezafe[i] = true;
            }
        }
        ezafe
    }
}
