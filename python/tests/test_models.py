import os
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from rishe import Entity, ModelPack, Pipeline, TaggedToken, hub
from rishe.cli import main

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

    def test_typed_tokens_and_entities(self):
        pos = ModelPack(str(PACK_ROOT / "pos"))
        tokens = pos.tag_tokens("کتاب‌های خوب")
        self.assertIsInstance(tokens[0], TaggedToken)
        self.assertEqual(tokens[0].text, "کتاب‌های")
        self.assertEqual(tokens[0].tag, "NOUN")
        self.assertEqual((tokens[0].start, tokens[0].end), (0, 8))

        ner = ModelPack(str(PACK_ROOT / "ner"))
        entities = ner.entities("سرلشگر فیروزآبادی به پادشاه عربستان گفت.")
        self.assertTrue(any(entity.label == "PER" for entity in entities))
        self.assertTrue(any(entity.label == "LOC" for entity in entities))
        self.assertTrue(all(isinstance(entity, Entity) for entity in entities))

    def test_pipeline_packs(self):
        all_packs = Pipeline(
            pos_pack=str(PACK_ROOT / "pos"),
            ner_pack=str(PACK_ROOT / "ner"),
            ezafe_pack=str(PACK_ROOT / "ezafe"),
        )
        self.assertEqual(all_packs.pos("کتاب‌های خوب")[0].tag, "NOUN")
        self.assertTrue(all_packs.ner("فیروزآبادی گفت."))
        self.assertTrue(all_packs.ezafe("کتابخانه مرکزی"))

        pos = Pipeline(pos_pack=str(PACK_ROOT / "pos"))
        self.assertEqual(pos.pos("کتاب‌های خوب")[0].tag, "NOUN")

        ner = Pipeline(ner_pack=str(PACK_ROOT / "ner"))
        self.assertTrue(any(entity.label == "PER" for entity in ner.ner("فیروزآبادی گفت.")))

        ezafe = Pipeline(ezafe_pack=str(PACK_ROOT / "ezafe"))
        self.assertTrue(ezafe.ezafe("کتابخانه مرکزی"))

        with self.assertRaises(ValueError):
            Pipeline().pos("متن")

    def test_cli_pack(self):
        output = StringIO()
        with redirect_stdout(output):
            code = main(["pos", "--pack", str(PACK_ROOT / "pos"), "کتاب‌های خوب"])
        self.assertEqual(code, 0)
        self.assertIn("NOUN", output.getvalue())


if __name__ == "__main__":
    unittest.main()
