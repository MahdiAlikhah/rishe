"""Sentence embeddings from a sentence-encoder pack.

    >>> from rishe import SentenceEmbedder, hub
    >>> encoder = SentenceEmbedder(hub.path("rishe-vector-e5-small-fa"))
    >>> vector = encoder.embed("این کتاب بسیار خوب است")
    >>> len(vector)
    384
    >>> round(encoder.similarity("این کتاب خوب است", "این کتاب بد است"), 2)
    0.95

This is a thin wrapper over :class:`rishe.ModelPack` that refuses a pack which
is not a sentence encoder, so a POS pack cannot be pooled into a vector that
looks like an embedding but means nothing.
"""

from __future__ import annotations

import os
from typing import List, Sequence, Tuple, Union

from ._core import ModelPack

__all__ = ["SentenceEmbedder"]

PathLike = Union[str, "os.PathLike[str]"]


class SentenceEmbedder:
    """A sentence-encoder pack, with the cosine helpers the vectors make cheap."""

    def __init__(self, path: PathLike) -> None:
        """Load the pack at ``path``.

        Raises ``ValueError`` when the pack is missing, malformed, or is not a
        sentence encoder (``task`` other than ``sentence``).
        """
        self._pack = ModelPack(path)
        if self._pack.task != "sentence":
            raise ValueError(
                f"pack at {path!s} is {self._pack.task!r}, not a sentence encoder; "
                "a POS, NER or reranker pack cannot produce vectors"
            )

    @property
    def task(self) -> str:
        """Task the pack was built for; always ``sentence``."""
        return self._pack.task

    @property
    def max_len(self) -> int:
        """Maximum number of subword tokens the model accepts."""
        return self._pack.max_len

    def embed(self, text: str) -> List[float]:
        """Return an L2-normalized sentence vector for ``text``."""
        return self._pack.sentence_vector(text)

    def embed_batch(self, texts: Sequence[str]) -> List[List[float]]:
        """Return one vector per text, in input order."""
        return self._pack.sentence_vectors(list(texts))

    def similarity(self, first: str, second: str) -> float:
        """Cosine similarity of two texts, as a plain dot product.

        The vectors are unit length, so no division is needed. Higher is more
        similar; the range is -1 to 1.
        """
        left, right = self.embed(first), self.embed(second)
        return sum(a * b for a, b in zip(left, right))

    def rank(
        self, query: str, candidates: Sequence[str]
    ) -> List[Tuple[float, str]]:
        """Return ``(score, text)`` pairs sorted by similarity, best first."""
        query_vector = self.embed(query)
        scored = [
            (sum(a * b for a, b in zip(query_vector, self.embed(text))), text)
            for text in candidates
        ]
        scored.sort(key=lambda item: item[0], reverse=True)
        return scored
