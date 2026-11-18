"""Corpus readers for Persian NLP datasets.

The readers yield :class:`Sentence` objects and stream lazily, so large files
do not have to fit in memory. CoNLL-U (UD Persian), PEYMA and plain text are
supported; other corpus-specific formats can be added as needed.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterator, List, Optional

__all__ = [
    "Token",
    "Sentence",
    "ConlluReader",
    "PeymaReader",
    "PlainTextReader",
]


@dataclass
class Token:
    id: str
    form: str
    lemma: Optional[str] = None
    upos: Optional[str] = None
    xpos: Optional[str] = None
    feats: Dict[str, str] = field(default_factory=dict)
    head: Optional[int] = None
    deprel: Optional[str] = None
    misc: Dict[str, str] = field(default_factory=dict)


@dataclass
class Sentence:
    text: str
    tokens: List[Token] = field(default_factory=list)
    id: Optional[str] = None
    comments: Dict[str, str] = field(default_factory=dict)


class ConlluReader:
    """Read a CoNLL-U file (UD Persian and compatible treebanks)."""

    def __init__(self, path: str | Path):
        self.path = Path(path)

    def __iter__(self) -> Iterator[Sentence]:
        return self.sentences()

    def sentences(self) -> Iterator[Sentence]:
        tokens: List[Token] = []
        comments: Dict[str, str] = {}
        sent_id: Optional[str] = None
        skipped = 0

        # `utf-8-sig` so a byte-order mark does not hide the first `# sent_id`.
        with self.path.open(encoding="utf-8-sig") as handle:
            for line_no, raw in enumerate(handle, 1):
                line = raw.rstrip("\n")
                if not line.strip():
                    if tokens:
                        yield _sentence(tokens, comments, sent_id)
                    tokens, comments, sent_id = [], {}, None
                    continue
                if line.startswith("#"):
                    key, separator, value = line[1:].strip().partition("=")
                    key, value = key.strip(), value.strip() if separator else ""
                    comments[key] = value
                    if key == "sent_id":
                        sent_id = value
                    continue
                columns = line.split("\t")
                if len(columns) < 10:
                    # A corrupt row used to vanish silently, so a file with every
                    # row broken read as an empty corpus. One skipped line is
                    # reported on stderr; the rest of the file still reads.
                    skipped += 1
                    if skipped <= 5:
                        print(
                            f"warning: {self.path}:{line_no}: expected 10 columns, "
                            f"found {len(columns)}; skipping",
                            file=sys.stderr,
                        )
                    continue
                token_id, form, lemma, upos, xpos, feats, head, deprel, _, misc = columns[:10]
                # Multiword-token ranges (`2-3`) and empty nodes (`8.1`) repeat the
                # words of the integer-id rows; keeping them doubles the sentence.
                if "-" in token_id or "." in token_id:
                    continue
                tokens.append(
                    Token(
                        id=token_id,
                        form=form,
                        lemma=None if lemma == "_" else lemma,
                        upos=None if upos == "_" else upos,
                        xpos=None if xpos == "_" else xpos,
                        feats=_pairs(feats),
                        head=int(head) if head.lstrip("-").isdigit() else None,
                        deprel=None if deprel == "_" else deprel,
                        misc=_pairs(misc),
                    )
                )

        if tokens:
            yield _sentence(tokens, comments, sent_id)


class PeymaReader:
    """Read the PEYMA test format: pipe-separated lines, blank-line sentences."""

    def __init__(self, path: str | Path):
        self.path = Path(path)

    def __iter__(self) -> Iterator[Sentence]:
        return self.sentences()

    def sentences(self) -> Iterator[Sentence]:
        lines: List[str] = []
        # `utf-8-sig` so a byte-order mark does not become a leading token.
        with self.path.open(encoding="utf-8-sig") as handle:
            for raw in handle:
                line = raw.strip()
                if line:
                    lines.append(line.split("|")[0])
                elif lines:
                    yield Sentence(text=" ".join(lines))
                    lines = []
        if lines:
            yield Sentence(text=" ".join(lines))


class PlainTextReader:
    """Read one-sentence-per-line or one-paragraph-per-line plain text."""

    def __init__(self, path: str | Path):
        self.path = Path(path)

    def __iter__(self) -> Iterator[Sentence]:
        return self.sentences()

    def sentences(self) -> Iterator[Sentence]:
        # `utf-8-sig` so a byte-order mark does not become a leading token.
        with self.path.open(encoding="utf-8-sig") as handle:
            for raw in handle:
                line = raw.strip()
                if line:
                    yield Sentence(text=line)


def _sentence(
    tokens: List[Token], comments: Dict[str, str], sent_id: Optional[str]
) -> Sentence:
    text = comments.get("text") or " ".join(token.form for token in tokens)
    return Sentence(text=text, tokens=tokens, id=sent_id, comments=comments)


def _pairs(value: str) -> Dict[str, str]:
    if value == "_":
        return {}
    pairs: Dict[str, str] = {}
    for item in value.split("|"):
        key, separator, item_value = item.partition("=")
        pairs[key] = item_value if separator else ""
    return pairs
