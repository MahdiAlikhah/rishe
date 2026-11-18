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
        let preverb = roots.preverb.as_str();
        // A separable preverb keeps its place: `بر` + `گشت` negates to `برنگشتم`,
        // and `می`/`نمی` go between preverb and stem.
        let past_body = past.strip_prefix(preverb).unwrap_or(past);
        let present_body = present.strip_prefix(preverb).unwrap_or(present);

        // No present stem in the source means no present paradigm; emitting the
        // endings alone (`م`, `ی`, `د`) was worse than emitting nothing.
        let (present, negative_present, subjunctive) = if present.is_empty() {
            (Vec::new(), Vec::new(), Vec::new())
        } else {
            // `باش` is the irregular subjunctive of `بود` and takes no `ب`; a
            // separable preverb blocks the marker as well, giving `برگردم`
            // rather than `ببرگردم`.
            let subjunctive = if present == "باش" || !preverb.is_empty() {
                present.to_string()
            } else {
                prefixed_stem(present, "ب")
            };
            (
                forms(
                    &format!("می\u{200C}{preverb}"),
                    present_body,
                    &PRESENT_ENDINGS,
                ),
                forms(
                    &format!("نمی\u{200C}{preverb}"),
                    present_body,
                    &PRESENT_ENDINGS,
                ),
                forms("", &subjunctive, &PRESENT_ENDINGS),
            )
        };

        Self {
            past: forms("", past, &PAST_ENDINGS),
            negative_past: forms("", &negated(preverb, past_body), &PAST_ENDINGS),
            imperfective_past: forms(&format!("می\u{200C}{preverb}"), past_body, &PAST_ENDINGS),
            present,
            negative_present,
            subjunctive,
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

/// Negate a stem, keeping a separable preverb where it is: `بر` + `گشت` gives
/// `برنگشتم`, while a bare `آ`/`ا` stem takes the `ی` glide (`نمی‌آیم`).
fn negated(preverb: &str, stem: &str) -> String {
    format!("{preverb}{}", prefixed_stem(stem, "ن"))
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
