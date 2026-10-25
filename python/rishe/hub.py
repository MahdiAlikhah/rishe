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
import json
import os
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path
from typing import Dict, NamedTuple, Optional
from urllib.parse import urlparse

__all__ = [
    "MODELS",
    "PACKS",
    "cache_dir",
    "cached",
    "cached_pack",
    "path",
    "path_pack",
    "pull",
    "pull_pack",
]


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

PACKS: Dict[str, str] = {
    "pos-compact": "pos",
    "ner-compact": "ner",
    "ezafe-compact": "ezafe",
}
PACK_BASE = "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main"
PACK_MANIFEST_SHA256 = "a8ad4ae2c354c2d278344e5433fce0dd3fb5116415d4e5d2c788b0b407010661"
PACK_REQUIRED = ("model.onnx", "tokenizer.json", "labels.json")


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
    return _download(model.url, target, model.sha256, force=force)


def _download(url: str, target: Path, sha256: str, *, force: bool = False) -> Path:
    """Download one file atomically and verify its checksum."""
    target = Path(target)
    if target.exists() and not force:
        return target
    target.parent.mkdir(parents=True, exist_ok=True)

    handle, temporary = tempfile.mkstemp(
        dir=target.parent, prefix=f".{target.name}.", suffix=".part"
    )
    # ponytail: an interrupted download restarts from zero; add Range resume
    # when model sizes keep growing or the CDN proves unreliable.
    digest = hashlib.sha256()
    total = 0
    try:
        with os.fdopen(handle, "wb") as output:
            request = urllib.request.Request(url, headers={"User-Agent": "rishe"})
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
        if actual != sha256.lower():
            raise ValueError(
                f"checksum mismatch for {url}: expected {sha256}, got {actual}"
            )
        os.replace(temporary, target)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)

    if total and sys.stderr.isatty():
        print(file=sys.stderr)
    return target


def _pack_base() -> str:
    return os.environ.get("RISHE_MODEL_BASE", PACK_BASE).rstrip("/")


def cached_pack(name: str) -> Optional[Path]:
    """Return the local directory of a cached task pack, or ``None``."""
    if name not in PACKS:
        raise ValueError(f"unknown pack {name!r}; known packs: {', '.join(PACKS)}")
    target = cache_dir() / name
    return target if all((target / file).is_file() for file in PACK_REQUIRED) else None


def path_pack(name: str) -> Path:
    """Return the local directory of a cached task pack, raising when it was not pulled."""
    target = cached_pack(name)
    if target is None:
        raise FileNotFoundError(
            f"{name!r} is not cached; run: rishe hub pull {name} "
            f"(or rishe.hub.pull_pack({name!r}))"
        )
    return target


def pull_pack(name: str, *, force: bool = False) -> Path:
    """Download a task pack (model, tokenizer and labels) and return its directory."""
    task = PACKS.get(name)
    if task is None:
        raise ValueError(f"unknown pack {name!r}; known packs: {', '.join(PACKS)}")
    target = cache_dir() / name
    if not force and all((target / file).is_file() for file in PACK_REQUIRED):
        return target
    target.mkdir(parents=True, exist_ok=True)

    manifest_path = cache_dir() / "rishe-models-manifest.json"
    _download(
        f"{_pack_base()}/manifest.json",
        manifest_path,
        PACK_MANIFEST_SHA256,
        force=True,
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    files = manifest.get("tasks", {}).get(task, {}).get("files", {})
    if not files:
        raise ValueError(f"manifest has no files for task {task!r}")

    for filename, metadata in files.items():
        _download(
            f"{_pack_base()}/{task}/{Path(filename).name}",
            target / Path(filename).name,
            metadata["sha256"],
            force=force,
        )

    missing = [file for file in PACK_REQUIRED if not (target / file).is_file()]
    if missing:
        raise RuntimeError(f"pack {name!r} is incomplete; missing: {', '.join(missing)}")
    return target
