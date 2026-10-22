"""Corpus readers for Persian NLP datasets.

The readers yield :class:`Sentence` objects and stream lazily, so large files
do not have to fit in memory. CoNLL-U (UD Persian), PEYMA and plain text are
supported; other corpus-specific formats can be added as needed.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterator, List, Optional

__all__ = [
    "Token",
    "Sentence",
    "ConlluReader",
    "PeymaReader",
    "PlainTextReader",
    "read_conllu",
    "read_peyma",
    "read_text",
]


@dataclass(slots=True)
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


@dataclass(slots=True)
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

        with self.path.open(encoding="utf-8") as handle:
            for raw in handle:
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
                    continue
                token_id, form, lemma, upos, xpos, feats, head, deprel, _, misc = columns[:10]
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
        with self.path.open(encoding="utf-8") as handle:
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
        with self.path.open(encoding="utf-8") as handle:
            for raw in handle:
                line = raw.strip()
                if line:
                    yield Sentence(text=line)


def read_conllu(path: str | Path) -> List[Sentence]:
    return list(ConlluReader(path))


def read_peyma(path: str | Path) -> List[Sentence]:
    return list(PeymaReader(path))


def read_text(path: str | Path) -> List[Sentence]:
    return list(PlainTextReader(path))


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
