"""
Rishe (ریشه): High-performance Persian NLP engine.
"""

from collections.abc import Iterable

from . import hub
from ._core import Embeddings, Pipeline, ProcessedDoc
from .compat import (
    Lemmatizer,
    Normalizer,
    SentenceTokenizer,
    Stemmer,
    TokenSplitter,
    WordTokenizer,
    sent_tokenize,
    word_tokenize,
)
from .corpora import (
    ConlluReader,
    PeymaReader,
    PlainTextReader,
    Sentence,
    Token,
    read_conllu,
    read_peyma,
    read_text,
)

__all__ = [
    "Pipeline",
    "ProcessedDoc",
    "Embeddings",
    "hub",
    "flatten",
    "Token",
    "Sentence",
    "ConlluReader",
    "PeymaReader",
    "PlainTextReader",
    "read_conllu",
    "read_peyma",
    "read_text",
    "Normalizer",
    "WordTokenizer",
    "SentenceTokenizer",
    "Lemmatizer",
    "Stemmer",
    "TokenSplitter",
    "word_tokenize",
    "sent_tokenize",
]


def flatten(items: Iterable) -> Iterable[str]:
    """Yield strings from a nested iterable, preserving order."""
    for item in items:
        if isinstance(item, str):
            yield item
        elif isinstance(item, Iterable) and not isinstance(item, (str, bytes)):
            yield from flatten(item)
