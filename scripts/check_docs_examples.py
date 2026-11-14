#!/usr/bin/env python3
"""Run the ```python blocks from each README's quick-start section.

Covers only the quick-start sections of README.md, README.pypi.md and
README.fa.md; the deeper tours carry their own examples (model packs, the hub,
embedding files), so they are not executed here. None of these blocks needs a
model pack or the network, but they do need the compiled extension: run this
after `pip install` (CI: the `examples` job) or with a built `_core`.

Blocks inside one section share a namespace: the later quick-start blocks reuse
`rishe` and `text` from the earlier ones.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SECTIONS = {
    "README.md": "Quick start",
    "README.pypi.md": "Quickstart",
    "README.fa.md": "شروع سریع",
}
HEADING = re.compile(r"^(#+)\s+(.*)$")
FENCE = re.compile(r"^```(\w*)\s*$")

# Appended, not prepended: an installed wheel wins, and a checkout with the
# extension already built still runs.
sys.path.append(str(ROOT / "python"))


def python_blocks(path, title):
    """[(line of the first code line, source)] for the ```python blocks of one section."""
    lines = path.read_text(encoding="utf-8").splitlines()
    start = next(
        (
            i
            for i, line in enumerate(lines)
            if (heading := HEADING.match(line)) and heading.group(2).strip() == title
        ),
        None,
    )
    if start is None:
        raise SystemExit(f"{path.name}: no {title!r} heading")
    level = len(HEADING.match(lines[start]).group(1))
    blocks, body, language = [], None, None
    for number, line in enumerate(lines[start + 1 :], start + 2):
        fence = FENCE.match(line)
        if body is not None:
            if fence:
                if language == "python":
                    blocks.append((number - len(body), "\n".join(body) + "\n"))
                body = None
            else:
                body.append(line)
        elif fence:
            body, language = [], fence.group(1)
        elif (heading := HEADING.match(line)) and len(heading.group(1)) <= level:
            break
    if body is not None:
        raise SystemExit(f"{path.name}: unterminated ```{language} fence under {title!r}")
    return blocks


def main():
    total = 0
    for name, title in SECTIONS.items():
        blocks = python_blocks(ROOT / name, title)
        if not blocks:
            raise SystemExit(f"{name}: the {title!r} section has no ```python block")
        namespace = {}
        for line, source in blocks:
            exec(compile(source, f"{name}:{line}", "exec"), namespace)
        total += len(blocks)
        print(f"{name}: {len(blocks)} block(s)")
    print(f"{total} block(s) ran")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
