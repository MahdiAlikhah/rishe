#!/usr/bin/env python3
"""
Downloads Hazm's raw lexicons, merges Parsivar broken plurals (Mokassar) and colloquial
mappings, normalizes everything to canonical Persian, and writes `data/*.tsv` for `build.rs`.
"""

from pathlib import Path
import urllib.request

DATA_DIR = Path("data")
DATA_DIR.mkdir(exist_ok=True)

HAZM_WORDS_URL = "https://raw.githubusercontent.com/roshan-research/hazm/master/hazm/data/words.dat"
HAZM_VERBS_URL = "https://raw.githubusercontent.com/roshan-research/hazm/master/hazm/data/verbs.dat"
HAZM_INFORMAL_URL = (
    "https://raw.githubusercontent.com/roshan-research/hazm/master/hazm/data/iwords.dat"
)
POS_NOUN  = 1 << 0  # 0x01
POS_ADJ   = 1 << 1  # 0x02
POS_PROPN = 1 << 2  # 0x04
POS_PRON  = 1 << 3  # 0x08
POS_ADV   = 1 << 4  # 0x10
POS_NUM   = 1 << 5  # 0x20


def canonical_persian(text: str) -> str:
    text = text.strip()
    replacements = {
        "\u064a": "\u06cc",  # Arabic Yeh -> Persian Yeh (ی)
        "\u0649": "\u06cc",  # Alef Maksura -> Persian Yeh (ی)
        "\u0643": "\u06a9",  # Arabic Kaf -> Persian Keh (ک)
        "\u06c0": "\u0647",  # Heh with Yeh -> Heh (ه)
        "\u0640": "",        # Strip Tatweel
    }
    for old, new in replacements.items():
        text = text.replace(old, new)
    return "".join(
        ch for ch in text if not (0x064B <= ord(ch) <= 0x0652 or ord(ch) == 0x0670)
    )


def parse_pos_flags(raw_pos_str: str) -> int:
    flags = 0
    for tag in raw_pos_str.replace(";", ",").split(","):
        t = tag.strip().upper()
        if t == "N":
            flags |= POS_NOUN
        elif t in ("AJ", "ADJ"):
            flags |= POS_ADJ
        elif t in ("NE", "PROPN", "NP"):
            flags |= POS_PROPN | POS_NOUN
        elif t in ("PRO", "PRON"):
            flags |= POS_PRON
        elif t == "ADV":
            flags |= POS_ADV
        elif t == "NUM":
            flags |= POS_NUM
    return flags if flags > 0 else POS_NOUN


def download_if_missing(url: str, dest: Path) -> Path:
    if not dest.exists():
        print(f"Downloading {url} -> {dest}...")
        try:
            urllib.request.urlretrieve(url, dest)
        except Exception as e:
            print(
                f"  [warn] Could not download {url} ({e}). Proceeding with built-in lexicon."
            )
    return dest


def build_nouns_tsv(raw_words_path: Path, out_path: Path):
    word_map: dict[str, tuple[str, int]] = {}

    with open(raw_words_path, encoding="utf-8") as f:
        for line in f:
            parts = line.strip().split("\t")
            if not parts:
                continue
            word = canonical_persian(parts[0])
            if not word:
                continue
            raw_pos = parts[2] if len(parts) >= 3 else "N"
            flags = parse_pos_flags(raw_pos)
            word_map[word] = (word, flags)

    mokassar_mappings = {
        "کتب": "کتاب",
        "مدارس": "مدرسه",
        "مساجد": "مسجد",
        "منازل": "منزل",
        "مراکز": "مرکز",
        "منابع": "منبع",
        "موارد": "مورد",
        "مسائل": "مسئله",
        "شرایط": "شرط",
        "حقایق": "حقیقت",
        "دقایق": "دقیقه",
        "وقایع": "واقعه",
        "عوامل": "عامل",
        "قوانین": "قانون",
        "فرشتگان": "فرشته",
        "علما": "عالم",
        "شهدا": "شهید",
        "فقرا": "فقیر",
        "امرا": "امیر",
        "وزرا": "وزیر",
        "وکلا": "وکیل",
        "اطبا": "طبیب",
        "ادبا": "ادیب",
        "شعرا": "شاعر",
        "عرفا": "عارف",
        "عقلا": "عاقل",
        "غربا": "غریب",
        "افراد": "فرد",
        "اعضا": "عضو",
        "اجزا": "جزء",
        "اشیا": "شیء",
        "اسامی": "اسم",
        "اراضی": "ارض",
        "اهالی": "اهل",
        "اماکن": "مکان",
        "اوقات": "وقت",
        "اعمال": "عمل",
        "افعال": "فعل",
        "اقلام": "قلم",
        "اموال": "مال",
        "احوال": "حال",
        "اشکال": "شکل",
        "انواع": "نوع",
        "ابزار": "ابزار",
        "اسناد": "سند",
        "اشخاص": "شخص",
        "اهداف": "هدف",
        "امواج": "موج",
        "ارواح": "روح",
        "اجسام": "جسم",
        "ابعاد": "بعد",
        "اعداد": "عدد",
        "اقسام": "قسم",
        "احکام": "حکم",
        "افکار": "فکر",
        "اخبار": "خبر",
        "آثار": "اثر",
        "آداب": "ادب",
        "آیات": "آیه",
        "دفاتر": "دفتر",
        "مقاصد": "مقصد",
        "مظاهر": "مظهر",
        "مناظر": "منظره",
        "مباحث": "مبحث",
        "مفاهیم": "مفهوم",
        "مضامین": "مضمون",
        "مشاهیر": "مشهور",
        "تصاویر": "تصویر",
        "قوانین": "قانون",
        "تکالیف": "تکلیف",
        "تدابیر": "تدبیر",
        "تفاسیر": "تفسیر",
        "تغییرات": "تغییر",
        "روابط": "رابطه",
        "ضوابط": "ضابطه",
        "عواطف": "عاطفه",
        "عواقب": "عاقبت",
        "فواصل": "فاصله",
        "قواعد": "قاعده",
        "موانع": "مانع",
        "منافع": "منفعت",
        "جوامع": "جامعه",
        "سواحل": "ساحل",
        "عجایب": "عجیبه",
        "غرایب": "غریبه",
        "مصائب": "مصیبت",
        "جرایم": "جرم",
        "ضمایم": "ضمیمه",
        "علایم": "علامت",
        "غنایم": "غنیمت",
        "رسایل": "رساله",
        "وسایل": "وسیله",
        "فضایل": "فضیلت",
        "قبایل": "قبیله",
        "دلایل": "دلیل",
        "مسائل": "مسئله",
    }

    for plural, singular in mokassar_mappings.items():
        p_norm = canonical_persian(plural)
        s_norm = canonical_persian(singular)
        word_map[p_norm] = (s_norm, POS_NOUN)
        word_map.setdefault(s_norm, (s_norm, POS_NOUN))

    sorted_items = sorted(word_map.items(), key=lambda kv: kv[0].encode("utf-8"))
    with open(out_path, "w", encoding="utf-8") as out:
        for word, (lemma, flags) in sorted_items:
            out.write(f"{word}\t{lemma}\t{flags}\n")
    print(f"Wrote {len(sorted_items):,} nouns/mokassar entries to {out_path}")


def build_verbs_tsv(raw_verbs_path: Path, out_path: Path):
    stem_to_lemma: dict[str, str] = {}

    with open(raw_verbs_path, encoding="utf-8") as f:
        for line in f:
            line = canonical_persian(line)
            if not line or line.startswith("#") or "#" not in line:
                continue
            parts = line.split("#")
            past_stem = parts[0].strip()
            present_stem = parts[1].strip()
            if not past_stem:
                continue
            # UD-Seraji / UD-PerDT / Hazm standard: canonical verb lemma is the past_stem
            stem_to_lemma[past_stem] = past_stem
            if present_stem:
                # Only set present_stem if not already claimed by a past_stem
                stem_to_lemma.setdefault(present_stem, past_stem)

    # Priority overrides for high-frequency Persian verbs whose present stems
    # collide with rare past stems (e.g., کن -> کرد instead of کند)
    priority_stems = {
        "است": "بود",
        "هست": "بود",
        "نیست": "بود",
        "بود": "بود",
        "باش": "بود",
        "کن": "کرد",
        "کرد": "کرد",
        "شو": "شد",
        "شد": "شد",
        "ده": "داد",
        "داد": "داد",
        "رو": "رفت",
        "رفت": "رفت",
        "خواه": "خواست",
        "خواست": "خواست",
        "دار": "داشت",
        "داشت": "داشت",
        "گو": "گفت",
        "گوی": "گفت",
        "گفت": "گفت",
        "آ": "آمد",
        "آی": "آمد",
        "آمد": "آمد",
        "بر": "برد",
        "برد": "برد",
        "زن": "زد",
        "زد": "زد",
        "گیر": "گرفت",
        "گرفت": "گرفت",
        "بین": "دید",
        "دید": "دید",
        "دان": "دانست",
        "دانست": "دانست",
        "توان": "توانست",
        "توانست": "توانست",
    }
    stem_to_lemma.update(priority_stems)

    sorted_items = sorted(
        stem_to_lemma.items(), key=lambda kv: kv[0].encode("utf-8")
    )
    with open(out_path, "w", encoding="utf-8") as out:
        for stem, lemma in sorted_items:
            out.write(f"{stem}\t{lemma}\t128\n")
    print(
        f"Wrote {len(sorted_items):,} verb stems (UD past-stem format) to {out_path}"
    )


def build_informal_tsvs(
    raw_informal_path: Path, words_out: Path, verbs_out: Path, raw_words_path: Path
):
    informal_map: dict[str, str] = {
        "اون": "آن",
        "اونا": "آن\u200cها",
        "اینا": "این\u200cها",
        "همون": "همان",
        "واسه": "برای",
        "اگه": "اگر",
        "مگه": "مگر",
        "دیگه": "دیگر",
        "کدوم": "کدام",
        "چیه": "چیست",
        "کیه": "کیست",
        "داره": "دارد",
        "دارن": "دارند",
        "دارین": "دارید",
        "نیستش": "نیست",
        "هستش": "هست",
        "میشه": "می\u200cشود",
        "نمیشه": "نمی\u200cشود",
        "میخوام": "می\u200cخواهم",
        "می\u200cخوام": "می\u200cخواهم",
        "میخوای": "می\u200cخواهی",
        "میخواد": "می\u200cخواهد",
        "میخوان": "می\u200cخواهند",
    }

    if raw_informal_path.exists():
        with open(raw_informal_path, encoding="utf-8") as f:
            for line in f:
                line = canonical_persian(line)
                if not line or line.startswith("#"):
                    continue
                parts = line.split()
                if len(parts) >= 2:
                    informal_map[parts[0]] = parts[1]

    # Informal forms are looked up before the noun lexicon, so drop keys that are
    # real, common words (تو, خان, رو): corpus freq within 8x of the target's.
    freq: dict[str, int] = {}
    with open(raw_words_path, encoding="utf-8") as f:
        for line in f:
            parts = line.strip().split("	")
            if len(parts) >= 2 and parts[1].isdigit():
                w = canonical_persian(parts[0])
                freq[w] = max(freq.get(w, 0), int(parts[1]))
    informal_map = {
        k: v
        for k, v in informal_map.items()
        if freq.get(k, 0) * 8 < freq.get(v, 0) or freq.get(k, 0) == 0
    }

    sorted_words = sorted(informal_map.items(), key=lambda kv: kv[0].encode("utf-8"))
    with open(words_out, "w", encoding="utf-8") as out:
        for inf, form in sorted_words:
            out.write(f"{inf}\t{form}\t1\n")

    informal_verb_stems = {
        "گ": "گوی",
        "د": "ده",
        "ر": "رو",
        "خوا": "خواه",
        "ذار": "گذار",
        "ذشت": "گذشت",
        "شین": "نشین",
        "شست": "نشست",
        "تون": "توان",
        "دون": "دان",
        "خون": "خوان",
        "مون": "مان",
    }
    sorted_verbs = sorted(informal_verb_stems.items(), key=lambda kv: kv[0].encode("utf-8"))
    with open(verbs_out, "w", encoding="utf-8") as out:
        for inf_stem, form_stem in sorted_verbs:
            out.write(f"{inf_stem}\t{form_stem}\t128\n")

    print(f"Wrote {len(sorted_words):,} informal words and {len(sorted_verbs):,} informal verb stems.")


if __name__ == "__main__":
    raw_words = download_if_missing(HAZM_WORDS_URL, DATA_DIR / "raw_hazm_words.dat")
    raw_verbs = download_if_missing(HAZM_VERBS_URL, DATA_DIR / "raw_hazm_verbs.dat")
    raw_inf = download_if_missing(HAZM_INFORMAL_URL, DATA_DIR / "raw_hazm_informal.dat")

    build_nouns_tsv(raw_words, DATA_DIR / "nouns.tsv")
    build_verbs_tsv(raw_verbs, DATA_DIR / "verbs.tsv")
    build_informal_tsvs(raw_inf, DATA_DIR / "informal_words.tsv", DATA_DIR / "informal_verbs.tsv", raw_words)