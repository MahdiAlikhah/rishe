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
import http.client
import json
import os
import shutil
import sys
import time
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
    "info",
    "is_pack",
    "path",
    "pull",
    "remove",
    "unlock",
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
    """Directory where downloaded models live.

    Resolved exactly like ``cache_dir`` in ``src/hub.rs``: ``RISHE_HOME`` when it
    is set (even to an empty string), then ``HOME``, then ``USERPROFILE``.
    """
    root = os.environ.get("RISHE_HOME")
    if root is not None:
        return Path(root) / "models"
    # Mirrors `cache_dir` in src/hub.rs: an empty `HOME` is still set, so it wins
    # over `USERPROFILE`. Testing truthiness instead put the two binaries in
    # different cache roots whenever `HOME` was empty.
    if "HOME" in os.environ:
        home = os.environ["HOME"]
    elif "USERPROFILE" in os.environ:
        home = os.environ["USERPROFILE"]
    else:
        home = str(Path.home())
    return Path(home) / ".cache" / "rishe" / "models"


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


def _safe_filename(inferred: str) -> str:
    """Reduce a URL or caller-supplied name to one plain cache file name.

    The result is joined onto the cache directory, so it must stay one component:
    a separator, a drive prefix or a ``..`` in it would escape the cache. A query
    string or fragment is dropped rather than becoming part of the name. Mirrors
    ``infer_filename`` in ``src/hub.rs``.
    """
    name = inferred.split("?")[0].split("#")[0]
    # Split on "/" only, like `infer_filename` in src/hub.rs. `Path(name).name`
    # would split on "\\" on Windows but not on POSIX, so the same URL would be
    # accepted on one platform and rejected on the other.
    name = name.rsplit("/", 1)[-1]
    if not name or name in {".", ".."}:
        raise ValueError("cannot infer a filename from the URL; pass filename=")
    if any(character in name for character in ("/", "\\", ":", "\0")):
        raise ValueError(
            "cannot infer a filename from the URL; it must be a plain file name"
        )
    # Windows resolves these names to devices wherever they appear, so the file
    # can be downloaded and verified but never renamed into place.
    stem = name.split(".")[0].upper()
    if stem in _WINDOWS_DEVICE_NAMES:
        raise ValueError(
            "cannot infer a filename from the URL; it is a reserved device name"
        )
    return name


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
    An already cached file is hashed again on every pull; use ``cached`` when
    you only need the path.
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
        # Only a network URL is a download. `file:`/`ftp:`/`data:` would copy or
        # fetch bytes by another protocol and drop them in the model cache.
        scheme = urlparse(source).scheme.lower()
        if scheme not in ("http", "https"):
            raise ValueError(
                f"unsupported URL scheme {scheme or '(none)'!r}; use http or https"
            )
        inferred = filename or Path(urlparse(source).path).name
        safe_name = _safe_filename(inferred)
        # A custom URL must not land on a registered model's cache file: the bytes
        # are only as trustworthy as the digest the caller passed.
        for known_name, known in MODELS.items():
            if known.filename == safe_name:
                raise ValueError(
                    f"{safe_name} belongs to the registered model {known_name!r}; "
                    "pass filename= to pick a different name"
                )
        model = Model(url=source, sha256=sha256.lower(), filename=safe_name)

    digest = model.sha256.lower()
    if len(digest) != 64 or any(
        char not in "0123456789abcdefABCDEF" for char in digest
    ):
        raise ValueError("sha256 must be a 64-character hex digest")

    target = cache_dir() / Path(model.filename).name
    return _download(model.url, target, model.sha256, force=force)


# A lock older than this was left by a process that died mid-download.
_STALE_LOCK_SECONDS = 3600

# Windows device names; mirrored by `infer_filename` in src/hub.rs.
_WINDOWS_DEVICE_NAMES = frozenset(
    ["CON", "PRN", "AUX", "NUL"]
    + [f"COM{index}" for index in range(1, 10)]
    + [f"LPT{index}" for index in range(1, 10)]
)


def _acquire_download_lock(target: Path) -> Path:
    """Claim ``target`` so two processes never share one ``.part`` file.

    A second pull fails with a clear message instead of corrupting the first. A
    lock older than :data:`_STALE_LOCK_SECONDS` is taken over, so a crash does
    not block the download forever. The lock records a pid token so its owner can
    tell its own lock from one another process took over.
    """
    lock = target.with_name(target.name + ".lock")
    token = f"{os.getpid()} {time.time_ns()}".encode()
    for _ in range(2):
        try:
            handle = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            try:
                os.write(handle, token)
            finally:
                os.close(handle)
            return lock
        except FileExistsError:
            try:
                age = time.time() - lock.stat().st_mtime
            except OSError:
                continue
            if age > _STALE_LOCK_SECONDS:
                lock.unlink(missing_ok=True)
                continue
            raise RuntimeError(
                f"another rishe process is downloading {target.name}; "
                "retry when it finishes"
            ) from None
    raise RuntimeError(f"cannot lock {target}")


def unlock(name: str) -> bool:
    """Clear a download lock left behind by a killed process.

    The lock is only taken over automatically after an hour, so an interrupted
    pull had no way to retry: ``force`` re-downloads but keeps the lock, and
    ``remove`` also drops the cached payload. A pack's lock belongs to the shared
    manifest, which blocks every pack.
    """
    if name not in PACKS and name not in MODELS:
        raise ValueError(_unknown(name))
    cleared = False
    if name in MODELS:
        candidates = [cache_dir() / (Path(MODELS[name].filename).name + ".lock")]
    else:
        manifest = cache_dir() / "rishe-models-manifest.json"
        candidates = [cache_dir() / (name + ".lock"), Path(str(manifest) + ".lock")]
    for lock in candidates:
        if lock.is_file():
            lock.unlink()
            cleared = True
    return cleared


def _download(url: str, target: Path, sha256: str, *, force: bool = False) -> Path:
    """Download one file atomically, resuming a partial download."""
    target = Path(target)
    if not force and target.is_file() and _sha256(target) == sha256.lower():
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    lock = _acquire_download_lock(target)
    token = lock.read_text(encoding="utf-8", errors="replace") if lock.is_file() else ""
    try:
        return _download_locked(url, target, sha256, force=force)
    finally:
        # Remove the lock only while it is still ours: a download that ran past the
        # stale window has already had its lock taken over by another process.
        if lock.is_file() and lock.read_text(encoding="utf-8", errors="replace") == token:
            lock.unlink(missing_ok=True)


def _download_locked(url: str, target: Path, sha256: str, *, force: bool = False) -> Path:
    """The download itself; the caller holds the target's lock."""
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
                    # A complete-but-unrenamed part answers a resume with 416;
                    # promote it instead of deleting the transfer.
                    if _sha256(part) == sha256.lower():
                        os.replace(part, target)
                        return target
                    part.unlink()
                    last_error = error
                    continue
                if 400 <= error.code < 500:
                    # `HTTPError` is a `URLError`, so without this a 404 below is
                    # retried twice more; the server will answer the same way.
                    raise RuntimeError(
                        f"download failed for {url}: {error.code} {error.reason}"
                    ) from error
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
                    # Flush to disk before os.replace promotes the part file: a
                    # crash between the replace and the page-cache writeback would
                    # otherwise leave the model short or empty.
                    output.flush()
                    os.fsync(output.fileno())

            if total and done < total:
                # A truncated body is a network failure, not a corrupted remote:
                # the prefix is still good, so the next attempt resumes it instead
                # of deleting the transfer and starting over.
                last_error = ValueError(
                    f"download of {url} stopped after {done} of {total} bytes"
                )
                continue

            actual = _sha256(part)
            if actual != sha256.lower():
                part.unlink()
                # A mismatch on a fresh download means the remote bytes are wrong,
                # so fetching them again gives the same mismatch. A mismatch on a
                # resume may be entirely in the stale prefix just deleted, so the
                # next attempt starts from zero and can succeed.
                if offset == 0:
                    raise ValueError(
                        f"checksum mismatch for {url}: expected {sha256}, got {actual}"
                    )
                last_error = ValueError(
                    f"checksum mismatch for {url}: the partial file did not match; "
                    "restarting the download"
                )
                continue
            os.replace(part, target)
            if total and sys.stderr.isatty():
                print(file=sys.stderr)
            return target
        except ValueError as error:
            if str(error).startswith("checksum mismatch"):
                raise
            last_error = error
        except (urllib.error.URLError, http.client.HTTPException, TimeoutError, OSError) as error:
            # `http.client` raises its own hierarchy for a malformed response
            # (IncompleteRead, BadStatusLine); it is not an OSError, so it used to
            # escape as a traceback instead of a retry.
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


def info(name: str) -> Optional[dict]:
    """Return cache information for a model or pack, or ``None`` when missing.

    The returned dict always carries the same five keys, including ``partial``,
    so a caller can test it without ``.get`` or a ``KeyError``.
    """
    if name not in PACKS and name not in MODELS:
        raise ValueError(_unknown(name))
    target = cached(name)
    if target is None:
        # An interrupted pull leaves bytes on disk; report them instead of None.
        if name in MODELS:
            part = cache_dir() / (Path(MODELS[name].filename).name + ".part")
            if part.is_file():
                return {
                    "name": name,
                    "kind": "model",
                    "path": str(part),
                    "bytes": part.stat().st_size,
                    "partial": True,
                }
        else:
            partial = cache_dir() / name
            if partial.is_dir():
                return {
                    "name": name,
                    "kind": "pack",
                    "path": str(partial),
                    "bytes": sum(
                        file.stat().st_size for file in partial.rglob("*") if file.is_file()
                    ),
                    "partial": True,
                }
        return None
    if target.is_dir():
        size = sum(file.stat().st_size for file in target.rglob("*") if file.is_file())
    else:
        size = target.stat().st_size
    return {
        "name": name,
        "kind": "pack" if name in PACKS else "model",
        "path": str(target),
        "bytes": size,
        "partial": False,
    }


def remove(name: str) -> bool:
    """Remove a cached model or pack. Returns whether anything was removed."""
    if name not in PACKS and name not in MODELS:
        raise ValueError(_unknown(name))
    removed = False
    target = cached(name)
    if target is None:
        # Clean up a partial pack directory as well.
        partial = cache_dir() / name
        if name in PACKS and partial.is_dir():
            shutil.rmtree(partial)
            removed = True
    elif target.is_dir():
        shutil.rmtree(target)
        removed = True
    else:
        target.unlink()
        removed = True
    if name in MODELS:
        # A model may be only partly downloaded; the part file holds that disk space.
        part = cache_dir() / (Path(MODELS[name].filename).name + ".part")
        if part.is_file():
            part.unlink()
            removed = True
        lock = cache_dir() / (Path(MODELS[name].filename).name + ".lock")
        if lock.is_file():
            lock.unlink()
            removed = True
    if name in PACKS:
        # A killed pack pull also leaves the shared manifest's part and lock in
        # the cache root; `remove` is the only command that can clear them.
        pack_lock = cache_dir() / (name + ".lock")
        if pack_lock.is_file():
            pack_lock.unlink()
            removed = True
        manifest = cache_dir() / "rishe-models-manifest.json"
        for stale in (
            Path(str(manifest) + ".part"),
            Path(str(manifest) + ".lock"),
        ):
            if stale.is_file():
                stale.unlink()
                removed = True
    return removed


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
        # An empty `files` object used to make the loop below run zero times and
        # bless an empty directory as a verified pack.
        if not files:
            return False
        for filename, metadata in files.items():
            path = target / Path(filename).name
            if not path.is_file() or _sha256(path) != metadata["sha256"].lower():
                return False
        return all((target / file).is_file() for file in PACK_REQUIRED)
    raise ValueError(f"unknown model or pack {name!r}")


def _pull_pack(name: str, *, force: bool = False) -> Path:
    """Download a task pack (model, tokenizer and labels) and return its directory."""
    task = PACKS.get(name)
    if task is None:
        raise ValueError(f"unknown pack {name!r}; known packs: {', '.join(PACKS)}")
    target = cache_dir() / name
    # A full-looking directory is not a valid pack: an older manifest or an
    # interrupted download leaves the same three files behind. Verify the hashes
    # before returning, as the single-file pull already does.
    if not force and verify(name):
        return target
    target.mkdir(parents=True, exist_ok=True)

    manifest_path = cache_dir() / "rishe-models-manifest.json"
    # The manifest is sha-pinned, so a cached copy with the right digest is the
    # same file; re-fetching it costs a round trip and fails offline for nothing.
    _download(
        f"{_pack_base()}/manifest.json",
        manifest_path,
        PACK_MANIFEST_SHA256,
        force=False,
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

