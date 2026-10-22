"""Hazm-compatible wrappers.

These classes let code written against Hazm migrate with an import change:

    from hazm import Lemmatizer, Normalizer, WordTokenizer
    ->
    from rishe import Lemmatizer, Normalizer, WordTokenizer

The wrappers delegate to a shared :class:`rishe.Pipeline`. They are a migration
aid, not a byte-for-byte clone: Rishe compiles its lexicons in, so Hazm's
``words_file``/``verbs_file`` constructor arguments are rejected rather than
silently ignored.
"""

from __future__ import annotations

import unicodedata
from typing import List, Optional, Tuple

from ._core import Pipeline

__all__ = [
    "Normalizer",
    "WordTokenizer",
    "SentenceTokenizer",
    "Lemmatizer",
    "Stemmer",
    "TokenSplitter",
    "word_tokenize",
    "sent_tokenize",
]

_pipeline: Optional[Pipeline] = None


def _pipe() -> Pipeline:
    global _pipeline
    if _pipeline is None:
        _pipeline = Pipeline()
    return _pipeline


class Normalizer:
    """Hazm-style normalizer backed by Rishe's normalizer operators."""

    def __init__(
        self,
        correct_spacing: bool = True,
        remove_diacritics: bool = True,
        remove_specials_chars: bool = True,
        decrease_repeated_chars: bool = True,
        persian_style: bool = True,
        persian_numbers: bool = True,
        unicodes_replacement: bool = True,
        seperate_mi: bool = True,
    ) -> None:
        self._correct_spacing = correct_spacing
        self._remove_diacritics = remove_diacritics
        self._remove_specials_chars = remove_specials_chars
        self._decrease_repeated_chars = decrease_repeated_chars
        self._persian_style = persian_style
        self._persian_numbers = persian_numbers
        self._unicodes_replacement = unicodes_replacement
        self._seperate_mi = seperate_mi
        self._pipe = Pipeline(strip_diacritics=remove_diacritics)

    def normalize(self, text: str) -> str:
        text = self._pipe.normalize(text)
        stages: List[str] = []
        if self._unicodes_replacement:
            stages.extend(["alphabet", "arabic_unicode"])
        if self._persian_style:
            stages.append("punctuation")
        if self._persian_numbers:
            stages.append("digits")
        if self._correct_spacing:
            stages.append("spacing")
        if self._decrease_repeated_chars:
            stages.append("repeated_letters")
        if self._seperate_mi:
            stages.append("verbal_spacing")
        if stages:
            text = self._pipe.normalize_stages(text, stages)
        if self._remove_specials_chars:
            text = self.remove_specials_chars(text)
        return text

    def correct_spacing(self, text: str) -> str:
        return self._pipe.normalize_stages(text, ["spacing"])

    def remove_diacritics(self, text: str) -> str:
        return self._pipe.mask_stages(text, ["diacritics"])

    def remove_specials_chars(self, text: str) -> str:
        return "".join(
            ch
            for ch in text
            if ch == "\u200c" or not unicodedata.category(ch).startswith(("S", "C"))
        )

    def decrease_repeated_chars(self, text: str) -> str:
        return self._pipe.normalize_stages(text, ["repeated_letters"])

    def persian_style(self, text: str) -> str:
        return self._pipe.normalize_stages(text, ["punctuation"])

    def persian_number(self, text: str) -> str:
        return self._pipe.normalize_stages(text, ["digits"])

    def unicodes_replacement(self, text: str) -> str:
        return self._pipe.normalize_stages(text, ["alphabet", "arabic_unicode"])

    def seperate_mi(self, text: str) -> str:
        return self._pipe.normalize_stages(text, ["verbal_spacing"])

    def token_spacing(self, tokens: List[str]) -> List[str]:
        joined = self._pipe.normalize_stages(
            " ".join(tokens), ["word_spacing", "verbal_spacing"]
        )
        return joined.split()


class WordTokenizer:
    """Hazm-style word tokenizer with the same option names."""

    def __init__(
        self,
        words_file: Optional[str] = None,
        verbs_file: Optional[str] = None,
        join_verb_parts: bool = True,
        join_abbreviations: bool = False,
        separate_emoji: bool = False,
        replace_links: bool = False,
        replace_ids: bool = False,
        replace_emails: bool = False,
        replace_numbers: bool = False,
        replace_hashtags: bool = False,
    ) -> None:
        if words_file is not None or verbs_file is not None:
            raise NotImplementedError(
                "Rishe compiles its lexicons in; custom words_file/verbs_file "
                "are not supported."
            )
        self.join_verb_parts = join_verb_parts
        self.join_abbreviations = join_abbreviations
        self.separate_emoji = separate_emoji
        self.replace_links = replace_links
        self.replace_ids = replace_ids
        self.replace_emails = replace_emails
        self.replace_numbers = replace_numbers
        self.replace_hashtags = replace_hashtags
        self._pipe = _pipe()

    def tokenize(self, text: str) -> List[str]:
        return self._pipe.word_tokenize_with(
            text,
            join_verb_parts=self.join_verb_parts,
            join_abbreviations=self.join_abbreviations,
            separate_emoji=self.separate_emoji,
            replace_links=self.replace_links,
            replace_ids=self.replace_ids,
            replace_emails=self.replace_emails,
            replace_numbers=self.replace_numbers,
            replace_hashtags=self.replace_hashtags,
        )

    def __call__(self, text: str) -> List[str]:
        return self.tokenize(text)


class SentenceTokenizer:
    def __init__(self) -> None:
        self._pipe = _pipe()

    def tokenize(self, text: str) -> List[str]:
        return self._pipe.sent_tokenize(text)

    def __call__(self, text: str) -> List[str]:
        return self.tokenize(text)


class Lemmatizer:
    def __init__(
        self,
        words_file: Optional[str] = None,
        verbs_file: Optional[str] = None,
        joined_verb_parts: bool = True,
    ) -> None:
        if words_file is not None or verbs_file is not None:
            raise NotImplementedError(
                "Rishe compiles its lexicons in; custom words_file/verbs_file "
                "are not supported."
            )
        self._pipe = _pipe()

    def lemmatize(self, word: str, pos: str = "") -> str:
        lemma, present = self._pipe.lemma_full(word)
        return f"{lemma}#{present}" if present else lemma


class Stemmer:
    def __init__(self, words_file: Optional[str] = None) -> None:
        if words_file is not None:
            raise NotImplementedError(
                "Rishe compiles its lexicons in; custom words_file is not supported."
            )
        self._pipe = _pipe()

    def stem(self, word: str) -> str:
        return self._pipe.stem(word)


class TokenSplitter:
    def __init__(self) -> None:
        self._pipe = _pipe()

    def split_token_words(self, token: str) -> List[Tuple[str, ...]]:
        parts = self._pipe.split_token(token)
        return [tuple(parts)] if len(parts) > 1 else []


def word_tokenize(text: str) -> List[str]:
    return WordTokenizer().tokenize(text)


def sent_tokenize(text: str) -> List[str]:
    return SentenceTokenizer().tokenize(text)
