import unicodedata

ranges = [(0xFB50, 0xFE00), (0xFE70, 0xFF00)]
final = {"\u0643": "\u06A9", "\u064A": "\u0649", "\u06C0": "\u0647"}

entries = []
for lo, hi in ranges:
    for cp in range(lo, hi):
        ch = chr(cp)
        if unicodedata.category(ch)[0] != "L":
            continue
        norm = unicodedata.normalize("NFKC", ch)
        if norm == ch:
            continue
        mapped = "".join(final.get(c, c) for c in norm)
        if not mapped.strip() or any(c.isspace() for c in mapped):
            continue
        if any(unicodedata.category(c)[0] != "L" for c in mapped):
            continue
        entries.append((cp, mapped))

entries.sort()
print("entries:", len(entries))

def rust_str(s):
    out = []
    for ch in s:
        if ch in '"\\':
            out.append("\\" + ch)
        elif 0x20 <= ord(ch) < 0x7F:
            out.append(ch)
        else:
            out.append("\\u{%04X}" % ord(ch))
    return "".join(out)

lines = []
lines.append("//! Decomposition of the Arabic presentation forms (Unicode blocks")
lines.append("//! `FB50–FDFF` and `FE70–FEFF`) into the Persian letters they display.")
lines.append("//!")
lines.append("//! Generated from Unicode `NFKC` decompositions, with the normalizer's own")
lines.append("//! `ك`→`ک`, `ي`→`ی` and `ۀ`→`ه` mappings applied, so a decomposed form and the")
lines.append("//! character the normalizer would have produced agree. Regenerate with")
lines.append("//! `scripts/gen_presentation_forms.py` when the Unicode version moves.")
lines.append("")
lines.append("/// Presentation form to its Persian spelling, sorted by code point.")
lines.append("pub(crate) const PRESENTATION_FORMS: &[(char, &str)] = &[")
for cp, mapped in entries:
    lines.append('    (\'\\u{%04X}\', "%s"),' % (cp, rust_str(mapped)))
lines.append("];")
lines.append("")
lines.append("/// The Persian spelling of a presentation form, or `None` for any other char.")
lines.append("#[inline]")
lines.append("pub(crate) fn expand(ch: char) -> Option<&'static str> {")
lines.append("    PRESENTATION_FORMS")
lines.append("        .binary_search_by_key(&ch, |&(form, _)| form)")
lines.append("        .ok()")
lines.append("        .map(|index| PRESENTATION_FORMS[index].1)")
lines.append("}")

path = r"C:\Users\MAK\Documents\rishe - Copy\src\presentation.rs"
with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(lines) + "\n")
print("wrote", path)
