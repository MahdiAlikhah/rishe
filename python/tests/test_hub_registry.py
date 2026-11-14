"""The hub registry is written twice, in Rust and in Python; fail if they differ."""

import re
import unittest
from pathlib import Path

from rishe import hub

RUST = (Path(__file__).resolve().parents[2] / "src" / "hub.rs").read_text(encoding="utf-8")


def _block(name):
    return re.search(rf"const {name}: [^=]+= &\[(.*?)\];", RUST, re.S).group(1)


def _strings(text):
    return re.findall(r'"([^"]*)"', text)


class RegistryParityTest(unittest.TestCase):
    def test_models_match_the_rust_table(self):
        values = _strings(_block("MODELS"))
        parsed = {
            values[i]: hub.Model(values[i + 1], values[i + 2], values[i + 3])
            for i in range(0, len(values), 4)
        }
        self.assertEqual(parsed, hub.MODELS)

    def test_packs_match_the_rust_table(self):
        values = _strings(_block("PACKS"))
        self.assertEqual(dict(zip(values[::2], values[1::2])), hub.PACKS)

    def test_shared_constants_match_the_rust_table(self):
        base = re.search(r'const PACK_BASE: &str = "([^"]*)"', RUST).group(1)
        manifest = re.search(r'const PACK_MANIFEST_SHA256: &str =\s*"([^"]*)"', RUST)
        required = _strings(
            re.search(r"const PACK_REQUIRED: \[&str; \d+\] = \[(.*?)\];", RUST, re.S).group(1)
        )
        self.assertEqual(base, hub.PACK_BASE)
        self.assertEqual(manifest.group(1), hub.PACK_MANIFEST_SHA256)
        self.assertEqual(tuple(required), hub.PACK_REQUIRED)


if __name__ == "__main__":
    unittest.main()
