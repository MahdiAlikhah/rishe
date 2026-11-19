use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use std::borrow::Cow;

const ZWNJ: char = '\u{200C}';

/// Switches for [`Normalizer`].
#[derive(Debug, Clone)]
pub struct NormalizerConfig {
    /// Map ASCII and Arabic digits to Persian digits.
    pub unify_digits: bool,
    /// Drop Arabic diacritics.
    pub strip_diacritics: bool,
    /// Insert ZWNJ in `می`/`نمی` and in `ها`/`تر` suffixes.
    pub fix_zwnj: bool,
}

impl Default for NormalizerConfig {
    fn default() -> Self {
        Self {
            unify_digits: true,
            strip_diacritics: true,
            fix_zwnj: true,
        }
    }
}

/// Flatten every ezafe spelling of a word-final `ه` to plain `ه`.
///
/// Persian writes the same ezafe three ways — `خانۀ`, `خانه‌ی`, `خانه ی` — and
/// the lexicon proves none of them is a distinct word: 0 of 187,582 nouns (and 0
/// of every other shipped list) contain `ۀ`, and it never appears inside a word.
/// So all three are the word's own `ه` plus a mark, and flattening loses no word
/// identity while making search and lookup work. A raw `خانۀ` is not in the
/// lexicon at all; the flattened `خانه` is.
///
/// This is the input direction only. Writing `ۀ`/`ه‌ی` back is a display choice,
/// which `ya` keeps as an opt-in style converter outside the default chain.
fn flatten_ezafe<'a>(input: &'a str) -> Cow<'a, str> {
    // `ۀ` is handled by `classify_char`; these two spellings survive it.
    if !input.contains("\u{647}\u{200C}\u{6CC}") && !input.contains("\u{647} \u{6CC}") {
        return Cow::Borrowed(input);
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(at) = rest.find('\u{647}') {
        out.push_str(&rest[..at]);
        out.push('\u{647}');
        let after = &rest[at + '\u{647}'.len_utf8()..];
        // The ezafe yā is word-final, so strip it only when a word boundary
        // follows. `خانه‌یی` keeps its `ی` (the yā is followed by another letter),
        // and so does `خانه یا`, where `یا` is a separate word.
        let candidate = after
            .strip_prefix("\u{200C}\u{6CC}")
            .or_else(|| after.strip_prefix(" \u{6CC}"));
        match candidate {
            Some(tail)
                if tail
                    .chars()
                    .next()
                    .is_none_or(|ch| !crate::normalize_ops::is_persian_letter(ch)) =>
            {
                rest = tail;
            }
            _ => rest = after,
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Character-level Persian normalizer.
pub struct Normalizer {
    config: NormalizerConfig,
    affix_matcher: AhoCorasick,
    affix_replacements: Vec<&'static str>,
}

impl Normalizer {
    /// Build a normalizer with the given switches.
    pub fn new(config: NormalizerConfig) -> Self {
        let rules: &[(&str, &str)] = &[
            ("می ", "می\u{200C}"),
            ("نمی ", "نمی\u{200C}"),
            (" هایی", "\u{200C}هایی"),
            (" هایم", "\u{200C}هایم"),
            (" هایت", "\u{200C}هایت"),
            (" هایش", "\u{200C}هایش"),
            (" هایمان", "\u{200C}هایمان"),
            (" هایتان", "\u{200C}هایتان"),
            (" هایشان", "\u{200C}هایشان"),
            (" های", "\u{200C}های"),
            (" ها", "\u{200C}ها"),
            (" ترین", "\u{200C}ترین"),
            (" تر", "\u{200C}تر"),
        ];

        let patterns: Vec<&str> = rules.iter().map(|(pat, _)| *pat).collect();
        let affix_replacements: Vec<&'static str> = rules.iter().map(|(_, rep)| *rep).collect();

        let affix_matcher = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(&patterns)
            .expect("Failed to compile Aho-Corasick ZWNJ automaton");

        Self {
            config,
            affix_matcher,
            affix_replacements,
        }
    }

    /// Normalize characters and, when `fix_zwnj` is set, affix spacing.
    #[inline]
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        // PDF text often carries the decomposed hamza/maddah instead of `أ`/`آ`;
        // composing first is what lets the rest of the pipeline match them.
        if let Some(composed) = compose_decomposed(input) {
            return Cow::Owned(self.normalize(&composed).into_owned());
        }
        let char_norm = self.normalize_chars(input);
        if !self.config.fix_zwnj {
            return char_norm;
        }
        match char_norm {
            Cow::Borrowed(s) => self.fix_affix_zwnj(s),
            Cow::Owned(s) => match self.fix_affix_zwnj(&s) {
                Cow::Borrowed(_) => Cow::Owned(s),
                Cow::Owned(modified) => Cow::Owned(modified),
            },
        }
    }

    /// Apply the character-level rules only, without the affix ZWNJ fixes.
    pub fn normalize_chars<'a>(&self, input: &'a str) -> Cow<'a, str> {
        let mut buf: Option<String> = None;
        let mut last_was_space = false;
        let mut last_was_zwnj = false;

        for (byte_idx, ch) in input.char_indices() {
            match self.classify_char(ch, last_was_space, last_was_zwnj) {
                CharAction::Keep(c) => {
                    if let Some(ref mut b) = buf {
                        b.push(c);
                    }
                    last_was_space = c == ' ';
                    last_was_zwnj = c == ZWNJ;
                }
                CharAction::Replace(c) => {
                    let b = buf.get_or_insert_with(|| {
                        let mut s = String::with_capacity(input.len());
                        s.push_str(&input[..byte_idx]);
                        s
                    });
                    b.push(c);
                    last_was_space = c == ' ';
                    last_was_zwnj = c == ZWNJ;
                }
                CharAction::Expand(replacement) => {
                    let b = buf.get_or_insert_with(|| {
                        let mut s = String::with_capacity(input.len());
                        s.push_str(&input[..byte_idx]);
                        s
                    });
                    // The table's targets used to skip the letter rules, so a
                    // medallion form came out as U+0649, which is not Persian and
                    // made `normalize` non-idempotent. Run them through the same
                    // classifier.
                    for ch in replacement.chars() {
                        match self.classify_char(ch, false, false) {
                            CharAction::Keep(c) | CharAction::Replace(c) => b.push(c),
                            CharAction::Expand(inner) => b.push_str(inner),
                            CharAction::Skip => {}
                        }
                    }
                    last_was_space = replacement.ends_with(' ');
                    last_was_zwnj = replacement.ends_with(ZWNJ);
                }
                CharAction::Skip => {
                    buf.get_or_insert_with(|| {
                        let mut s = String::with_capacity(input.len());
                        s.push_str(&input[..byte_idx]);
                        s
                    });
                }
            }
        }

        match buf {
            Some(s) => Cow::Owned(flatten_ezafe(&s).into_owned()),
            None => flatten_ezafe(input),
        }
    }

    /// Replace spaced `می`/`نمی` and `ها`/`تر` affixes with their ZWNJ forms at word boundaries.
    pub fn fix_affix_zwnj<'a>(&self, input: &'a str) -> Cow<'a, str> {
        let mut buf: Option<String> = None;
        let mut last_end = 0;
        let mut search_at = 0;

        while search_at < input.len() {
            let Some(mat) = self.affix_matcher.find(&input[search_at..]) else {
                break;
            };
            let start = search_at + mat.start();
            let end = search_at + mat.end();
            let pat_id = mat.pattern().as_usize();

            if self.is_valid_affix_boundary(input, start, end, pat_id) {
                let b = buf.get_or_insert_with(|| String::with_capacity(input.len() + 16));
                b.push_str(&input[last_end..start]);
                let mut replacement = self.affix_replacements[pat_id];
                // A ZWNJ after a non-joining letter is invisible, and the lexicon
                // spells those words solid: `مرد ها` is `مردها`, not `مردها`.
                if pat_id >= 2
                    && input[..start]
                        .chars()
                        .next_back()
                        .is_some_and(|ch| crate::normalize_ops::NON_LEFT_JOINERS.contains(ch))
                {
                    replacement = replacement.trim_start_matches(ZWNJ);
                }
                b.push_str(replacement);
                last_end = end;
                search_at = end;
            } else {
                // A rejected `"می "` inside `"قدیمی "` ends on the very space the
                // `" ها"` match begins with, so skipping to its end hid that
                // match. Resume one character in instead.
                search_at = start + input[start..].chars().next().map_or(1, char::len_utf8);
            }
        }

        match buf {
            Some(mut b) => {
                b.push_str(&input[last_end..]);
                Cow::Owned(b)
            }
            None => Cow::Borrowed(input),
        }
    }

    #[inline]
    fn is_valid_affix_boundary(&self, text: &str, start: usize, end: usize, pat_id: usize) -> bool {
        let is_prefix = pat_id <= 1;
        if is_prefix {
            let valid_before = text[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphabetic());
            let valid_after = text[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic());
            valid_before && valid_after
        } else {
            let valid_before = text[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphabetic());
            let valid_after = text[end..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphabetic());
            valid_before && valid_after
        }
    }

    #[inline(always)]
    fn classify_char(&self, ch: char, last_was_space: bool, last_was_zwnj: bool) -> CharAction {
        match ch {
            '\u{064A}' | '\u{0649}' => CharAction::Replace('\u{06CC}'),
            '\u{0643}' => CharAction::Replace('\u{06A9}'),
            '\u{06C0}' => CharAction::Replace('\u{0647}'),
            ZWNJ => {
                if last_was_zwnj || last_was_space {
                    CharAction::Skip
                } else {
                    CharAction::Keep(ZWNJ)
                }
            }
            ' ' | '\u{00A0}' | '\u{2000}'..='\u{200A}' => {
                if last_was_space {
                    CharAction::Skip
                } else if ch == ' ' {
                    CharAction::Keep(' ')
                } else {
                    CharAction::Replace(' ')
                }
            }
            // Bidi controls: the overrides and the terminated/isolate forms.
            '\u{0640}'
            | '\u{200B}'
            | '\u{200D}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}' => CharAction::Skip,
            // Presentation forms: text copied out of PDFs and old documents keeps
            // these, so `ﻛﺘﺎﺏ` never matched the lexicon. The `alphabet` stage
            // decides the letter when it has an opinion, so both paths agree;
            // the generated table decomposes the rest. Teh marbuta and heh goal
            // land here too because they spell the Persian `ه`.
            '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFC}' | '\u{0629}' | '\u{06C1}' => {
                match crate::normalize_ops::alphabet_char(ch)
                    .or_else(|| crate::presentation::expand(ch))
                {
                    Some(replacement) => CharAction::Expand(replacement),
                    None => CharAction::Keep(ch),
                }
            }
            '\u{064B}'..='\u{0656}' | '\u{0670}' if self.config.strip_diacritics => {
                CharAction::Skip
            }
            '0'..='9' if self.config.unify_digits => {
                CharAction::Replace(char::from_u32(ch as u32 - 0x30 + 0x06F0).unwrap())
            }
            '\u{0660}'..='\u{0669}' if self.config.unify_digits => {
                CharAction::Replace(char::from_u32(ch as u32 - 0x0660 + 0x06F0).unwrap())
            }
            _ => CharAction::Keep(ch),
        }
    }
}

enum CharAction {
    Keep(char),
    Replace(char),
    /// A presentation form that stands for one or more letters.
    Expand(&'static str),
    Skip,
}

/// Compose `ا` + combining hamza/maddah/subscript-alef into `أ`/`آ`/`إ`.
/// Returns `None` when there is nothing to compose.
pub(crate) fn compose_decomposed(input: &str) -> Option<String> {
    const MARKS: [(char, char); 3] = [
        ('\u{0653}', '\u{0622}'),
        ('\u{0654}', '\u{0623}'),
        ('\u{0655}', '\u{0625}'),
    ];
    if !input.contains(['\u{0653}', '\u{0654}', '\u{0655}']) {
        return None;
    }

    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut changed = false;
    while let Some(ch) = chars.next() {
        if ch == '\u{0627}' {
            if let Some((_, letter)) = MARKS.iter().find(|(mark, _)| chars.peek() == Some(mark)) {
                out.push(*letter);
                chars.next();
                changed = true;
                continue;
            }
        }
        out.push(ch);
    }
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ezafe_spelling_flattens_to_plain_heh() {
        let n = Normalizer::new(NormalizerConfig::default());
        // Persian writes the same ezafe three ways. None of them is a distinct
        // word (0 of 187,582 nouns contain `ۀ`), so the default normalizer
        // flattens all three to the word's own `ه`: uniform text, and the
        // flattened form is the one the lexicon actually holds.
        let khaneh = "خانه";
        for ezafe in [
            "خانۀ".to_string(),
            "خانه\u{200C}ی".to_string(),
            "خانه ی".to_string(),
        ] {
            assert_eq!(n.normalize(&ezafe), khaneh, "{ezafe:?}");
        }
        // Flattening is idempotent and does not touch a `ی` that is a letter.
        for word in ["دنیا", "میان", "خانۀ", "خانه\u{200C}ی"] {
            let once = n.normalize(word).into_owned();
            assert_eq!(n.normalize(&once), once, "{word:?}");
        }
    }

    #[test]
    fn a_heh_followed_by_ya_is_only_flattened_at_the_ezafe() {
        let n = Normalizer::new(NormalizerConfig::default());
        // `خانه‌یی` is not the ezafe, so the `ی` stays.
        assert_eq!(
            n.normalize("خانه\u{200C}یی.",).into_owned(),
            "خانه\u{200C}یی.".to_string()
        );
        // A yā that only looks adjacent after a space stays a separate word.
        assert_eq!(n.normalize("خانه یا").into_owned(), "خانه یا".to_string());
    }

    #[test]
    fn presentation_forms_normalize_once() {
        let n = Normalizer::new(NormalizerConfig::default());
        // The generated table's targets skipped the letter rules, so a medallion
        // yeh came out as U+0649 and a second pass changed the string again.
        for form in [
            "\u{FEF4}",
            "\u{FC09}",
            "\u{FE93}",
            "\u{FEDB}\u{FEF4}\u{FEF2}",
        ] {
            let once = n.normalize(form).into_owned();
            let twice = n.normalize(&once).into_owned();
            assert_eq!(once, twice, "{form:?}");
            assert!(
                !once.chars().any(|ch| matches!(
                    ch,
                    '\u{0649}' | '\u{0629}' | '\u{06C1}' | '\u{064A}' | '\u{0643}'
                )),
                "{form:?} -> {once:?}"
            );
        }
    }

    #[test]
    fn the_presentation_table_is_binary_searchable() {
        // `expand` binary-searches the table, so an unsorted or duplicated entry
        // silently stops resolving instead of failing. The table is generated, so
        // this is the check that a regeneration has to keep passing.
        let forms = crate::presentation::PRESENTATION_FORMS;
        assert!(!forms.is_empty());
        for pair in forms.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "{:?} then {:?}",
                pair[0].0,
                pair[1].0
            );
        }
        for &(form, target) in forms {
            assert!(!target.is_empty(), "{form:?} maps to nothing");
            assert_eq!(crate::presentation::expand(form), Some(target), "{form:?}");
        }
        // A character the table does not list stays unresolved rather than being
        // mapped to whatever entry the search lands on.
        assert_eq!(crate::presentation::expand('A'), None);
        assert_eq!(crate::presentation::expand('\u{FB4F}'), None);
    }

    #[test]
    fn a_letter_plus_marks_ligature_decomposes() {
        // These decompose to a letter plus a combining mark (`ذٰ` is `ذ` with a
        // superscript alef), so the generator's letter-only filter dropped them.
        // `alphabet_char` covers some as a fallback, which hid the rest: `ﱜ`,
        // `ﲐ` and `ﳙ` survived normalization verbatim.
        let normalizer = Normalizer::new(NormalizerConfig::default());
        for (form, letter) in [
            ("\u{FC5B}", "\u{0630}"),
            ("\u{FC5C}", "\u{0631}"),
            // Alef maksura maps to the Persian `ی`, like the standalone character.
            ("\u{FC5D}", "\u{06CC}"),
            ("\u{FC90}", "\u{06CC}"),
            ("\u{FCD9}", "\u{0647}"),
            ("\u{FD3C}", "\u{0627}"),
            ("\u{FD3D}", "\u{0627}"),
        ] {
            assert_eq!(
                normalizer.normalize(form),
                letter,
                "{form:?} should become {letter:?} with the mark stripped"
            );
        }
    }

    #[test]
    fn an_isolated_haraka_is_seen_by_strip_diacritics() {
        // `FE70`-`FE7F` decompose to a space or a tatweel plus a combining mark
        // (`ﹰ` is " ً"). The generator's letter-only filter dropped all sixteen,
        // so `strip_diacritics` never saw an isolated-form haraka and the form
        // survived normalization verbatim.
        let stripping = Normalizer::new(NormalizerConfig::default());
        let keeping = Normalizer::new(NormalizerConfig {
            strip_diacritics: false,
            ..NormalizerConfig::default()
        });
        for form in ["\u{FE70}", "\u{FE72}", "\u{FE76}", "\u{FE7E}", "\u{FCF2}"] {
            assert_eq!(stripping.normalize(form), "", "{form:?} should be stripped");
            let kept = keeping.normalize(form).into_owned();
            assert!(
                kept.chars()
                    .all(|ch| ('\u{064B}'..='\u{0652}').contains(&ch)),
                "{form:?} kept as {kept:?}, expected only the combining mark"
            );
        }
    }

    #[test]
    fn decomposed_hamza_composes() {
        let n = Normalizer::new(NormalizerConfig::default());
        assert_eq!(n.normalize("ا\u{0654}حمد"), "أحمد");
        assert_eq!(n.normalize("ا\u{0653}ب"), "آب");
        assert_eq!(n.normalize("ا\u{0655}"), "إ");
        // The marks themselves are still stripped when nothing composes.
        assert_eq!(n.normalize("ا\u{064E}"), "ا");
    }

    #[test]
    fn zwnj_affix_respects_non_joiners() {
        let n = Normalizer::new(NormalizerConfig::default());
        // The same rule as `word_spacing`: no ZWNJ after a non-joining letter.
        assert_eq!(n.normalize("مرد ها"), "مردها");
        assert_eq!(n.normalize("سبز تر"), "سبزتر");
        assert_eq!(n.normalize("کتاب ها"), "کتاب‌ها");
        assert_eq!(n.normalize("می روم"), "می‌روم");
    }
}
