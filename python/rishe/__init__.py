"""
Rishe (ریشه): High-performance Persian NLP engine.
"""

from typing import Optional

from . import hub
from ._core import (
    Embeddings,
    Entity,
    JalaliDate,
    ModelPack,
    Pipeline,
    ProcessedDoc,
    TaggedToken,
)
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
from .corpora import ConlluReader, PeymaReader, PlainTextReader, Sentence, Token

__all__ = [
    "Pipeline",
    "ProcessedDoc",
    "JalaliDate",
    "parse_date",
    "Embeddings",
    "ModelPack",
    "TaggedToken",
    "Entity",
    "hub",
    "Token",
    "Sentence",
    "ConlluReader",
    "PeymaReader",
    "PlainTextReader",
    "Normalizer",
    "WordTokenizer",
    "SentenceTokenizer",
    "Lemmatizer",
    "Stemmer",
    "TokenSplitter",
    "word_tokenize",
    "sent_tokenize",
]


def parse_date(text: str) -> Optional[JalaliDate]:
    """Parse a Jalali date from Persian or numeric text."""
    return JalaliDate.parse(text)
