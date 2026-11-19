use crate::morphology::VerbRoots;
use std::collections::HashMap;

pub(crate) const PAST_ENDINGS: [&str; 6] = ["م", "ی", "", "یم", "ید", "ند"];
pub(crate) const PRESENT_ENDINGS: [&str; 6] = ["م", "ی", "د", "یم", "ید", "ند"];
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
            // A separable preverb comes FIRST: `برمی‌گردم`, not `می‌برگردم`. The
            // normalizer folds `بر می‌گردم` to `برمی‌گردم`, so emitting the other
            // order made `conjugate` and `normalize` disagree about the same word.
            let present_prefix = preverb_imperfective(preverb, "می");
            let negative_prefix = preverb_imperfective(preverb, "نمی");
            (
                forms(&present_prefix, present_body, &PRESENT_ENDINGS),
                forms(&negative_prefix, present_body, &PRESENT_ENDINGS),
                forms("", &subjunctive, &PRESENT_ENDINGS),
            )
        };

        Self {
            past: forms("", past, &PAST_ENDINGS),
            negative_past: forms("", &negated(preverb, past_body), &PAST_ENDINGS),
            imperfective_past: forms(
                &preverb_imperfective(preverb, "می"),
                past_body,
                &PAST_ENDINGS,
            ),
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

    /// The fields of the conjugation in a fixed order.
    ///
    /// `into_map` returns a `HashMap`, whose iteration order differs per process
    /// and per call; the Python binding needs one stable order so callers can
    /// print or compare the result. Crate-internal, so the frozen `1.0.0` API
    /// surface does not grow for a serialization detail.
    #[cfg(feature = "python")]
    pub(crate) fn into_pairs(self) -> [(&'static str, Vec<String>); 8] {
        [
            ("past", self.past),
            ("negative_past", self.negative_past),
            ("imperfective_past", self.imperfective_past),
            ("present", self.present),
            ("negative_present", self.negative_present),
            ("subjunctive", self.subjunctive),
            ("future", self.future),
            ("perfect", self.perfect),
        ]
    }
}

fn forms(prefix: &str, stem: &str, endings: &[&str; 6]) -> Vec<String> {
    endings
        .iter()
        .map(|ending| format!("{prefix}{stem}{ending}"))
        .collect()
}

/// The imperfective prefix for a verb, with a separable preverb kept in front.
///
/// The preverb leads, because it keeps its position and the imperfective marker
/// attaches to the stem: `بر` + `می` + `‌` gives `برمی‌گردم`. Without a preverb
/// this is just `می‌`/`نمی‌`.
fn preverb_imperfective(preverb: &str, marker: &str) -> String {
    format!("{preverb}{marker}\u{200C}")
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
