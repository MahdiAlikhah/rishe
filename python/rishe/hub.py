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
import shutil
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Dict, NamedTuple, Optional
from urllib.parse import urlparse

from ._deprecation import deprecated

__all__ = [
    "MODELS",
    "PACKS",
    "cache_dir",
    "cached",
    "cached_pack",
    "info",
    "is_pack",
    "path",
    "path_pack",
    "pull",
    "pull_pack",
    "remove",
    "verify",
]


class Model(NamedTuple):
    url: str
    sha256: str
    filename: str


MODELS: Dict[str, Model] = {
    "rishe-embed-fa-300-v2": Model(
        url="https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/embeddings/rishe-embed-fa-300-v2/rishe-embed-fa-300-v2.rvec",
        sha256="b870a2d88dc04cadaf117d7fb8aad07264c640293adb8689e69cac3d123a37f1",
        filename="rishe-embed-fa-300-v2.rvec",
    ),
    "rishe-embed-fa-300": Model(
        url="https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/embeddings/rishe-embed-fa-300.vec",
        sha256="afed6aa266e07745ce7b6b1e2f6600f6f0373dc91e197902d7305e19d2d35bf5",
        filename="rishe-embed-fa-300.vec",
    ),
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
    "depparse-compact": "depparse",
}
PACK_BASE = "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main"
PACK_MANIFEST_SHA256 = "60885efc0680a47a3f7efbd3bc699c19766dc33c6b072b44c31086811d43f12e"
PACK_REQUIRED = ("model.onnx", "tokenizer.json", "labels.json")

# The registry manifest (`manifest.json`) is versioned independently of the pack
# files. A missing "schema" field means MANIFEST_SCHEMA (the only version
# published so far); `_manifest_schema` refuses any other major version so an
# older client never misreads a newer index. Adding optional fields does not
# bump the version; a breaking layout change does, and both readers are updated
# together.
MANIFEST_SCHEMA = 1


def _manifest_schema(manifest: dict) -> int:
    """Validate the manifest ``schema`` field and return the version.

    A missing field means :data:`MANIFEST_SCHEMA`. Only that version is
    accepted; any other integer or a non-integer value is refused with a
    message naming the found value and the supported version.
    """
    if "schema" not in manifest:
        return MANIFEST_SCHEMA
    value = manifest["schema"]
    if isinstance(value, bool) or not isinstance(value, int) or value != MANIFEST_SCHEMA:
        raise ValueError(
            f"unsupported manifest schema {value!r}; "
            f"this build reads schema {MANIFEST_SCHEMA}"
        )
    return MANIFEST_SCHEMA


def cache_dir() -> Path:
    """Directory where downloaded models live."""
    root = os.environ.get("RISHE_HOME")
    base = Path(root) if root else Path.home() / ".cache" / "rishe"
    return base / "models"


def _unknown(name: str) -> str:
    return (
        f"unknown model or pack {name!r}; "
        f"models: {', '.join(MODELS)}; packs: {', '.join(PACKS)}"
    )


def is_pack(name: str) -> bool:
    """Whether ``name`` refers to a task pack rather than a single model file."""
    return name in PACKS


def cached(name: str) -> Optional[Path]:
    """Return the cached path of a model or pack, or ``None`` when missing.

    Models resolve to a file, task packs to a directory.
    """
    if name in PACKS:
        target = cache_dir() / name
        return target if all((target / file).is_file() for file in PACK_REQUIRED) else None
    model = MODELS.get(name)
    if model is None:
        raise ValueError(_unknown(name))
    target = cache_dir() / Path(model.filename).name
    return target if target.exists() else None


def path(name: str) -> Path:
    """Return the cached path of a model or pack, raising when it was not pulled."""
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
    """Download a known model or task pack (or a custom URL) and return its path.

    Custom URLs must pass ``sha256``; the checksum is verified before the file
    is moved into place, so an interrupted or tampered download is never used.
    """
    if name_or_url in PACKS:
        if sha256 or filename:
            raise ValueError("task packs do not accept sha256= or filename=")
        return _pull_pack(name_or_url, force=force)

    model = MODELS.get(name_or_url)
    if model is None:
        if not sha256:
            raise ValueError(
                f"{_unknown(name_or_url)}. Pass sha256= to pull a custom URL."
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
    """Download one file atomically, resuming a partial download."""
    target = Path(target)
    if target.exists() and not force:
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    part = target.with_name(target.name + ".part")
    last_error: Optional[Exception] = None

    for _ in range(3):
        offset = part.stat().st_size if part.exists() else 0
        headers = {"User-Agent": "rishe"}
        if offset:
            headers["Range"] = f"bytes={offset}-"
        total = 0
        try:
            request = urllib.request.Request(url, headers=headers)
            try:
                response = urllib.request.urlopen(request, timeout=60)
            except urllib.error.HTTPError as error:
                if error.code == 416 and part.exists():
                    part.unlink()
                    last_error = error
                    continue
                raise
            with response:
                status = getattr(response, "status", 200)
                if offset and status == 206:
                    mode = "ab"
                else:
                    mode = "wb"
                    offset = 0
                length = int(response.headers.get("Content-Length") or 0)
                total = offset + length if length else 0
                done = offset
                next_report = 64 << 20
                with open(part, mode) as output:
                    for chunk in iter(lambda: response.read(1 << 20), b""):
                        output.write(chunk)
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

            actual = _sha256(part)
            if actual != sha256.lower():
                part.unlink()
                raise ValueError(
                    f"checksum mismatch for {url}: expected {sha256}, got {actual}"
                )
            os.replace(part, target)
            if total and sys.stderr.isatty():
                print(file=sys.stderr)
            return target
        except (urllib.error.URLError, TimeoutError, OSError, ValueError) as error:
            last_error = error

    raise RuntimeError(f"download failed after 3 attempts: {last_error}") from last_error


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _pack_base() -> str:
    return os.environ.get("RISHE_MODEL_BASE", PACK_BASE).rstrip("/")


@deprecated("rishe.hub.cached_pack", "rishe.hub.cached")
def cached_pack(name: str) -> Optional[Path]:
    """Deprecated alias of :func:`cached` for task packs."""
    return cached(name)


@deprecated("rishe.hub.path_pack", "rishe.hub.path")
def path_pack(name: str) -> Path:
    """Deprecated alias of :func:`path` for task packs."""
    return path(name)


def info(name: str) -> Optional[dict]:
    """Return cache information for a model or pack, or ``None`` when missing."""
    if name not in PACKS and name not in MODELS:
        raise ValueError(_unknown(name))
    target = cached(name)
    if target is None:
        return None
    if target.is_dir():
        size = sum(file.stat().st_size for file in target.rglob("*") if file.is_file())
    else:
        size = target.stat().st_size
    return {"name": name, "kind": "pack" if name in PACKS else "model", "path": str(target), "bytes": size}


def remove(name: str) -> bool:
    """Remove a cached model or pack. Returns whether anything was removed."""
    if name not in PACKS and name not in MODELS:
        raise ValueError(_unknown(name))
    target = cached(name)
    if target is None:
        # Clean up a partial pack directory as well.
        partial = cache_dir() / name
        if name in PACKS and partial.is_dir():
            shutil.rmtree(partial)
            return True
        return False
    if target.is_dir():
        shutil.rmtree(target)
    else:
        target.unlink()
    return True


def verify(name: str) -> bool:
    """Verify cached files against their known checksums."""
    if name in MODELS:
        target = cached(name)
        return target is not None and _sha256(target) == MODELS[name].sha256
    if name in PACKS:
        target = cache_dir() / name
        manifest_path = cache_dir() / "rishe-models-manifest.json"
        if not target.is_dir() or not manifest_path.is_file():
            return False
        if _sha256(manifest_path) != PACK_MANIFEST_SHA256:
            return False
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        _manifest_schema(manifest)
        files = manifest.get("tasks", {}).get(PACKS[name], {}).get("files", {})
        for filename, metadata in files.items():
            path = target / Path(filename).name
            if not path.is_file() or _sha256(path) != metadata["sha256"].lower():
                return False
        return True
    raise ValueError(f"unknown model or pack {name!r}")


def _pull_pack(name: str, *, force: bool = False) -> Path:
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
    _manifest_schema(manifest)
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


@deprecated("rishe.hub.pull_pack", "rishe.hub.pull")
def pull_pack(name: str, *, force: bool = False) -> Path:
    """Deprecated alias of :func:`pull` for task packs."""
    return pull(name, force=force)
