#!/usr/bin/env python3
"""Pull the packs the env-gated tests need, then print the variables to set.

`python/tests/test_models.py` and `test_embeddings.py` skip 12 tests unless
`RISHE_TEST_PACK`, `RISHE_TEST_DEPPARSE_PACK` and `RISHE_TEST_EMBEDDINGS_PACK`
are set, so without this nothing in CI ever runs ONNX inference. This downloads
the four task packs and the first-party embedding into the Hub cache, then builds
`<root>/{pos,ner,ezafe,depparse}` as links to that cache — the tests expect that
layout, while the cache names packs `pos-compact` and so on.

    eval "$(python scripts/pull_test_packs.py)"
    python -m unittest discover -s python/tests -v

`--selftest` checks the linking without downloading ~225 MB.
"""
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.append(str(ROOT / "python"))

from rishe import hub  # noqa: E402

EMBEDDING = "rishe-embed-fa-300-v2"


def link(target, path):
    """Point `path` at an existing directory, replacing whatever is there."""
    # A Windows junction is not is_symlink() but rmtree still refuses it, so
    # remove it as a directory instead of recursing into it.
    if os.name == "nt" and os.path.isjunction(path):
        path.rmdir()
    elif path.is_symlink() or path.is_file():
        path.unlink()
    elif path.exists():
        shutil.rmtree(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    if os.name == "nt":
        # A junction needs no admin rights where a symlink does.
        subprocess.run(
            ["cmd", "/c", "mklink", "/J", str(path), str(target)],
            check=True, capture_output=True,
        )
    else:
        path.symlink_to(target, target_is_directory=True)


def selftest():
    with tempfile.TemporaryDirectory() as tmp:
        source, root = Path(tmp, "src"), Path(tmp, "root")
        source.mkdir()
        link(source, root / "pos")
        assert (root / "pos").is_dir(), "link is not a directory"
        link(source, root / "pos")  # a second run replaces it, not nests it
        assert (root / "pos").is_dir()
    print("pull_test_packs: selftest ok")


def main(argv):
    if argv[:1] == ["--selftest"]:
        selftest()
        return 0

    root = ROOT / ".test-packs"
    for name, task in hub.PACKS.items():
        link(hub.pull(name), root / task)
    print(f'export RISHE_TEST_PACK="{root}"')
    print(f'export RISHE_TEST_DEPPARSE_PACK="{root / "depparse"}"')
    print(f'export RISHE_TEST_EMBEDDINGS_PACK="{hub.pull(EMBEDDING)}"')
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
