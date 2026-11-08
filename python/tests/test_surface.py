"""Mechanical checks for ``docs/API_CONVENTIONS.md``.

Each set below is the *complete* list of current violations of one rule. Fixing
a violation means deleting it from the set, so the allowlists can only shrink.
"""

import re
import unittest
from pathlib import Path

STUB = Path(__file__).resolve().parents[1] / "rishe" / "_core.pyi"

BANNED_SUFFIXES = ("_with", "_full", "_stages", "_char_spans")
# Rule 1: no sibling methods that differ only by an option bundle.
# These are the deprecated aliases kept until 1.0.0; the set may not grow.
SUFFIX_VIOLATIONS = {
    "word_tokenize_with",
    "lemma_full",
    "process_stages",
    "normalize_stages",
    "mask_stages",
}

# Rule 3: one name for "how many". Empty since 0.11.0 renamed the parameters;
# keep the mechanism so a new violation fails the build.
COUNT_PARAMETER_VIOLATIONS = set()

# Rule 4: every result class exposes ``label``. Empty since 0.11.0 added it to
# all three result types; the mechanism stays so a regression fails the build.
LABEL_VIOLATIONS = set()

# Rule 2: batch calls end with ``_batch`` unless the name is a plural noun.
BATCH_EXCEPTIONS = {"sentence_vectors"}
# ``pipe`` is the deprecated alias of ``process_batch``; removed in 1.0.0.
BATCH_VIOLATIONS = {"pipe"}

RESULT_CLASSES = ("TaggedToken", "Entity", "Dependency")


def parse_stub():
    """Return ``(owner, name, signature)`` for every stub entry, flattened."""
    text = re.sub(r"\s+", " ", STUB.read_text(encoding="utf-8"))
    owners = [(match.start(), match.group(1)) for match in re.finditer(r"class (\w+)", text)]
    entries = []
    for match in re.finditer(r"def (\w+)\(([^)]*)\)", text):
        owner = None
        for position, name in owners:
            if position < match.start():
                owner = name
            else:
                break
        entries.append((owner, match.group(1), match.group(2)))
    return entries


class SurfaceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.entries = parse_stub()

    def test_stub_is_parsed(self):
        self.assertGreater(len(self.entries), 50)

    def test_no_retired_suffixes(self):
        found = {name for _, name, _ in self.entries if name.endswith(BANNED_SUFFIXES)}
        self.assertEqual(
            found,
            SUFFIX_VIOLATIONS,
            f"new={sorted(found - SUFFIX_VIOLATIONS)} "
            f"fixed={sorted(SUFFIX_VIOLATIONS - found)}",
        )

    def test_count_parameters_are_named_n(self):
        found = set()
        for _, name, signature in self.entries:
            for parameter in re.findall(r"\b(top_n|max)\b", signature):
                found.add(f"{name}:{parameter}")
        self.assertEqual(
            found,
            COUNT_PARAMETER_VIOLATIONS,
            f"new={sorted(found - COUNT_PARAMETER_VIOLATIONS)} "
            f"fixed={sorted(COUNT_PARAMETER_VIOLATIONS - found)}",
        )

    def test_result_classes_expose_label(self):
        methods = {}
        for owner, name, _ in self.entries:
            methods.setdefault(owner, set()).add(name)
        found = set()
        for result in RESULT_CLASSES:
            names = methods.get(result, set())
            if "label" in names:
                continue
            for alias in ("tag", "relation"):
                if alias in names:
                    found.add(f"{result}.{alias}")
        self.assertEqual(
            found,
            LABEL_VIOLATIONS,
            f"new={sorted(found - LABEL_VIOLATIONS)} "
            f"fixed={sorted(LABEL_VIOLATIONS - found)}",
        )

    def test_batch_methods_use_the_batch_suffix(self):
        found = {
            name
            for _, name, signature in self.entries
            if re.search(r"\btexts\b", signature)
            and not name.endswith("_batch")
            and name not in BATCH_EXCEPTIONS
        }
        self.assertEqual(
            found,
            BATCH_VIOLATIONS,
            f"new={sorted(found - BATCH_VIOLATIONS)} "
            f"fixed={sorted(BATCH_VIOLATIONS - found)}",
        )


if __name__ == "__main__":
    unittest.main()
