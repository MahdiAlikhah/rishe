"""Execute the Quick start blocks of the three READMEs.

The quick start is the code a new user copies first, so it is the part most
likely to rot. Later README sections are not run: they need a downloaded model
pack or embedding file. The compiled extension must be importable.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (README, heading text, lowercased) — one section per README.
SECTIONS = [
    ("README.md", "quick start"),
    ("README.pypi.md", "quickstart"),
    ("README.fa.md", "شروع سریع"),
]

HEADING = re.compile(r"^(#{1,6})\s+(.*)$")
PYTHON_FENCE = re.compile(r"^```python\n(.*?)^```", re.S | re.M)


def quick_start_blocks(text: str, marker: str, source: str) -> list[str]:
    lines = text.splitlines()
    for start, line in enumerate(lines):
        heading = HEADING.match(line)
        if heading and marker in heading.group(2).lower():
            level = len(heading.group(1))
            for end in range(start + 1, len(lines)):
                next_heading = HEADING.match(lines[end])
                if next_heading and len(next_heading.group(1)) <= level:
                    return PYTHON_FENCE.findall("\n".join(lines[start:end]))
            return PYTHON_FENCE.findall("\n".join(lines[start:]))
    raise SystemExit(f"{source}: no {marker!r} heading found")


def main() -> int:
    ran = 0
    for name, marker in SECTIONS:
        text = (ROOT / name).read_text(encoding="utf-8")
        blocks = quick_start_blocks(text, marker, name)
        if not blocks:
            raise SystemExit(f"{name}: the quick start section has no ```python block")
        namespace = {"__name__": "readme_quick_start"}
        # One namespace per file: later blocks reuse names from earlier ones,
        # exactly as a reader copies them in order.
        for number, block in enumerate(blocks, start=1):
            where = f"{name}#quick-start-block-{number}"
            exec(compile(block, where, "exec"), namespace)  # noqa: S102 - our own READMEs
            ran += 1
    print(f"ran {ran} quick-start blocks from {len(SECTIONS)} READMEs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
