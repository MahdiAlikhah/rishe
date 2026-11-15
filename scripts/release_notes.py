#!/usr/bin/env python3
"""Print the CHANGELOG section for a version, for use as GitHub release notes.

Usage: `release_notes.py <version>`. Exits 1 when the CHANGELOG has no
`## [<version>]` section, so a tag without an entry fails instead of publishing
an empty or wrong page.

`--selftest` checks the heading match: `## [1.0.1]` must not pick up the
`## [1.0.10]` section, which a prefix test would.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADING = "## ["


def section(text, version):
    body, found = [], False
    for line in text.splitlines():
        if line.startswith(f"{HEADING}{version}]"):
            found = True
            continue
        if found and line.startswith(HEADING):
            break
        if found:
            body.append(line)
    return "\n".join(body).strip() if found else None


def selftest():
    log = (
        "# Changelog\n\n## [Unreleased]\n\n"
        "## [1.0.10] - 2026-12-01\n\nTEN.\n\n"
        "## [1.0.1] - 2026-11-14\n\nONE.\n\n"
        "## [1.0.0] - 2026-11-01\n\nZERO.\n"
    )
    assert section(log, "1.0.1") == "ONE."
    assert section(log, "1.0.10") == "TEN.", "prefix collision"
    assert section(log, "1.0.0") == "ZERO."
    assert section(log, "9.9.9") is None
    print("release_notes: selftest ok")
    return 0


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    if len(argv) != 1:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    body = section((ROOT / "CHANGELOG.md").read_text(encoding="utf-8"), argv[0])
    if body is None:
        print(f"no CHANGELOG section for {argv[0]}", file=sys.stderr)
        return 1
    print(body)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
