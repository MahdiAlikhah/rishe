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

/// Verb paradigm for one root; every vector holds the six person forms, 1st/2nd/3rd
/// singular then plural.
#[derive(Debug, Clone, Default)]
pub struct Conjugation {
    /// Simple past: `رفتم`, `رفتی`, ...
    pub past: Vec<String>,
    /// Negated past: `نرفتم`, ...
    pub negative_past: Vec<String>,
    /// Past continuous: `می‌رفتم`, ...
    pub imperfective_past: Vec<String>,
    /// Present: `می‌روم`, ...
    pub present: Vec<String>,
    /// Negated present: `نمی‌روم`, ...
    pub negative_present: Vec<String>,
    /// Subjunctive: `بروم`, ...
    pub subjunctive: Vec<String>,
    /// Future with the `خواه` auxiliary: `خواهم رفت`, ...
    pub future: Vec<String>,
    /// Present perfect: `رفته‌ام`, ...
    pub perfect: Vec<String>,
}

impl Conjugation {
    /// Build the paradigm from a verb's past and present stems.
    pub fn from_roots(roots: &VerbRoots) -> Self {
        let past = roots.past.as_str();
        let present = roots.present.as_str();
        let negative_past = prefixed_stem(past, "ن");
        let subjunctive = if present == "باش" {
            present.to_string()
        } else {
            prefixed_stem(present, "ب")
        };

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

    /// Convert to a field-name to forms map, for serialization.
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

fn prefixed_stem(stem: &str, prefix: &str) -> String {
    if let Some(rest) = stem.strip_prefix('آ') {
        format!("{prefix}یا{rest}")
    } else if let Some(rest) = stem.strip_prefix('ا') {
        format!("{prefix}ی{rest}")
    } else {
        format!("{prefix}{stem}")
    }
}
