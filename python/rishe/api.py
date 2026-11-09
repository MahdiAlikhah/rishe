"""Module-level convenience functions over a shared default pipeline.

    >>> import rishe
    >>> rishe.tokenize("سلام دنیا")
    ['سلام', 'دنیا']

Every function here uses one lazily created :class:`rishe.Pipeline`, so the first
call pays the setup cost once and later calls reuse it. Pass your own ``Pipeline``
when you need model packs, embeddings or custom normalization options.
"""

from __future__ import annotations

import threading
from typing import List, Optional, Sequence, Tuple

from ._core import Pipeline, ProcessedDoc
from ._deprecation import deprecated
from .compat import word_tokenize as _compat_word_tokenize

__all__ = [
    "DEFAULT_MASKERS",
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
    "word_tokenize",
]

#: Masker names applied by :func:`mask` when ``maskers`` is omitted, in order.
DEFAULT_MASKERS: Tuple[str, ...] = (
    "url",
    "email",
    "mention",
    "hashtag",
    "emoji",
    "html",
    "non_persian",
    "digits",
    "diacritics",
    "punctuation",
)

#: Keyword arguments accepted by :func:`tokenize`.
TOKENIZE_OPTIONS: Tuple[str, ...] = (
    "join_verb_parts",
    "join_abbreviations",
    "separate_emoji",
    "replace_links",
    "replace_ids",
    "replace_emails",
    "replace_numbers",
    "replace_hashtags",
)

_lock = threading.Lock()
_default: Optional[Pipeline] = None


def pipeline() -> Pipeline:
    """Return the shared default :class:`~rishe.Pipeline`.

    The instance is created on first use and reused afterwards; it is safe to
    call from several threads.
    """
    global _default
    if _default is None:
        with _lock:
            if _default is None:
                _default = Pipeline()
    return _default


def normalize(text: str, stages: Optional[Sequence[str]] = None) -> str:
    """Normalize Persian text.

    ``stages`` restricts or orders the normalizer steps; omit it for the default
    pipeline.
    """
    return pipeline().normalize(text, None if stages is None else list(stages))


def tokenize(text: str, **options: bool) -> List[str]:
    """Tokenize Persian text into surface words.

    This does not normalize. Call :func:`normalize` first, or use
    :func:`process` (``rishe.process(text).tokens``), which runs the whole
    pipeline. Keyword arguments toggle word-tokenizer options:

    ``join_verb_parts``, ``join_abbreviations``, ``separate_emoji``,
    ``replace_links``, ``replace_ids``, ``replace_emails``,
    ``replace_numbers`` and ``replace_hashtags``.
    """
    unknown = sorted(set(options) - set(TOKENIZE_OPTIONS))
    if unknown:
        raise TypeError(f"tokenize() got an unexpected keyword argument {unknown[0]!r}")
    return pipeline().tokenize(text, **options)


def tokenize_spans(text: str) -> List[Tuple[str, int, int]]:
    """Tokenize into ``(token, start, end)`` character spans."""
    return pipeline().tokenize_spans(text)


def sent_tokenize(text: str) -> List[str]:
    """Split Persian text into sentences."""
    return pipeline().sent_tokenize(text)


def process(text: str, stages: Optional[Sequence[str]] = None) -> ProcessedDoc:
    """Run the full pipeline and return a :class:`~rishe.ProcessedDoc`."""
    return pipeline().process(text, None if stages is None else list(stages))


def process_batch(texts: Sequence[str]) -> List[ProcessedDoc]:
    """Run the full pipeline over many inputs, returning one doc per input."""
    return pipeline().process_batch(list(texts))


def lemmatize(word: str) -> str:
    """Return the lemma of a single word."""
    return pipeline().lemmatize(word)


def stem(word: str) -> str:
    """Return the stem of a single word."""
    return pipeline().stem(word)


def mask(
    text: str,
    maskers: Optional[Sequence[str]] = None,
    mask_token: Optional[str] = None,
) -> str:
    """Mask or remove sensitive parts of ``text``.

    ``maskers`` selects and orders the masker names (``url``, ``email``,
    ``mention``, ``hashtag``, ``emoji``, ``html``, ``non_persian``, ``digits``,
    ``diacritics``, ``punctuation``); it defaults to :data:`DEFAULT_MASKERS`.
    ``mask_token`` overrides the replacement string of every masker.
    """
    names = list(DEFAULT_MASKERS if maskers is None else maskers)
    return pipeline().mask(text, names, mask_token)


def keywords(
    text: str, n: int = 5, method: str = "rake", max_length: int = 3
) -> List[str]:
    """Extract up to ``n`` keywords with ``method`` (``"rake"`` or ``"textrank"``)."""
    pipe = pipeline()
    if method == "rake":
        return pipe.rake(text, n, max_length)
    if method == "textrank":
        return pipe.textrank(text, n, max_length)
    raise ValueError(f"unknown keyword method {method!r}; use 'rake' or 'textrank'")


@deprecated("rishe.word_tokenize", "rishe.tokenize")
def word_tokenize(text: str) -> List[str]:
    """Deprecated alias that keeps the 0.10 behaviour: normalize, then join verb parts."""
    return _compat_word_tokenize(text)
