use crate::morphology::VerbRoots;
use std::collections::HashMap;

const PAST_ENDINGS: [&str; 6] = ["م", "ی", "", "یم", "ید", "ند"];
const PRESENT_ENDINGS: [&str; 6] = ["م", "ی", "د", "یم", "ید", "ند"];
const PERFECT_ENDINGS: [&str; 6] = [
    "\u{200C}ام",
    "\u{200C}ای",
    " است",
    "\u{200C}ایم",
    "\u{200C}اید",
    "\u{200C}اند",
];
const FUTURE_AUX: [&str; 6] = ["خواهم", "خواهی", "خواهد", "خواهیم", "خواهید", "خواهند"];

#[derive(Debug, Clone, Default)]
pub struct Conjugation {
    pub past: Vec<String>,
    pub negative_past: Vec<String>,
    pub imperfective_past: Vec<String>,
    pub present: Vec<String>,
    pub negative_present: Vec<String>,
    pub subjunctive: Vec<String>,
    pub future: Vec<String>,
    pub perfect: Vec<String>,
}

impl Conjugation {
    pub fn from_roots(roots: &VerbRoots) -> Self {
        let past = roots.past.as_str();
        let present = roots.present.as_str();
        let negative_past = negative_past_stem(past);
        let subjunctive = subjunctive_stem(present);

        Self {
            past: forms("", past, &PAST_ENDINGS),
            negative_past: forms("", &negative_past, &PAST_ENDINGS),
            imperfective_past: forms("می\u{200C}", past, &PAST_ENDINGS),
            present: forms("می\u{200C}", present, &PRESENT_ENDINGS),
            negative_present: forms("نمی\u{200C}", present, &PRESENT_ENDINGS),
            subjunctive: forms("", &subjunctive, &PRESENT_ENDINGS),
            future: FUTURE_AUX.map(|aux| format!("{aux} {past}")).to_vec(),
            perfect: PERFECT_ENDINGS
                .map(|ending| format!("{past}ه{ending}"))
                .to_vec(),
        }
    }

    pub fn into_map(self) -> HashMap<String, Vec<String>> {
        HashMap::from([
            ("past".to_string(), self.past),
            ("negative_past".to_string(), self.negative_past),
            ("imperfective_past".to_string(), self.imperfective_past),
            ("present".to_string(), self.present),
            ("negative_present".to_string(), self.negative_present),
            ("subjunctive".to_string(), self.subjunctive),
            ("future".to_string(), self.future),
            ("perfect".to_string(), self.perfect),
        ])
    }
}

fn forms(prefix: &str, stem: &str, endings: &[&str; 6]) -> Vec<String> {
    endings
        .iter()
        .map(|ending| format!("{prefix}{stem}{ending}"))
        .collect()
}

fn negative_past_stem(past: &str) -> String {
    if let Some(rest) = past.strip_prefix('آ') {
        format!("نیا{rest}")
    } else if let Some(rest) = past.strip_prefix('ا') {
        format!("نی{rest}")
    } else {
        format!("ن{past}")
    }
}

fn subjunctive_stem(present: &str) -> String {
    if present == "باش" {
        return present.to_string();
    }
    if let Some(rest) = present.strip_prefix('آ') {
        format!("بیا{rest}")
    } else if let Some(rest) = present.strip_prefix('ا') {
        format!("بی{rest}")
    } else {
        format!("ب{present}")
    }
}
