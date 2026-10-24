#!/usr/bin/env python3
"""
Persian POS baselines on UD Persian.

    python scripts/benchmark_models.py

Hazm and Shekar are scored on the UD test sets. Shekar runs its own tokenizer,
so predictions are aligned to gold tokens and coverage is reported; Hazm is fed
the gold token list. Rishe rows appear once the compact model pack lands.

No accuracy claim ships without this table.
"""
import sys
import time
from collections import Counter
from difflib import SequenceMatcher
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.stdout.reconfigure(encoding="utf-8")

from benchmark import read
from conllu import parse

DATASETS = ("fa_seraji-ud-test.conllu", "fa_perdt-ud-test.conllu")


def load_ud_pos(name):
    """-> [(sentence_text, [(form, upos)])] using only integer-ID tokens."""
    sentences = []
    for sentence in parse(read(name)):
        gold = []
        for token in sentence:
            if isinstance(token["id"], tuple):
                continue
            upos = token.get("upos")
            if upos in (None, "_"):
                continue
            gold.append((token["form"], upos))
        if gold:
            sentences.append((sentence.metadata.get("text", "").strip(), gold))
    return sentences


def align(gold, predicted):
    """Match equal surface forms; returns (gold_index, predicted_index) pairs."""
    matcher = SequenceMatcher(
        a=[form for form, _ in gold],
        b=[form for form, _ in predicted],
        autojunk=False,
    )
    pairs = []
    for block in matcher.get_matching_blocks():
        for offset in range(block.size):
            pairs.append((block.a + offset, block.b + offset))
    return pairs


def scores(gold_tags, predicted_tags):
    correct = sum(a == b for a, b in zip(gold_tags, predicted_tags))
    accuracy = correct / len(gold_tags) if gold_tags else 0.0

    labels = sorted(set(gold_tags) | set(predicted_tags))
    f1_scores = []
    true_positive = false_positive = false_negative = 0
    for label in labels:
        tp = sum(g == label and p == label for g, p in zip(gold_tags, predicted_tags))
        fp = sum(g != label and p == label for g, p in zip(gold_tags, predicted_tags))
        fn = sum(g == label and p != label for g, p in zip(gold_tags, predicted_tags))
        true_positive += tp
        false_positive += fp
        false_negative += fn
        if tp:
            f1_scores.append(2 * tp / (2 * tp + fp + fn))
    micro = (
        2 * true_positive / (2 * true_positive + false_positive + false_negative)
        if true_positive
        else 0.0
    )
    macro = sum(f1_scores) / len(f1_scores) if f1_scores else 0.0
    return accuracy, micro, macro


def evaluate(dataset, library, sentences, tagger, prediction):
    gold_tags, predicted_tags = [], []
    matched = total = 0
    for text, gold in sentences[:3]:
        prediction(tagger, text, gold)
    started = time.perf_counter()
    for text, gold in sentences:
        predicted = prediction(tagger, text, gold)
        total += len(gold)
        for gold_index, predicted_index in align(gold, predicted):
            gold_tags.append(gold[gold_index][1])
            predicted_tags.append(predicted[predicted_index][1])
            matched += 1
    ms = (time.perf_counter() - started) / len(sentences) * 1000
    accuracy, micro, macro = scores(gold_tags, predicted_tags)
    coverage = matched / total if total else 0.0
    print(
        f"| {dataset} | {len(sentences)} | {library} | {coverage:.4f} | {accuracy:.4f} "
        f"| {micro:.4f} | {macro:.4f} | {ms:.2f} |"
    )
    return coverage, accuracy, micro, macro, ms


def hazm_prediction(tagger, _text, gold):
    return tagger.tag([form for form, _ in gold])


def shekar_prediction(tagger, text, _gold):
    return tagger.transform(text)


def load_peyma_ner():
    sentences, current = [], []
    for line in read("peyma_test.txt").splitlines():
        if line.strip():
            form, tag = line.split("|")[:2]
            current.append((form, tag))
        elif current:
            sentences.append(current)
            current = []
    if current:
        sentences.append(current)
    return sentences


def normalize_entity(text):
    return "".join(text.split()).replace("\u200c", "")


def gold_entities(tokens):
    entities, words, label = [], [], None

    def flush():
        nonlocal words, label
        if words and label:
            entities.append((" ".join(words), label))
        words, label = [], None

    for form, tag in tokens:
        if tag == "O":
            flush()
            continue
        current = tag[2:] if len(tag) > 2 and tag[1] in "_/-" else tag
        if tag.startswith("B") or not words or current != label:
            flush()
            label = current
        words.append(form)
    flush()
    return entities


def evaluate_ner(dataset, library, sentences, tagger):
    gold_set = Counter(
        (normalize_entity(text), label)
        for tokens in sentences
        for text, label in gold_entities(tokens)
    )
    predicted_set = Counter()
    texts = []
    for tokens in sentences:
        text = " ".join(form for form, _ in tokens)
        texts.append(text)
    for text in texts[:3]:
        tagger.transform(text)
    started = time.perf_counter()
    for text in texts:
        for entity, label in tagger.transform(text):
            predicted_set[(normalize_entity(entity), label)] += 1
    ms = (time.perf_counter() - started) / len(texts) * 1000

    matched = sum((gold_set & predicted_set).values())
    precision = matched / sum(predicted_set.values()) if predicted_set else 0.0
    recall = matched / sum(gold_set.values()) if gold_set else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    print(
        f"| {dataset} | {len(sentences)} | {library} | {sum(gold_set.values())} "
        f"| {precision:.4f} | {recall:.4f} | {f1:.4f} | {ms:.2f} |"
    )
    return precision, recall, f1, ms


def main():
    from hazm import POSTagger
    from shekar.ner import AlbertNER
    from shekar.pos import AlbertPOS

    print("# Persian model baselines\n")
    print("Gold tokenization for Hazm; Shekar runs its own tokenizer.")
    print("Coverage is the fraction of gold tokens aligned by surface form.\n")
    print("## POS (UD Persian)\n")
    print("| dataset | sentences | library | coverage | token acc | micro F1 | macro F1 | ms/sentence |")
    print("|---|---|---|---|---|---|---|---|")

    hazm = POSTagger(
        repo_id="roshan-research/hazm-postagger",
        model_filename="pos_tagger.model",
        universal_tag=True,
    )
    shekar = AlbertPOS()

    for dataset in DATASETS:
        name = "UD-Seraji" if "seraji" in dataset else "UD-PerDT"
        sentences = load_ud_pos(dataset)
        evaluate(name, "Hazm", sentences, hazm, hazm_prediction)
        evaluate(name, "Shekar", sentences, shekar, shekar_prediction)

    print("\n## NER (PEYMA test)\n")
    print("Entity-level exact match after stripping whitespace and ZWNJ.")
    print("Hazm is N/A: its NER wrapper needs an external spaCy model.\n")
    print("| dataset | sentences | library | gold entities | precision | recall | F1 | ms/sentence |")
    print("|---|---|---|---|---|---|---|---|")
    ner_sentences = load_peyma_ner()
    evaluate_ner("PEYMA", "Shekar", ner_sentences, AlbertNER())

    print("\nNotes:")
    print("- Hazm POS is a CRF over word/prev/next features, fed gold tokens.")
    print("- Shekar POS/NER are q8 ONNX ALBERT with their own tokenizer, so their rows are end-to-end.")
    print("- Latency is single-thread, models constructed once, first 3 calls warmed up.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
