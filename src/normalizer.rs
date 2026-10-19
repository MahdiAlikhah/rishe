use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use std::borrow::Cow;

const ZWNJ: char = '\u{200C}';

#[derive(Debug, Clone)]
pub struct NormalizerConfig {
    pub unify_digits: bool,
    pub strip_diacritics: bool,
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

pub struct Normalizer {
    config: NormalizerConfig,
    affix_matcher: AhoCorasick,
    affix_replacements: Vec<&'static str>,
}

impl Normalizer {
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

    #[inline]
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
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
            Some(s) => Cow::Owned(s),
            None => Cow::Borrowed(input),
        }
    }

    pub fn fix_affix_zwnj<'a>(&self, input: &'a str) -> Cow<'a, str> {
        let mut buf: Option<String> = None;
        let mut last_end = 0;

        for mat in self.affix_matcher.find_iter(input) {
            let start = mat.start();
            let end = mat.end();
            let pat_id = mat.pattern().as_usize();

            if !self.is_valid_affix_boundary(input, start, end, pat_id) {
                continue;
            }

            let b = buf.get_or_insert_with(|| String::with_capacity(input.len() + 16));
            b.push_str(&input[last_end..start]);
            b.push_str(self.affix_replacements[pat_id]);
            last_end = end;
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
                .map_or(true, |c| !c.is_alphabetic());
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
                .map_or(true, |c| !c.is_alphabetic());
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
            '\u{0640}' | '\u{200B}' | '\u{200D}' | '\u{200E}' | '\u{200F}' | '\u{FEFF}' => {
                CharAction::Skip
            }
            '\u{064B}'..='\u{0652}' | '\u{0670}' if self.config.strip_diacritics => {
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
    Skip,
}
