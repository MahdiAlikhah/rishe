"""Deprecation helpers for the 0.11.x migration window.

Every alias added while the surface is renamed emits ``DeprecationWarning`` so
users see the change once, and the alias disappears at ``1.0.0``. See
``docs/API_CONVENTIONS.md`` rule 12.
"""

from __future__ import annotations

import functools
import warnings
from typing import Any, Callable, TypeVar

__all__ = ["deprecated"]

F = TypeVar("F", bound=Callable[..., Any])


def deprecated(old: str, new: str) -> Callable[[F], F]:
    """Mark ``old`` as a deprecated alias of ``new``."""

    def decorator(function: F) -> F:
        message = (
            f"{old} is deprecated and will be removed in rishe 1.0.0; "
            f"use {new} instead."
        )

        @functools.wraps(function)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            warnings.warn(message, DeprecationWarning, stacklevel=2)
            return function(*args, **kwargs)

        wrapper.__deprecated__ = message  # type: ignore[attr-defined]
        return wrapper  # type: ignore[return-value]

    return decorator
