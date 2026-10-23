"""Model download and cache.

Downloads are explicit; importing or using Rishe never touches the network.
The cache lives in ``$RISHE_HOME/models`` or ``~/.cache/rishe/models``.

    >>> from rishe import hub
    >>> path = hub.pull("hazm-fasttext-300")
    >>> hub.cached("hazm-fasttext-300") == path
    True
"""

from __future__ import annotations

import hashlib
import os
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path
from typing import Dict, NamedTuple, Optional
from urllib.parse import urlparse

__all__ = ["MODELS", "cache_dir", "cached", "path", "pull"]


class Model(NamedTuple):
    url: str
    sha256: str
    filename: str


MODELS: Dict[str, Model] = {
    "hazm-fasttext-300": Model(
        url="https://huggingface.co/roshan-research/hazm-word-embedding/resolve/main/fasttext_skipgram_300.vec",
        sha256="43054876a1a3a62a067ad51304957f5f69fbca1620ca2fe610da6d4c9c8d2d04",
        filename="hazm-fasttext-300.vec",
    ),
}


def cache_dir() -> Path:
    """Directory where downloaded models live."""
    root = os.environ.get("RISHE_HOME")
    base = Path(root) if root else Path.home() / ".cache" / "rishe"
    return base / "models"


def cached(name: str) -> Optional[Path]:
    """Return the local path of a cached model, or ``None``."""
    model = MODELS.get(name)
    if model is None:
        raise ValueError(f"unknown model {name!r}; known models: {', '.join(MODELS)}")
    target = cache_dir() / Path(model.filename).name
    return target if target.exists() else None


def path(name: str) -> Path:
    """Return the local path of a cached model, raising when it was not pulled."""
    target = cached(name)
    if target is None:
        raise FileNotFoundError(
            f"{name!r} is not cached; run: rishe hub pull {name} "
            f"(or rishe.hub.pull({name!r}))"
        )
    return target


def pull(
    name_or_url: str,
    *,
    sha256: Optional[str] = None,
    filename: Optional[str] = None,
    force: bool = False,
) -> Path:
    """Download a known model (or a custom URL) into the cache and return its path.

    Custom URLs must pass ``sha256``; the checksum is verified before the file
    is moved into place, so an interrupted or tampered download is never used.
    """
    model = MODELS.get(name_or_url)
    if model is None:
        if not sha256:
            raise ValueError(
                f"unknown model {name_or_url!r}; known models: {', '.join(MODELS)}. "
                "Pass sha256= to pull a custom URL."
            )
        source = name_or_url
        inferred = filename or Path(urlparse(source).path).name
        safe_name = Path(inferred).name
        if not safe_name or safe_name in {".", ".."}:
            raise ValueError("cannot infer a filename from the URL; pass filename=")
        model = Model(url=source, sha256=sha256.lower(), filename=safe_name)

    if len(model.sha256) != 64:
        raise ValueError("sha256 must be a 64-character hex digest")

    target = cache_dir() / Path(model.filename).name
    if target.exists() and not force:
        return target
    cache_dir().mkdir(parents=True, exist_ok=True)

    handle, temporary = tempfile.mkstemp(
        dir=cache_dir(), prefix=f".{model.filename}.", suffix=".part"
    )
    # ponytail: an interrupted download restarts from zero; add Range resume
    # when model sizes keep growing or the CDN proves unreliable.
    digest = hashlib.sha256()
    total = 0
    try:
        with os.fdopen(handle, "wb") as output:
            request = urllib.request.Request(model.url, headers={"User-Agent": "rishe"})
            try:
                response = urllib.request.urlopen(request, timeout=60)
            except urllib.error.URLError as error:
                raise RuntimeError(f"download failed: {error}") from error
            with response:
                total = int(response.headers.get("Content-Length") or 0)
                done = 0
                next_report = 64 << 20
                while True:
                    chunk = response.read(1 << 20)
                    if not chunk:
                        break
                    output.write(chunk)
                    digest.update(chunk)
                    done += len(chunk)
                    if total and sys.stderr.isatty() and done >= next_report:
                        print(
                            f"\r  {done / 1e6:.0f}/{total / 1e6:.0f} MB "
                            f"({100 * done / total:.0f}%)",
                            end="",
                            file=sys.stderr,
                            flush=True,
                        )
                        next_report += 64 << 20

        actual = digest.hexdigest()
        if actual != model.sha256:
            raise ValueError(
                f"checksum mismatch for {model.url}: expected {model.sha256}, got {actual}"
            )
        os.replace(temporary, target)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)

    if total and sys.stderr.isatty():
        print(file=sys.stderr)
    return target
