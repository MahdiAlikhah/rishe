"""
Rishe (ریشه): High-performance Persian NLP engine.
"""

from typing import Optional

from . import hub
from ._core import (
    Dependency,
    Embeddings,
    Entity,
    JalaliDate,
    ModelPack,
    Pipeline,
    ProcessedDoc,
    TaggedToken,
)
from .api import (
    DEFAULT_MASKERS,
    keywords,
    lemmatize,
    mask,
    normalize,
    pipeline,
    process,
    process_batch,
    sent_tokenize,
    stem,
    tokenize,
    tokenize_spans,
    word_tokenize,
)
from .compat import (
    Lemmatizer,
    Normalizer,
    SentenceTokenizer,
    Stemmer,
    TokenSplitter,
    WordTokenizer,
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
    "Dependency",
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
    "pipeline",
    "normalize",
    "tokenize",
    "tokenize_spans",
    "sent_tokenize",
    "process",
    "process_batch",
    "lemmatize",
    "stem",
    "mask",
    "keywords",
    "DEFAULT_MASKERS",
    "word_tokenize",
]


def parse_date(text: str) -> Optional[JalaliDate]:
    """Parse a Jalali date from Persian or numeric text."""
    return JalaliDate.parse(text)
