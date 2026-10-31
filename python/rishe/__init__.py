"""
Rishe (ریشه): High-performance Persian NLP engine.
"""

from . import hub
from ._core import Embeddings, Entity, ModelPack, Pipeline, ProcessedDoc, TaggedToken
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
