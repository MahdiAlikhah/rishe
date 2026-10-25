import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from rishe import ModelPack, hub

PACK_ROOT = Path(os.environ["RISHE_TEST_PACK"]) if os.environ.get("RISHE_TEST_PACK") else None


class PackCacheTest(unittest.TestCase):
    def test_pack_cache(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                self.assertIsNone(hub.cached_pack("pos-compact"))
                target = Path(root) / "models" / "pos-compact"
                target.mkdir(parents=True)
                for name in hub.PACK_REQUIRED:
                    (target / name).write_bytes(b"x")
                self.assertEqual(hub.cached_pack("pos-compact"), target)
                self.assertEqual(hub.path_pack("pos-compact"), target)
                with self.assertRaises(ValueError):
                    hub.cached_pack("unknown")
                with self.assertRaises(FileNotFoundError):
                    hub.path_pack("ner-compact")


@unittest.skipUnless(PACK_ROOT is not None and PACK_ROOT.is_dir(), "RISHE_TEST_PACK is not set")
class ModelPackTest(unittest.TestCase):
    def test_pos(self):
        pack = ModelPack(str(PACK_ROOT / "pos"))
        self.assertEqual(pack.task, "pos")
        self.assertEqual(pack.max_len, 192)
        self.assertEqual(len(pack.labels()), 16)
        words = ["کتاب‌های", "خوب", "را", "به", "دوستانم", "می‌دهم"]
        result = pack.tag_words(words)
        self.assertEqual([word for word, _ in result], words)
        self.assertEqual(result[0][1], "NOUN")
        self.assertEqual(result[1][1], "ADJ")
        self.assertEqual(result[-1][1], "VERB")
        self.assertTrue(pack.tag("کتاب‌های خوب"))

    def test_ner(self):
        pack = ModelPack(str(PACK_ROOT / "ner"))
        self.assertEqual(pack.task, "ner")
        self.assertEqual(len(pack.labels()), 21)
        result = pack.tag_words(["سرلشگر", "فیروزآبادی", "به", "پادشاه", "عربستان", "گفت"])
        self.assertEqual(len(result), 6)
        self.assertTrue(all(label in pack.labels() for _, label in result))
        self.assertTrue(any(label != "O" for _, label in result))

    def test_ezafe(self):
        pack = ModelPack(str(PACK_ROOT / "ezafe"))
        self.assertEqual(pack.task, "ezafe")
        self.assertEqual(pack.max_len, 128)
        result = pack.tag_words(["کتابخانه", "مرکزی", "دانشگاه", "شریف"])
        self.assertEqual(len(result), 4)
        self.assertTrue(all(label in {"NO_EZAFE", "EZAFE"} for _, label in result))


if __name__ == "__main__":
    unittest.main()
