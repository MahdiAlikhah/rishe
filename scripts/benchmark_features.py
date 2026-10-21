#!/usr/bin/env python3
"""
Rishe vs Hazm vs Shekar: P0 feature benchmarks.

Run: python scripts/benchmark_features.py

Only features with a comparable public API are scored. Hazm has no spell
checker or inflector; Shekar has no token splitter. Those rows say N/A.
"""
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8")

from benchmark import canon_lemma, load_ud, timeit

from hazm import Conjugation as HazmConjugation
from hazm import Lemmatizer as HazmLemmatizer
from hazm import Stemmer as HazmStemmer
from hazm import TokenSplitter as HazmTokenSplitter
from hazm import stopwords_list as hazm_stopwords
from rishe import Pipeline
from shekar import Conjugator as ShekarConjugator
from shekar import Inflector as ShekarInflector
from shekar import Lemmatizer as ShekarLemmatizer
from shekar import SpellChecker as ShekarSpellChecker
from shekar import Stemmer as ShekarStemmer
from shekar import data as shekar_data

ZWNJ = "\u200c"
ALPHABET = "آابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهی"


def pct(hit, total):
    return 100.0 * hit / total if total else 0.0


def flatten(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, (list, tuple, set)):
        for item in value:
            yield from flatten(item)


def main():
    pipe = Pipeline()
    hazm_stem = HazmStemmer()
    shekar_stem = ShekarStemmer()
    hazm_conj = HazmConjugation()
    shekar_conj = ShekarConjugator()
    shekar_inflect = ShekarInflector()
    hazm_split = HazmTokenSplitter()
    hazm_lemma = HazmLemmatizer()
    shekar_lemma = ShekarLemmatizer()
    shekar_spell = ShekarSpellChecker()

    seraji = load_ud("fa_seraji-ud-test.conllu")
    perdt = load_ud("fa_perdt-ud-test.conllu")

    print("# P0 feature benchmarks: Rishe vs Hazm vs Shekar")
    print("\nCaveat: there is no gold data for most of these features. The tables")
    print("report proxy accuracy, agreement, coverage, and speed; read them together.")

    # ---------------------------------------------------------------- stemming
    print("\n## Stemming (proxy: stem == UD gold lemma)\n")
    print("| dataset | words | library | exact lemma | lemma prefix | ms/word |")
    print("|---|---|---|---|---|---|")
    for name, data in (("UD-Seraji", seraji), ("UD-PerDT", perdt)):
        words = [
            (word, lemma)
            for _, _, rows in data
            for word, lemma in rows
            if any(ch.isalpha() for ch in word)
        ]
        stemmers = {
            "identity": lambda w: w,
            "Rishe": pipe.stem,
            "Hazm": hazm_stem.stem,
            "Shekar": lambda w: shekar_stem.transform(w),
        }
        for lib, fn in stemmers.items():
            exact = prefix = 0
            for word, gold in words:
                stem = canon_lemma(fn(word))
                lemma = canon_lemma(gold)
                if stem == lemma:
                    exact += 1
                if stem and lemma.startswith(stem):
                    prefix += 1
            ms = 0.0 if lib == "identity" else timeit(fn, [w for w, _ in words], reps=2)
            print(
                f"| {name} | {len(words)} | {lib} | {pct(exact, len(words)):.2f}% "
                f"| {pct(prefix, len(words)):.2f}% | {ms:.4f} |"
            )

    # ------------------------------------------------------------ conjugation
    verbs = [
        ("رفت", "رو"),
        ("کرد", "کن"),
        ("گفت", "گوی"),
        ("آمد", "آی"),
        ("داد", "ده"),
        ("بود", "باش"),
    ]
    expected = {
        "رفت": ["رفتم", "می‌روم", "بروم", "خواهم رفت", "رفته‌ام"],
        "کرد": ["کردم", "می‌کنم", "بکنم", "خواهم کرد", "کرده‌ام"],
        "گفت": ["گفتم", "می‌گویم", "بگویم", "خواهم گفت", "گفته‌ام"],
        "آمد": ["آمدم", "می‌آیم", "بیایم", "خواهم آمد", "آمده‌ام"],
        "داد": ["دادم", "می‌دهم", "بدهم", "خواهم داد", "داده‌ام"],
        "بود": ["بودم", "باشم", "خواهم بود", "بوده‌ام"],
    }

    def rishe_forms(word):
        forms = pipe.conjugate(word)
        return set() if forms is None else {f for values in forms.values() for f in values}

    def hazm_forms(pair):
        return set(hazm_conj.get_all(f"{pair[0]}#{pair[1]}"))

    def shekar_forms(pair):
        return set(
            flatten(shekar_conj.conjugate(past_stem=pair[0], present_stem=pair[1]))
        )

    library_forms = {
        "Rishe": {past: rishe_forms(past) for past, _ in verbs},
        "Hazm": {past: hazm_forms((past, present)) for past, present in verbs},
        "Shekar": {past: shekar_forms((past, present)) for past, present in verbs},
    }
    rishe_all = set().union(*library_forms["Rishe"].values())
    total_expected = sum(len(v) for v in expected.values())

    print("\n## Conjugation (spot-check + coverage)\n")
    print(
        "| library | forms (6 verbs) | spot-check | Rishe forms covered | ms/verb |"
    )
    print("|---|---|---|---|---|")
    for lib, by_verb in library_forms.items():
        forms = set().union(*by_verb.values())
        hits = sum(form in forms for values in expected.values() for form in values)
        coverage = pct(len(rishe_all & forms), len(rishe_all))
        if lib == "Rishe":
            ms = timeit(lambda pair: pipe.conjugate(pair[0]), verbs, reps=3)
        elif lib == "Hazm":
            ms = timeit(lambda pair: hazm_conj.get_all(f"{pair[0]}#{pair[1]}"), verbs, reps=3)
        else:
            ms = timeit(
                lambda pair: shekar_conj.conjugate(
                    past_stem=pair[0], present_stem=pair[1]
                ),
                verbs,
                reps=3,
            )
        print(
            f"| {lib} | {len(forms)} | {hits}/{total_expected} "
            f"| {coverage:.1f}% | {ms:.3f} |"
        )
    print("\nSpot-check is 29 hand-written forms (past/present/subjunctive/future/perfect).")
    print("Rishe forms covered = % of Rishe-generated forms also generated by that library.")

    # --------------------------------------------------------------- inflection
    adjectives = ["خوب", "بزرگ", "زیبا", "سریع", "مهم", "سخت", "آسان", "جدید", "قدیمی", "کوچک"]
    nouns = ["کتاب", "خانه", "دریا", "میز", "دانش", "شهر", "کار", "روز", "سال", "مردم"]
    comp = sum(pipe.comparative(a) == shekar_inflect.comparative(a) for a in adjectives)
    sup = sum(pipe.superlative(a) == shekar_inflect.superlative(a) for a in adjectives)
    plural = sum(pipe.plural(n) == shekar_inflect.plural(n) for n in nouns)
    possessive = sum(
        pipe.possessive(n, i) == shekar_inflect.possessives(n)[i]
        for n in nouns
        for i in range(6)
    )
    rishe_poss = lambda n: [pipe.possessive(n, i) for i in range(6)]

    print("\n## Inflection (Rishe vs Shekar; Hazm has no inflector)\n")
    print("| form | agreement | Rishe ms/op | Shekar ms/op |")
    print("|---|---|---|---|")
    print(
        f"| comparative | {pct(comp, len(adjectives)):.1f}% | "
        f"{timeit(pipe.comparative, adjectives, reps=100):.4f} | "
        f"{timeit(shekar_inflect.comparative, adjectives, reps=100):.4f} |"
    )
    print(
        f"| superlative | {pct(sup, len(adjectives)):.1f}% | "
        f"{timeit(pipe.superlative, adjectives, reps=100):.4f} | "
        f"{timeit(shekar_inflect.superlative, adjectives, reps=100):.4f} |"
    )
    print(
        f"| plural | {pct(plural, len(nouns)):.1f}% | "
        f"{timeit(pipe.plural, nouns, reps=100):.4f} | "
        f"{timeit(shekar_inflect.plural, nouns, reps=100):.4f} |"
    )
    print(
        f"| possessive (6 forms) | {pct(possessive, len(nouns) * 6):.1f}% | "
        f"{timeit(rishe_poss, nouns, reps=100):.4f} | "
        f"{timeit(shekar_inflect.possessives, nouns, reps=100):.4f} |"
    )
    print(
        "\nPossessive differences are Shekar's uniform ZWNJ before singular suffixes "
        "(کتاب‌م/خانه‌م); Rishe follows standard orthography (کتابم/خانه‌ام)."
    )

    # ------------------------------------------------------------ token split
    split_tokens = [
        "کتاب‌ها",
        "کتابهایم",
        "خانه‌ام",
        "بهش",
        "ازش",
        "می‌روم",
        "نمی‌خواهم",
        "داستان‌سرا",
        "صداوسیماجمهوری",
        "شهرموشها",
        "زبان‌شناسی",
        "کتابخانه",
        "بی‌ادب",
        "هم‌کلاس",
        "صبحانه",
    ]
    rishe_split = {t: pipe.split_token(t) for t in split_tokens}
    hazm_splits = {
        t: [tuple(part) for part in hazm_split.split_token_words(t)]
        for t in split_tokens
    }
    among = sum(tuple(rishe_split[t]) in hazm_splits[t] for t in split_tokens)
    first = sum(
        bool(hazm_splits[t]) and tuple(rishe_split[t]) == hazm_splits[t][0]
        for t in split_tokens
    )
    rishe_nonid = sum(len(rishe_split[t]) > 1 for t in split_tokens)
    hazm_nonid = sum(any(len(part) > 1 for part in hazm_splits[t]) for t in split_tokens)

    print("\n## Token splitting (Rishe vs Hazm; Shekar has no TokenSplitter)\n")
    print("| tokens | Rishe split | Hazm split | Rishe split among Hazm candidates | same first candidate | Rishe ms/token | Hazm ms/token |")
    print("|---|---|---|---|---|---|---|")
    print(
        f"| {len(split_tokens)} | {rishe_nonid} | {hazm_nonid} | "
        f"{among}/{len(split_tokens)} | {first}/{len(split_tokens)} | "
        f"{timeit(pipe.split_token, split_tokens, reps=100):.4f} | "
        f"{timeit(hazm_split.split_token_words, split_tokens, reps=20):.4f} |"
    )
    print("\nExamples:\n")
    print("| token | Rishe | Hazm candidates |")
    print("|---|---|---|")
    for token in split_tokens[:8]:
        print(f"| {token} | {rishe_split[token]} | {hazm_splits[token]} |")

    # ------------------------------------------------------------ spell check
    freq = {}
    with open(Path(__file__).resolve().parents[1] / "data" / "word_freq.tsv", encoding="utf-8") as f:
        for line in f:
            word, _, value = line.rstrip("\n").partition("\t")
            if word and value.isdigit():
                freq[word] = int(value)

    candidates = [
        word
        for word, count in freq.items()
        if count >= 1000
        and len(word) >= 4
        and all(ch.isalpha() or ch == ZWNJ for ch in word)
        and not pipe.is_stopword(word)
    ]
    random.seed(42)
    sampled = random.sample(candidates, min(500, len(candidates)))

    def make_typo(word):
        for _ in range(20):
            chars = list(word)
            op = random.choice(("delete", "transpose", "replace", "insert"))
            if op == "delete" and len(chars) > 3:
                del chars[random.randrange(len(chars))]
            elif op == "transpose" and len(chars) > 3:
                i = random.randrange(len(chars) - 1)
                chars[i], chars[i + 1] = chars[i + 1], chars[i]
            elif op == "replace":
                chars[random.randrange(len(chars))] = random.choice(ALPHABET)
            else:
                chars.insert(random.randrange(len(chars) + 1), random.choice(ALPHABET))
            typo = "".join(chars)
            if typo != word and typo not in freq and not pipe.is_stopword(typo):
                return typo
        return None

    pairs = [(word, make_typo(word)) for word in sampled]
    pairs = [(word, typo) for word, typo in pairs if typo]

    print(f"\n## Spell correction (synthetic 1-edit typos; {len(pairs)} words)\n")
    print("| library | top-1 recovers original | correct in top-5 | ms/word |")
    print("|---|---|---|---|")
    for lib, fn in (
        ("Rishe", lambda word: pipe.spell_suggest(word, 5)),
        ("Shekar", lambda word: shekar_spell.suggest(word, n_best=5)),
    ):
        top1 = any5 = 0
        for word, typo in pairs:
            suggestions = fn(typo)
            if suggestions and suggestions[0] == word:
                top1 += 1
            if word in suggestions:
                any5 += 1
        ms = timeit(fn, [typo for _, typo in pairs], reps=1)
        print(
            f"| {lib} | {pct(top1, len(pairs)):.1f}% | "
            f"{pct(any5, len(pairs)):.1f}% | {ms:.4f} |"
        )
    print("\nHazm has no spell checker. Typos are one random edit from a known word.")

    # --------------------------------------------------------------- stopwords
    rishe_sw = set(pipe.stopwords())
    hazm_sw = set(hazm_stopwords())
    shekar_sw = set(shekar_data.stopwords)

    print("\n## Stopword lists\n")
    print("| library | count | shared with Rishe | % of Rishe | Jaccard |")
    print("|---|---|---|---|---|")
    for name, words in (
        ("Rishe", rishe_sw),
        ("Hazm", hazm_sw),
        ("Shekar", shekar_sw),
    ):
        shared = len(rishe_sw & words)
        print(
            f"| {name} | {len(words)} | {shared} | "
            f"{pct(shared, len(rishe_sw)):.1f}% | "
            f"{pct(shared, len(rishe_sw | words)):.1f}% |"
        )

    # ---------------------------------------------------------- dual-stem lemma
    print("\n## Dual-stem lemma agreement (present stem; UD-Seraji unique words)\n")
    words = list(
        dict.fromkeys(
            word
            for _, _, rows in seraji
            for word, _ in rows
            if any(ch.isalpha() for ch in word)
        )
    )

    def hazm_present(word):
        value = hazm_lemma.lemmatize(word).replace(ZWNJ, "")
        return value.split("#", 1)[1] if "#" in value else None

    def shekar_present(word):
        value = next(iter(shekar_lemma.transform([word])), word).replace(ZWNJ, "")
        return value.split("/", 1)[1] if "/" in value else None

    rishe_p = {w: pipe.lemma_full(w)[1] for w in words}
    hazm_p = {w: hazm_present(w) for w in words}
    shekar_p = {w: shekar_present(w) for w in words}

    rishe_cov = sum(v is not None for v in rishe_p.values())
    hazm_cov = sum(v is not None for v in hazm_p.values())
    shekar_cov = sum(v is not None for v in shekar_p.values())
    all_three = [w for w in words if rishe_p[w] and hazm_p[w] and shekar_p[w]]

    print("| library | words with a present stem |")
    print("|---|---|")
    print(f"| Rishe | {rishe_cov}/{len(words)} ({pct(rishe_cov, len(words)):.1f}%) |")
    print(f"| Hazm | {hazm_cov}/{len(words)} ({pct(hazm_cov, len(words)):.1f}%) |")
    print(f"| Shekar | {shekar_cov}/{len(words)} ({pct(shekar_cov, len(words)):.1f}%) |")

    if all_three:
        rh = sum(rishe_p[w] == hazm_p[w] for w in all_three)
        rs = sum(rishe_p[w] == shekar_p[w] for w in all_three)
        hs = sum(hazm_p[w] == shekar_p[w] for w in all_three)
        print(f"\nOn the {len(all_three)} words where all three return a present stem:")
        print(f"- Rishe vs Hazm: {pct(rh, len(all_three)):.1f}%")
        print(f"- Rishe vs Shekar: {pct(rs, len(all_three)):.1f}%")
        print(f"- Hazm vs Shekar: {pct(hs, len(all_three)):.1f}%")

    # ------------------------------------------------------------- token spans
    sentences = [d[0] for d in perdt[:1000]]
    total = matched = 0
    for text in sentences:
        for token, start, end in pipe.tokenize_spans(text):
            total += 1
            if text[start:end] == token:
                matched += 1
    print("\n## Token spans (Rishe only; Python character offsets)\n")
    print("| sentences | tokens | correct slices |")
    print("|---|---|---|")
    print(f"| {len(sentences)} | {total} | {pct(matched, total):.2f}% |")


if __name__ == "__main__":
    main()
