use crate::normalize_ops::{is_persian_letter, is_punctuation};
use crate::stopwords;
use crate::utilities::word_boundary;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use std::sync::OnceLock;

const PERSIAN_DIGITS: &str = "۰۱۲۳۴۵۶۷۸۹";
const ARABIC_DIGITS: &str = "٠١٢٣٤٥٦٧٨٩";
/// Arabic combining marks, U+064B..U+0656 plus U+0670.
///
/// This is the same range `Normalizer::classify_char` drops, so
/// `compat(remove_diacritics=True)` and `normalize` agree: the masker used to
/// list only the eight harakat, which left `ا` + U+0653 MADDAH (`آب`) alone and
/// made the compatibility layer disagree with itself.
const DIACRITICS: &str = "\u{064B}\u{064C}\u{064D}\u{064E}\u{064F}\u{0650}\u{0651}\u{0652}\u{0653}\u{0654}\u{0655}\u{0656}\u{0670}";
const VS16: char = '\u{FE0F}';
const ZWJ: char = '\u{200D}';
const KEYCAP: char = '\u{20E3}';
const OFFENSIVE_WORDS: &str = include_str!("../data/offensive_words.txt");

/// Masker names used, in this order, when a caller gives none.
///
/// One list for the crate, both CLIs and the Python package; a new masker is
/// added here only if it is safe to run on every text. `apply_chain` shields
/// the `<URL>`/`<EMAIL>` placeholders, so the later maskers cannot eat them.
pub const DEFAULT_MASKERS: [&str; 10] = [
    "url",
    "email",
    "mention",
    "hashtag",
    "emoji",
    "html",
    "non_persian",
    "digits",
    "diacritics",
    "punctuation",
];

/// A masking operation, selectable by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskOp {
    /// `http` and `https` links.
    Url,
    /// Email addresses.
    Email,
    /// `@user` mentions.
    Mention,
    /// `#hashtag` tokens.
    Hashtag,
    /// Emoji sequences, including ZWJ families and flags.
    Emoji,
    /// HTML tags and character entities.
    Html,
    /// Characters outside Persian letters, digits and punctuation.
    NonPersian,
    /// Whole-word stopwords.
    Stopwords,
    /// Whole-word offensive words.
    Offensive,
    /// ASCII, Persian and Arabic digits.
    Digits,
    /// Arabic diacritics.
    Diacritics,
    /// Persian and ASCII punctuation.
    Punctuation,
}

impl MaskOp {
    /// Resolve a masker name or alias, or `None` when unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "url" | "urls" => Self::Url,
            "email" | "emails" => Self::Email,
            "mention" | "mentions" => Self::Mention,
            "hashtag" | "hashtags" => Self::Hashtag,
            "emoji" | "emojis" => Self::Emoji,
            "html" | "html_tag" | "html_tags" => Self::Html,
            "non_persian" | "non_persian_letters" => Self::NonPersian,
            "stopword" | "stopwords" => Self::Stopwords,
            "offensive" | "offensive_words" => Self::Offensive,
            "digit" | "digits" => Self::Digits,
            "diacritic" | "diacritics" => Self::Diacritics,
            "punctuation" | "punctuations" => Self::Punctuation,
            _ => return None,
        })
    }

    /// Placeholder used when the caller passes no mask; a space removes the token.
    pub fn default_mask(self) -> &'static str {
        match self {
            Self::Url => "<URL>",
            Self::Email => "<EMAIL>",
            Self::Mention | Self::Hashtag | Self::Html => " ",
            _ => "",
        }
    }

    /// Apply this masker with an explicit mask, or its default placeholder when `None`.
    pub fn apply(self, text: &str, mask: Option<&str>) -> String {
        let mask = mask.unwrap_or_else(|| self.default_mask());
        match self {
            Self::Url => mask_urls(text, mask),
            Self::Email => mask_emails(text, mask),
            Self::Mention => mask_sigil(text, '@', mask),
            Self::Hashtag => mask_sigil(text, '#', mask),
            Self::Emoji => mask_emojis(text, mask),
            Self::Html => mask_html(text, mask),
            Self::NonPersian => mask_non_persian(text, mask),
            Self::Stopwords => mask_stopwords(text, mask),
            Self::Offensive => mask_offensive(text, mask),
            Self::Digits => mask_digits(text, mask),
            Self::Diacritics => mask_diacritics(text, mask),
            Self::Punctuation => mask_punctuation(text, mask),
        }
    }
}

/// Stand-ins absent from `text` and from the caller's `mask`, one per protected
/// string.
///
/// A masker only sees a string, so `html` read `<URL>` as a tag and deleted it,
/// and `non_persian`/`punctuation` ate its letters: the placeholders have to be
/// swapped out between stages.
///
/// The stand-in has to survive every masker, and no masker may be able to create
/// one. A Persian letter that appears in neither the text nor the mask does both:
/// `non_persian` keeps Persian letters, so it survives, and since the character is
/// in no input, no deletion can join two of them and no inserted mask can contain
/// one.
///
/// Each stand-in is **one** character, never a run. A run is not safe even when
/// its character is absent: a masker deletes characters, and deleting what sits
/// between two runs glues them into a wider run. That is how a ZWNJ fallback kept
/// turning real text into a `<URL>` the input never had, across two separate
/// release fixes. A single character cannot be built by deletion, so the class of
/// bug is gone rather than patched again.
///
/// Fewer than `count` free characters means the text holds every Persian letter;
/// `apply_chain` then masks the segments between placeholders instead of shielding.
fn build_shields(text: &str, mask: &str, count: usize) -> Vec<String> {
    // Persian letters, most to least common in running text. All of
    // `PERSIAN_LETTERS` plus the four Arabic-coded letters `non_persian` keeps.
    const CANDIDATES: &[char] = &[
        'ا', 'ر', 'ن', 'د', 'ی', 'ه', 'م', 'ت', 'ب', 'س', 'و', 'ک', 'ش', 'ز', 'ف', 'گ', 'ل', 'ع',
        'ق', 'خ', 'ج', 'پ', 'چ', 'ح', 'ط', 'ص', 'ض', 'غ', 'ظ', 'ذ', 'ث', 'ژ', 'آ', 'ء', 'ؤ', 'أ',
        'ئ', 'ۀ', 'ي', 'ك', 'ى', 'ة',
    ];
    CANDIDATES
        .iter()
        .copied()
        .filter(|ch| !text.contains(*ch) && !mask.contains(*ch))
        .take(count)
        .map(|ch| ch.to_string())
        .collect()
}

/// Mask `text` when no shield character is free: split at the placeholders so no
/// masker can see across one, then mask each piece on its own. Nothing is
/// substituted, so nothing can be fabricated.
fn apply_chain_in_segments(
    text: &str,
    ops: &[MaskOp],
    mask: Option<&str>,
    protected: &[&str],
) -> String {
    let mut pieces = vec![text.to_string()];
    let mut separators = Vec::new();
    for placeholder in protected {
        let mut next = Vec::new();
        for piece in std::mem::take(&mut pieces) {
            let mut rest = piece.as_str();
            while let Some(index) = rest.find(placeholder) {
                next.push(rest[..index].to_string());
                separators.push(*placeholder);
                rest = &rest[index + placeholder.len()..];
            }
            next.push(rest.to_string());
        }
        pieces = next;
    }

    for op in ops {
        for piece in &mut pieces {
            *piece = op.apply(piece, mask);
        }
    }

    let mut remaining = pieces.into_iter();
    let mut out = remaining.next().unwrap_or_default();
    for (separator, piece) in separators.iter().zip(remaining.by_ref()) {
        out.push_str(separator);
        out.push_str(&piece);
    }
    for piece in remaining {
        out.push_str(&piece);
    }
    out
}

/// Run `ops` in order, keeping each stage's output out of the next stage.
///
/// The caller's `mask` is protected too: without that, stage one wrote `[MASK]`
/// and stage two masked its letters, so `[MASK]` came back as `[MASK]MASK[MASK]`.
/// Whitespace is not protected: swapping every space in the text for a stand-in
/// made the text one long word, so the whole-word rules ran to the end of the line
/// and `mask("x https://a.com y", ["url"], " ")` returned `"x "`.
pub(crate) fn apply_chain(text: &str, ops: &[MaskOp], mask: Option<&str>) -> String {
    if ops.is_empty() {
        return text.to_string();
    }
    let mut owned = vec!["<URL>", "<EMAIL>"];
    if let Some(token) = mask {
        if !token.trim().is_empty() {
            owned.push(token);
        }
    }
    // The caller's mask can be its own placeholder only if it equals a protected
    // one, which the shield path already treats as a single entry.
    let mut protected: Vec<&str> = Vec::new();
    for candidate in owned {
        if !protected.contains(&candidate) {
            protected.push(candidate);
        }
    }

    let shields = build_shields(text, mask.unwrap_or(""), protected.len());
    if shields.len() < protected.len() {
        return apply_chain_in_segments(text, ops, mask, &protected);
    }

    let mut current = text.to_string();
    for op in ops {
        for (placeholder, shield) in protected.iter().zip(&shields) {
            if current.contains(placeholder) {
                current = current.replace(placeholder, shield);
            }
        }
        current = op.apply(&current, mask);
        for (placeholder, shield) in protected.iter().zip(&shields).rev() {
            if current.contains(shield.as_str()) {
                current = current.replace(shield.as_str(), placeholder);
            }
        }
    }
    current
}

/// Trim only when masking changed the text; a pass with no matches leaves it alone.
fn finish(text: &str, out: String) -> String {
    if out == text {
        return out;
    }
    out.trim().to_string()
}

/// Replace links up to the next whitespace with `mask`.
pub fn mask_urls(text: &str, mask: &str) -> String {
    // Case-insensitive so a capitalised link still takes the scan: this check used
    // to skip `HTTPS://`/`WWW.` outright, so the masker never saw them. No
    // lowercased copy of the whole document: `url_end` already compares
    // case-insensitively, and a long text should not be copied to answer a
    // yes/no question. Every prefix starts with one of these letters.
    let has_link_start = text
        .bytes()
        .any(|byte| matches!(byte, b'h' | b'H' | b'w' | b'W' | b'f' | b'F'));
    if !has_link_start {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < text.len() {
        // The same span rule the tokenizer uses, so `(https://a.com)` keeps its
        // `)`, and `www.`/`ftp://` are URLs here too.
        if let Some(end) = crate::word_tokenizer::url_end(text, i) {
            out.push_str(mask);
            i = end;
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    finish(text, out)
}

/// Replace well-formed email addresses with `mask`.
pub fn mask_emails(text: &str, mask: &str) -> String {
    if !text.contains('@') {
        return text.to_string();
    }

    let spans = email_spans(text);
    if spans.is_empty() {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end) in spans {
        out.push_str(&text[last..start]);
        out.push_str(mask);
        last = end;
    }
    out.push_str(&text[last..]);
    finish(text, out)
}

pub(crate) fn email_spans(text: &str) -> Vec<(usize, usize)> {
    // Char-based: a Persian local part or an IDN domain is multi-byte, and
    // indexing bytes by `start - 1` used to step back into the middle of one.
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut spans = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].1 == '@' {
            // Stop at the previous span, or the local part of `a@b.cc@d.ee` overlaps it.
            let last_end = spans.last().map_or(0, |&(_, end)| end);
            let mut start = i;
            while start > 0 && chars[start - 1].0 >= last_end && is_email_local(chars[start - 1].1)
            {
                start -= 1;
            }
            let mut domain_end = i + 1;
            while domain_end < chars.len() && is_email_domain(chars[domain_end].1) {
                domain_end += 1;
            }

            let mut match_end = None;
            for dot in (i + 1..domain_end).rev() {
                if chars[dot].1 != '.' || dot == i + 1 {
                    continue;
                }
                let mut run_end = dot + 1;
                while run_end < domain_end && chars[run_end].1.is_alphabetic() {
                    run_end += 1;
                }
                if run_end - (dot + 1) >= 2 {
                    match_end = Some(run_end);
                    break;
                }
            }

            if start < i {
                if let Some(end) = match_end {
                    let end_byte = chars.get(end).map_or(text.len(), |&(offset, _)| offset);
                    spans.push((chars[start].0, end_byte));
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    spans
}

fn is_email_local(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-')
}

fn is_email_domain(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '.' | '-')
}

/// Replace `sigil` tokens up to the next whitespace with `mask`.
pub fn mask_sigil(text: &str, sigil: char, mask: &str) -> String {
    if !text.contains(sigil) {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        // A mask differs from the tokenizer on one point: a sigil after `.` or `_`
        // still starts a mention here, because masking should not leave it behind.
        if let Some(end) = crate::word_tokenizer::sigil_end(
            &chars,
            i,
            sigil,
            |prev| !prev.is_alphanumeric(),
            |ch| !ch.is_whitespace(),
        ) {
            // Stop before the punctuation that ended the sentence, the way the
            // `url` masker already does: `@user!` used to come back as `""`, losing
            // the `!`, while `https://a.b!` kept it.
            let mut end = end;
            while end > i + 1 && crate::word_tokenizer::URL_TRAILING.contains(&chars[end - 1]) {
                end -= 1;
            }
            out.push_str(mask);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    finish(text, out)
}

/// Replace emoji sequences with `mask`.
pub fn mask_emojis(text: &str, mask: &str) -> String {
    // A keycap's base is an ASCII digit, so `is_emoji_base` alone would skip the
    // whole text before `emoji_end` ever sees it. `U+20E3` only appears in one.
    if !text.chars().any(is_emoji_base) && !text.contains(KEYCAP) {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < chars.len() {
        if let Some(end) = emoji_end(&chars, i) {
            out.push_str(mask);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    finish(text, out)
}

/// End of the emoji cluster starting at `i`: a regional pair, or a base with its
/// skin tone, variation selector and ZWJ-joined continuation. The tokenizer splits
/// tokens on exactly these clusters, so both share this walk.
pub(crate) fn emoji_end(chars: &[char], i: usize) -> Option<usize> {
    let ch = chars[i];
    if is_regional(ch) && i + 1 < chars.len() && is_regional(chars[i + 1]) {
        return Some(i + 2);
    }
    // A keycap is an ASCII base plus `U+FE0F U+20E3`, so the base looks like a
    // letter and the marks were left behind: `1️⃣` survived the `emoji` masker and
    // split into three tokens. The `U+FE0F` is optional in the wild, so the width
    // is read from the marks rather than assumed to be three.
    if matches!(ch, '0'..='9' | '#' | '*') {
        if let Some(width) = keycap_width(chars, i + 1) {
            return Some(i + 1 + width);
        }
    }
    if !is_emoji_base(ch) {
        return None;
    }
    let mut end = i + 1;
    if end < chars.len() && is_skin_tone(chars[end]) {
        end += 1;
    }
    if end < chars.len() && chars[end] == VS16 {
        end += 1;
    }
    while end + 1 < chars.len() && chars[end] == ZWJ && is_emoji_base(chars[end + 1]) {
        end += 2;
        if end < chars.len() && is_skin_tone(chars[end]) {
            end += 1;
        }
        if end < chars.len() && chars[end] == VS16 {
            end += 1;
        }
    }
    Some(end)
}

pub(crate) fn is_emoji_base(ch: char) -> bool {
    // ❶..➓ are pictographs of numbers, not emoji, so they must survive.
    if matches!(ch as u32, 0x2776..=0x2793) {
        return false;
    }
    matches!(
        ch as u32,
        0x231A..=0x231B
            | 0x23E9..=0x23F3
            // 2B00..2BFF is the Misc Symbols and Arrows block: it holds the emoji
            // ⬛ U+2B1B, ⭕ U+2B55, ⬅ U+2B05 and friends. Without it those were
            // glued to the neighbouring word while every other pictograph split.
            | 0x2B00..=0x2BFF
            | 0x2600..=0x26FF
            | 0x2700..=0x2775
            | 0x2794..=0x27BF
            | 0x1F000..=0x1F02F
            | 0x1F0A0..=0x1F0FF
            | 0x1F100..=0x1F1FF
            | 0x1F200..=0x1F2FF
            | 0x1F300..=0x1F5FF
            | 0x1F600..=0x1F64F
            | 0x1F680..=0x1F6FF
            | 0x1F700..=0x1F77F
            | 0x1F780..=0x1F7FF
            | 0x1F800..=0x1F8FF
            | 0x1F900..=0x1F9FF
            | 0x1FA00..=0x1FA6F
            | 0x1FA70..=0x1FAFF
    )
}

pub(crate) fn is_regional(ch: char) -> bool {
    matches!(ch as u32, 0x1F1E6..=0x1F1FF)
}

/// Width of the marks that make a keycap at `at`: `U+FE0F U+20E3` (2) or a bare
/// `U+20E3` (1). `None` when the marks there are not a keycap.
fn keycap_width(chars: &[char], at: usize) -> Option<usize> {
    match chars.get(at) {
        Some(&VS16) if chars.get(at + 1) == Some(&KEYCAP) => Some(2),
        Some(&KEYCAP) => Some(1),
        _ => None,
    }
}

pub(crate) fn is_skin_tone(ch: char) -> bool {
    matches!(ch as u32, 0x1F3FB..=0x1F3FF)
}

/// `&CounterClockwiseContourIntegral;` is the longest HTML5 named entity at 33
/// bytes including the `&`; one byte of slack keeps the window a hard bound.
const MAX_ENTITY_LEN: usize = 34;

/// Real tags are short and never contain a newline, so a `>` beyond this is not
/// a closing bracket.
const MAX_TAG_LEN: usize = 1024;

/// End of a look-ahead window from `start`, snapped back to a character boundary
/// so a byte count cannot split a character and panic the slice.
fn bounded_end(text: &str, start: usize, max: usize) -> usize {
    let mut end = (start + max).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// Whether the text between `<` and `>` is markup rather than prose.
///
/// `"a<b و c>d"` used to lose `"<b و c>"` because any `>` closed a tag. Real
/// tags are ASCII markup; non-ASCII is allowed only inside a quoted attribute
/// value, where scraped pages carry Persian. A tag also never spans a newline.
fn looks_like_tag(content: &str) -> bool {
    if content.contains('\n') || content.contains('\r') {
        return false;
    }
    // `<!-- ... -->`, `<!DOCTYPE ...>`, `<?xml ...?>` and `<!\[CDATA\[` are
    // markup with no prose reading. A bare `<!`/`<?` in front of Persian prose is
    // not: `<!مهم>` used to be deleted as a tag.
    if let Some(rest) = content.strip_prefix('!') {
        return rest.starts_with("--")
            || rest.starts_with("[CDATA[")
            || rest
                .get(..7)
                .is_some_and(|head| head.eq_ignore_ascii_case("DOCTYPE"))
            || content.is_ascii();
    }
    if content.starts_with('?') {
        return content.is_ascii();
    }
    let mut quote = None;
    for ch in content.chars() {
        match quote {
            Some(open) => {
                if ch == open {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                _ if !ch.is_ascii() => return false,
                _ => {}
            },
        }
    }
    true
}

/// Decode HTML entities and replace tags with `mask`.
pub fn mask_html(text: &str, mask: &str) -> String {
    if !text.contains('<') && !text.contains('&') {
        return text.to_string();
    }

    // Strip tags before unescaping: `&lt;b&gt;` is text a reader should see, so
    // unescaping first would make it look like a tag and delete it.
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < text.len() {
        if text.as_bytes()[i] == b'<' {
            let tag_start = text[i + 1..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?'));
            if tag_start {
                // Only look for the `>` of an unterminated tag nearby: scanning
                // the whole rest of the text made this quadratic in `<` count.
                let window_end = bounded_end(text, i, MAX_TAG_LEN);
                if let Some(close) = text[i..window_end].find('>') {
                    if close > 1 && looks_like_tag(&text[i + 1..i + close]) {
                        out.push_str(mask);
                        i += close + 1;
                        continue;
                    }
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    finish(text, html_unescape(&out))
}

fn html_unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < text.len() {
        if text.as_bytes()[i] == b'&' {
            // Bound the `;` search: an unterminated `&` used to scan the whole
            // rest of the text, making this quadratic in `&` count.
            let window_end = bounded_end(text, i, MAX_ENTITY_LEN);
            if let Some(semi) = text[i..window_end].find(';').filter(|semi| *semi <= 32) {
                if let Some(replacement) = decode_entity(&text[i + 1..i + semi]) {
                    out.push_str(&replacement);
                    i += semi + 1;
                    continue;
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

/// A numeric reference to a control character or noncharacter is an HTML parse
/// error, so it is left undecoded rather than emitting a NUL into the text.
fn text_char(code: u32) -> Option<String> {
    let ch = char::from_u32(code)?;
    let noncharacter = (0xFDD0..=0xFDEF).contains(&code) || code & 0xFFFE == 0xFFFE;
    let keep = matches!(ch, '\t' | '\n' | '\r') || (!ch.is_control() && !noncharacter);
    keep.then(|| ch.to_string())
}

// ponytail: common HTML entities and numeric references only; add the full
// HTML5 table if real input needs it.
fn decode_entity(entity: &str) -> Option<String> {
    let named = match entity {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }

    if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        return u32::from_str_radix(hex, 16).ok().and_then(text_char);
    }
    if let Some(decimal) = entity.strip_prefix('#') {
        return decimal.parse::<u32>().ok().and_then(text_char);
    }
    None
}

/// Replace non-Persian characters with `mask`.
///
/// There were `keep_english`/`keep_diacritics` flags, but no surface reached
/// them: the masker name always passed `false, false`. Two unreachable knobs
/// are a contract nobody tests, so the function takes what it uses.
pub fn mask_non_persian(text: &str, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for ch in text.chars() {
        let allowed = is_persian_letter(ch)
            // The same letters with an Arabic code point: `normalize` maps them,
            // so deleting them here shortened `كتاب` to `تاب` and `مصطفى` to `مصطف`.
            || matches!(ch, 'ي' | 'ك' | 'ى' | 'ة')
            // Every whitespace, not just `' '`: deleting a newline joined lines
            // and deleting a space joined words (`سلام\nدنیا,خوب` -> `سلامدنیاخوب`).
            || ch.is_whitespace()
            || PERSIAN_DIGITS.contains(ch)
            || ch.is_ascii_digit()
            || is_punctuation(ch)
            || ch.is_ascii_punctuation();

        if allowed {
            out.push(ch);
        } else {
            out.push_str(mask);
        }
    }

    finish(text, out)
}

/// Replace whole-word stopwords with `mask`.
pub fn mask_stopwords(text: &str, mask: &str) -> String {
    replace_words(text, stopword_matcher(), mask)
}

/// Replace whole-word offensive words with `mask`.
pub fn mask_offensive(text: &str, mask: &str) -> String {
    replace_words(text, offensive_matcher(), mask)
}

static STOPWORD_MATCHER: OnceLock<AhoCorasick> = OnceLock::new();
static OFFENSIVE_MATCHER: OnceLock<AhoCorasick> = OnceLock::new();

fn stopword_matcher() -> &'static AhoCorasick {
    STOPWORD_MATCHER.get_or_init(|| build_matcher(stopwords::stopwords()))
}

fn offensive_matcher() -> &'static AhoCorasick {
    OFFENSIVE_MATCHER.get_or_init(|| {
        // The list spells some entries with a space and others with a ZWNJ; the
        // text may use either, or neither (`کصلیس` is written closed). All three
        // spellings go in: `بی‌ناموس` and `کصلیس` only passed the filter because
        // the list held one of the other two forms.
        let mut patterns: Vec<String> = Vec::with_capacity(OFFENSIVE_WORDS.lines().count() * 3);
        for word in OFFENSIVE_WORDS.lines().map(str::trim) {
            if word.is_empty() {
                continue;
            }
            patterns.push(word.to_owned());
            if word.contains(' ') {
                patterns.push(word.replace(' ', "\u{200C}"));
                patterns.push(word.replace(' ', ""));
            } else if word.contains('\u{200C}') {
                patterns.push(word.replace('\u{200C}', " "));
                patterns.push(word.replace('\u{200C}', ""));
            }
        }
        patterns.sort();
        patterns.dedup();
        build_matcher(patterns)
    })
}

fn build_matcher<I, P>(patterns: I) -> AhoCorasick
where
    I: IntoIterator<Item = P>,
    P: AsRef<[u8]>,
{
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(patterns)
        .expect("valid Aho-Corasick patterns")
}

fn replace_words(text: &str, matcher: &AhoCorasick, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;

    for found in matcher.find_iter(text) {
        if !word_boundary(text, found.start(), found.end()) {
            continue;
        }
        out.push_str(&text[last..found.start()]);
        out.push_str(mask);
        last = found.end();
    }
    out.push_str(&text[last..]);
    finish(text, out)
}

/// Replace each digit with `mask`.
pub fn mask_digits(text: &str, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_digit() || PERSIAN_DIGITS.contains(ch) || ARABIC_DIGITS.contains(ch) {
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }
    finish(text, out)
}

/// Replace each diacritic with `mask`.
pub fn mask_diacritics(text: &str, mask: &str) -> String {
    // Compose first, exactly like `Normalizer`: a decomposed `ا` + U+0653 MADDAH
    // is the letter `آ`, so stripping the mark alone would turn `آب` into `اب`
    // instead of leaving the word alone.
    let composed = crate::normalizer::compose_decomposed(text);
    let text = composed.as_deref().unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if DIACRITICS.contains(ch) {
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }
    finish(text, out)
}

/// Replace Persian and ASCII punctuation with `mask`.
pub fn mask_punctuation(text: &str, mask: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if is_punctuation(ch) || ch.is_ascii_punctuation() {
            out.push_str(mask);
        } else {
            out.push(ch);
        }
    }
    finish(text, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_links_and_contacts() {
        assert_eq!(
            MaskOp::Url.apply("برای اطلاعات بیشتر به https://shekar.io مراجعه کنید.", None),
            "برای اطلاعات بیشتر به <URL> مراجعه کنید."
        );
        // The URL ends where the sentence does: the closing bracket, comma,
        // full stop and guillemet are not part of it.
        assert_eq!(mask_urls("(https://a.com) بعد", "<URL>"), "(<URL>) بعد");
        assert_eq!(mask_urls("«https://a.com»", "<URL>"), "«<URL>»");
        assert_eq!(mask_urls("به https://a.com، برو", "<URL>"), "به <URL>، برو");
        assert_eq!(mask_urls("پایان https://a.com.", "<URL>"), "پایان <URL>.");
        // The masker and the tokenizer scan the same prefixes and the same
        // word-start rule.
        assert_eq!(mask_urls("www.example.com", "<URL>"), "<URL>");
        assert_eq!(mask_urls("ftp://files.example.com", "<URL>"), "<URL>");
        assert_eq!(mask_urls("abchttps://a.com", "<URL>"), "abchttps://a.com");
        assert_eq!(
            MaskOp::Email.apply("برای تماس با ما به info@shekar.io ایمیل بزنید.", None),
            "برای تماس با ما به <EMAIL> ایمیل بزنید."
        );
        assert_eq!(
            mask_emails("mail info@a.co, ok", "<EMAIL>"),
            "mail <EMAIL>, ok"
        );
        assert_eq!(
            MaskOp::Url.apply("x https://a.b y", Some("[MASK]")),
            "x [MASK] y"
        );
    }

    #[test]
    fn overlapping_emails_do_not_panic() {
        // The backwards scan from the second `@` used to cross the first
        // span's end, giving overlapping spans and a reversed byte range.
        assert_eq!(mask_emails("a@b.cc@d.ee", "<EMAIL>"), "<EMAIL>@d.ee");
        assert_eq!(mask_emails("x@y.com@z.org", "<EMAIL>"), "<EMAIL>@z.org");
        assert_eq!(mask_emails("a@b.cc", "<EMAIL>"), "<EMAIL>");
        assert_eq!(mask_emails("a@b@c.dd", "<EMAIL>"), "a@<EMAIL>");
    }

    #[test]
    fn emails_with_unicode_parts_and_idn_domains_are_masked() {
        // Byte-based scanning treated the UTF-8 bytes of a Persian letter as
        // separators, so the address came back untouched.
        assert_eq!(mask_emails("x کاربر@example.com", "<EMAIL>"), "x <EMAIL>");
        assert_eq!(mask_emails("x user@مثال.ایران", "<EMAIL>"), "x <EMAIL>");
        assert_eq!(mask_emails("x a@b.cc@d.ee", "<EMAIL>"), "x <EMAIL>@d.ee");
    }

    #[test]
    fn masks_social_tokens() {
        // The `!` used to be masked with the handle, so the sentence lost its
        // punctuation; the `url` masker always kept it.
        assert_eq!(
            MaskOp::Mention.apply("سلام @user! چطوری؟", None),
            "سلام  ! چطوری؟"
        );
        assert_eq!(MaskOp::Hashtag.apply("#سلام #خوش_آمدید", None), "");
        // A sigil with nothing after it is punctuation, not a mention or tag.
        assert_eq!(MaskOp::Mention.apply("خالص @ و", None), "خالص @ و");
        assert_eq!(MaskOp::Hashtag.apply("خالص # و", None), "خالص # و");
        // A sigil inside a word stays.
        assert_eq!(MaskOp::Hashtag.apply("abc#def", None), "abc#def");
    }

    #[test]
    fn masks_emoji_sequences() {
        assert_eq!(
            MaskOp::Emoji.apply("درود بر شما😊!🌟", None),
            "درود بر شما!"
        );
        assert_eq!(MaskOp::Emoji.apply("family 👨‍👩‍👧‍👦", None), "family");
        assert_eq!(MaskOp::Emoji.apply("flag 🇮🇷", None), "flag");
        // Star, watch and alarm are emoji and must go.
        for ch in ['\u{2B50}', '\u{231A}', '\u{23F0}'] {
            assert_eq!(
                MaskOp::Emoji.apply(&format!("x{ch}y"), None),
                "xy",
                "{ch:?}"
            );
        }
        // A circled digit is a number, not an emoji, and must stay.
        for ch in ['\u{2776}', '\u{2777}'] {
            assert_eq!(
                MaskOp::Emoji.apply(&format!("x{ch}y"), None),
                format!("x{ch}y"),
                "{ch:?}"
            );
        }
    }

    #[test]
    fn html_ignores_prose_between_angle_brackets() {
        // Any `>` used to close a tag, so the Persian words between `<` and `>`
        // were deleted: this came back as "اگر ad باشد".
        assert_eq!(mask_html("اگر a<b و c>d باشد", ""), "اگر a<b و c>d باشد");
        // Real markup still goes: an ASCII tag, one with Persian inside a quoted
        // attribute value, and a comment.
        assert_eq!(mask_html("<p>متن</p>", ""), "متن");
        assert_eq!(mask_html("<a title=\"سلام\">متن</a>", ""), "متن");
        assert_eq!(mask_html("<!-- یادداشت -->متن", ""), "متن");
        // A tag never spans a newline.
        assert_eq!(mask_html("a<b\nc>d", ""), "a<b\nc>d");
        // `<!`/`<?` before Persian prose is not markup: `<!مهم>` used to be
        // deleted as if it were a comment or a processing instruction.
        assert_eq!(mask_html("این <!مهم> است", ""), "این <!مهم> است");
        assert_eq!(mask_html("این <?مهم> است", ""), "این <?مهم> است");
        // The real declarations and comments still go.
        assert_eq!(mask_html("<!DOCTYPE html>متن", ""), "متن");
        assert_eq!(mask_html("<?xml version=\"1.0\"?>متن", ""), "متن");
        assert_eq!(mask_html("<!CDATA[ x ]]>متن", ""), "متن");
    }

    #[test]
    fn chain_keeps_placeholders() {
        let text = "به https://a.b برو";
        // Each later masker used to treat `<URL>` as ordinary text.
        for later in [
            MaskOp::Html,
            MaskOp::NonPersian,
            MaskOp::Punctuation,
            MaskOp::Digits,
            MaskOp::Diacritics,
            MaskOp::Emoji,
        ] {
            assert_eq!(
                apply_chain(text, &[MaskOp::Url, later], None),
                "به <URL> برو",
                "{later:?}"
            );
        }
        assert_eq!(
            apply_chain("x info@a.co y", &[MaskOp::Email, MaskOp::Html], None),
            "x <EMAIL> y"
        );
    }

    #[test]
    fn non_persian_keeps_arabic_coded_persian_letters() {
        // `normalize` maps these to `ی`/`ک`/`ه`; deleting them shortened words.
        assert_eq!(mask_non_persian("او كتابي خواند", ""), "او كتابي خواند");
        assert_eq!(mask_non_persian("مصطفى كبيرة", ""), "مصطفى كبيرة");
        assert_eq!(MaskOp::NonPersian.apply("سلام hello", None), "سلام");
    }

    #[test]
    fn non_persian_drops_every_non_persian_letter() {
        // The removed `keep_english` knob let `hello` through, which is exactly
        // what `non_persian` promises to remove.
        assert_eq!(mask_non_persian("سلام hello 123!", ""), "سلام  123!");
        // `finish` trims the masked tail, so only the kept run survives.
        assert_eq!(mask_non_persian("سلام ۱۲۳ hello", " "), "سلام ۱۲۳");
    }

    #[test]
    fn a_byte_cap_inside_a_character_is_not_a_panic() {
        // The window end is a byte offset; it used to land inside a character and
        // panic the slice, which every default `mask()` call could reach.
        let entity = format!("&ab{}", "ب".repeat(20));
        assert_eq!(mask_html(&entity, "[MASK]"), entity);
        let tag = format!("<a{}z{}>", "ب".repeat(510), "ب".repeat(10));
        assert_eq!(mask_html(&tag, "[MASK]"), tag);
        // The cap still masks ordinary tags, so the bound did not disable it.
        assert_eq!(mask_html("<b>سلام</b>", ""), "سلام");
    }

    #[test]
    fn masks_html_tags_and_entities() {
        assert_eq!(
            MaskOp::Html.apply("<p>این یک <strong>متن</strong> نمونه است.</p>", None),
            "این یک  متن  نمونه است."
        );
        assert_eq!(mask_html("a &amp; b", ""), "a & b");
        assert_eq!(mask_html("a < b > c", ""), "a < b > c");
        assert_eq!(mask_html("اگر 1 < 2 و 3 > 2", ""), "اگر 1 < 2 و 3 > 2");
        assert_eq!(mask_html("&#1587;&#x644;&#x627;&#1605;", ""), "سلام");
        // Escaped markup is text a reader should see, not a tag to delete.
        assert_eq!(mask_html("&lt;b&gt;سلام&lt;/b&gt;", ""), "<b>سلام</b>");
        assert_eq!(mask_html("&lt;script&gt;", ""), "<script>");
        // Control characters and noncharacters are never decoded.
        assert_eq!(
            mask_html("&#0;&#1;&#x1f;&#127;&#xFFFE;&#xFFFF;", ""),
            "&#0;&#1;&#x1f;&#127;&#xFFFE;&#xFFFF;"
        );
        assert_eq!(mask_html("a&#0;b", ""), "a&#0;b");
        // Whitespace controls are legitimate and still decode.
        assert_eq!(mask_html("a&#9;&#10;&#13;b", ""), "a\t\n\rb");
    }

    #[test]
    fn masks_non_persian_letters() {
        // Digits and punctuation survive: `digits` and `punctuation` are the
        // maskers that remove those, and this one is `non_persian_letters`.
        assert_eq!(
            MaskOp::NonPersian.apply("سلام hello 123!", None),
            "سلام  123!"
        );
        assert_eq!(mask_non_persian("سلام hello 123!", ""), "سلام  123!");
    }

    #[test]
    fn non_persian_keeps_separators() {
        // Deleting the newline and the comma joined two lines and two words.
        assert_eq!(
            MaskOp::NonPersian.apply("سلام\nدنیا,خوب", None),
            "سلام\nدنیا,خوب"
        );
        assert_eq!(
            MaskOp::NonPersian.apply("سلام دنیا\tخوب", None),
            "سلام دنیا\tخوب"
        );
        assert_eq!(MaskOp::NonPersian.apply("a-b_c", None), "-_");
        assert_eq!(
            MaskOp::NonPersian.apply("قیمت 123 تومان", None),
            "قیمت 123 تومان"
        );
    }

    #[test]
    fn masks_stopwords_and_offensive_words() {
        assert_eq!(MaskOp::Stopwords.apply("این کتاب است", None), "کتاب");
        assert_eq!(mask_stopwords("کتاب و دفتر", ""), "کتاب  دفتر");
        assert_eq!(MaskOp::Offensive.apply("این بی‌شرف است", None), "این  است");
    }

    #[test]
    fn a_word_ends_at_a_non_persian_letter_or_digit() {
        // The masker's predicate only looked for Persian letters, so `استx` was
        // the stopword `است` and `و2` was `و`, while `utilities` disagreed. One
        // predicate for the crate: ASCII alphanumerics are word characters.
        assert_eq!(mask_stopwords("استx", ""), "استx");
        assert_eq!(mask_stopwords("xاست", ""), "xاست");
        assert_eq!(mask_stopwords("است2", ""), "است2");
        assert_eq!(mask_stopwords("است", ""), "");
        // Punctuation still ends the word.
        assert_eq!(mask_stopwords("است،", ""), "،");
        assert_eq!(mask_stopwords("است.", ""), ".");
    }

    #[test]
    fn offensive_words_match_their_space_and_zwnj_spellings() {
        // The list spells some entries with a space and some with a ZWNJ; text
        // uses either, and closed spellings like `کصلیس` are common too.
        for word in ["بی پدر", "بی‌پدر", "بی ناموس", "بی‌ناموس"] {
            assert_eq!(MaskOp::Offensive.apply(word, None), "", "{word:?}");
        }
        for word in ["کس لیس", "کس‌لیس", "کص لیس", "کص‌لیس", "کصلیس"] {
            assert_eq!(MaskOp::Offensive.apply(word, None), "", "{word:?}");
        }
        // Ordinary text is untouched.
        assert_eq!(
            MaskOp::Offensive.apply("کتاب خوب است", None),
            "کتاب خوب است"
        );
    }

    #[test]
    fn masks_digits_diacritics_punctuation() {
        assert_eq!(
            MaskOp::Digits.apply("این متن 1234 شامل اعداد است.", None),
            "این متن  شامل اعداد است."
        );
        assert_eq!(
            MaskOp::Diacritics.apply("کُجا نِشانِ قَدَم ناتَمام خواهَد ماند؟", None),
            "کجا نشان قدم ناتمام خواهد ماند؟"
        );
        assert_eq!(
            MaskOp::Punctuation.apply("دریغ است ایران که ویران شود!", None),
            "دریغ است ایران که ویران شود"
        );
    }

    #[test]
    fn the_diacritics_masker_agrees_with_the_normalizer() {
        // The masker listed only the eight harakat while `Normalizer` strips the
        // whole U+064B..U+0656 range plus U+0670, so `آب` (`ا` + MADDAH) lost its
        // mark instead of composing to `آ` and `compat(remove_diacritics=True)`
        // disagreed with `normalize`.
        assert_eq!(MaskOp::Diacritics.apply("ا\u{0653}ب", None), "آب");
        assert_eq!(MaskOp::Diacritics.apply("آب", None), "آب");
        for ch in ['\u{0653}', '\u{0654}', '\u{0655}', '\u{0656}', '\u{0670}'] {
            assert_eq!(
                MaskOp::Diacritics.apply(&format!("ب{ch}"), None),
                "ب",
                "U+{:04X} was not stripped",
                ch as u32
            );
        }
    }

    #[test]
    fn the_emoji_ranges_cover_misc_symbols_and_arrows() {
        // The Misc Symbols and Arrows block was missing, so ⬛ ⭕ ⬅ stuck to the
        // neighbouring word while every other pictograph split off cleanly.
        for ch in ['\u{2B1B}', '\u{2B55}', '\u{2B05}', '\u{2B50}'] {
            assert!(
                is_emoji_base(ch),
                "U+{:04X} should be treated as an emoji",
                ch as u32
            );
        }
        // Pictographs of numbers are not emoji and must keep their meaning.
        for ch in ['\u{2776}', '\u{2777}', '\u{2793}'] {
            assert!(
                !is_emoji_base(ch),
                "U+{:04X} is a number pictograph, not an emoji",
                ch as u32
            );
        }
    }

    #[test]
    fn a_pass_without_matches_leaves_the_text_alone() {
        assert_eq!(MaskOp::Url.apply("  a  b  ", None), "  a  b  ");
        assert_eq!(MaskOp::Mention.apply("  a  b  ", None), "  a  b  ");
    }

    #[test]
    fn a_sigil_inside_a_word_is_not_a_mention() {
        assert_eq!(
            MaskOp::Mention.apply("info@example.com", None),
            "info@example.com"
        );
        assert_eq!(MaskOp::Hashtag.apply("C#tag", None), "C#tag");
    }

    #[test]
    fn op_names_resolve() {
        assert_eq!(MaskOp::from_name("url"), Some(MaskOp::Url));
        assert_eq!(MaskOp::from_name("stopwords"), Some(MaskOp::Stopwords));
        assert_eq!(MaskOp::from_name("nope"), None);
    }

    #[test]
    fn unterminated_entities_and_tags_stay_linear() {
        use std::time::Instant;

        // Both searches used to scan to the end of the text per `&` or `<`,
        // so 160k characters took 0.6s and every doubling quadrupled the time.
        // Compare 20k against 160k: linear is 8x, quadratic would be 64x.
        let time = |n: usize, ch: &str| {
            let text = ch.repeat(n);
            let start = Instant::now();
            mask_html(&text, "<MASK>");
            start.elapsed()
        };

        for ch in ["&", "<a"] {
            let small = time(20_000, ch).as_nanos().max(1);
            let large = time(160_000, ch).as_nanos();
            assert!(
                large < small * 32,
                "{ch:?}: 8x the input took {}x the time, so the scan is not linear",
                large / small
            );
        }
    }

    #[test]
    fn a_chain_leaves_text_its_placeholders_cannot_collide_with() {
        // The shields used to be the fixed pairs `ژژ`/`چچ`, which real words
        // contain, so masking invented a URL or an email out of the text.
        let ops = [MaskOp::Url, MaskOp::Email];
        for text in ["سلام ژژ دنیا", "چچ", "غژژ", "فژژ", "چچار"] {
            assert_eq!(apply_chain(text, &ops, None), text, "{text:?} was altered");
        }
        // A shield run derived from a text that already contains ZWNJ runs.
        let hostile = "می\u{200C}\u{200C}\u{200C}\u{200C}روم";
        assert_eq!(apply_chain(hostile, &ops, None), hostile);
    }

    #[test]
    fn a_chain_keeps_the_callers_mask_token() {
        // Stage one wrote `[MASK]` and the next stage masked its letters, so the
        // token came back as `[MASK]MASK[MASK]`.
        for token in ["[MASK]", "<X>", "URL", "**", "ژژ"] {
            let out = apply_chain(
                "info@a.co",
                &[MaskOp::Email, MaskOp::NonPersian, MaskOp::Punctuation],
                Some(token),
            );
            assert_eq!(out, token, "token {token:?} was re-masked");
        }
    }

    #[test]
    fn a_chain_never_fabricates_a_placeholder_when_every_letter_is_taken() {
        // Three releases in a row a masker turned real text into a `<URL>` the
        // input never had, because the stand-in was made of characters the text
        // already contained and a masker could glue two of them into one.
        // `build_shields` now returns only single characters and `apply_chain`
        // falls back to masking each segment, so no masker ever sees across a
        // placeholder.
        let mut crowded = String::from(crate::normalize_ops::PERSIAN_LETTERS);
        crowded.push_str("يكىة  ");
        let ops = [MaskOp::Digits];
        for suffix in ["می\u{200C}۱\u{200C}کنم", "می\u{200C}۱\u{200C}۲\u{200C}کنم"] {
            let text = format!("{crowded}{suffix}");
            let out = apply_chain(&text, &ops, None);
            for placeholder in ["<URL>", "<EMAIL>"] {
                assert!(
                    !out.contains(placeholder),
                    "{placeholder} was fabricated from {suffix:?}: {out:?}"
                );
            }
        }
    }

    #[test]
    fn the_segment_fallback_stays_linear() {
        // Every shield character is present, so the chain masks each segment
        // instead. That path must not rescan the text per placeholder.
        use std::time::Instant;

        let ops = [MaskOp::Url, MaskOp::Email];
        let time = |n: usize| {
            let mut text = String::from(crate::normalize_ops::PERSIAN_LETTERS);
            text.push_str("يكىة");
            text.push_str(&"\u{200C}".repeat(n));
            text.push_str(" https://a.example");
            let start = Instant::now();
            let out = apply_chain(&text, &ops, None);
            assert!(out.contains("<URL>"), "no placeholder survived: {out:?}");
            start.elapsed().as_nanos().max(1)
        };

        let small = time(1_000);
        let large = time(8_000);
        assert!(
            large < small * 32,
            "8x the ZWNJ runs took {}x the time, so the scan is not linear",
            large / small
        );
    }

    #[test]
    fn a_sigil_keeps_the_punctuation_that_ended_it() {
        // `mask_sigil` ran to the next whitespace, so `@user!` masked the `!` as
        // well. The `url` masker already trimmed it, so the two disagreed.
        assert_eq!(MaskOp::Mention.apply("@user!", None), "!");
        assert_eq!(MaskOp::Mention.apply("@user.", None), ".");
        assert_eq!(MaskOp::Mention.apply("@user،", None), "،");
        assert_eq!(MaskOp::Mention.apply("(@user)", None), "( )");
        assert_eq!(MaskOp::Hashtag.apply("#خبر!", None), "!");
        // The url masker's behaviour is the one both now follow.
        assert_eq!(MaskOp::Url.apply("https://a.b!", None), "<URL>!");
    }

    #[test]
    fn a_keycap_is_one_emoji() {
        // `1️⃣` is an ASCII digit plus two marks, so the base was in no emoji
        // range: the `emoji` masker leaked it and the tokenizer split it in three.
        for keycap in ["1\u{FE0F}\u{20E3}", "#\u{FE0F}\u{20E3}", "*\u{20E3}"] {
            let text = format!("a{keycap}b");
            assert_eq!(
                mask_emojis(&text, ""),
                "ab",
                "{keycap:?} was not masked as one cluster"
            );
            let chars: Vec<char> = text.chars().collect();
            assert_eq!(
                emoji_end(&chars, 1),
                Some(1 + keycap.chars().count()),
                "{keycap:?} cluster is the wrong width"
            );
        }
        // A lone digit next to a mark that is not a keycap is not an emoji.
        assert_eq!(emoji_end(&['1', '\u{FE0F}'], 0), None);
    }
}
