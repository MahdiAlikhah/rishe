import hashlib
import json
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
TINY_PACK = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "tiny-pack"


class TinyPackTest(unittest.TestCase):
    def test_tiny_pack(self):
        pack = ModelPack(TINY_PACK)
        self.assertEqual(pack.task, "pos")
        self.assertEqual(pack.tag("good bad"), [("good", "A"), ("bad", "B")])
        self.assertEqual(pack.tag_tokens("good bad")[0].start, 0)

        pipe = Pipeline(pos_pack=TINY_PACK)
        self.assertEqual(pipe.pos("good bad")[1].tag, "B")


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
                self.assertEqual(hub.info("pos-compact")["path"], str(target))
                self.assertTrue(hub.remove("pos-compact"))
                self.assertFalse(hub.remove("pos-compact"))
                self.assertIsNone(hub.cached_pack("pos-compact"))
                with self.assertRaises(ValueError):
                    hub.cached_pack("unknown")
                with self.assertRaises(FileNotFoundError):
                    hub.path_pack("ner-compact")

    def test_verify_pack(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                target = Path(root) / "models" / "pos-compact"
                target.mkdir(parents=True)
                files = {
                    name: (b"data-" + name.encode())
                    for name in hub.PACK_REQUIRED
                }
                for name, data in files.items():
                    (target / name).write_bytes(data)
                manifest = {
                    "tasks": {
                        "pos": {
                            "files": {
                                name: {"sha256": hashlib.sha256(data).hexdigest()}
                                for name, data in files.items()
                            }
                        }
                    }
                }
                manifest_path = Path(root) / "models" / "rishe-models-manifest.json"
                manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
                digest = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
                with patch.object(hub, "PACK_MANIFEST_SHA256", digest):
                    self.assertTrue(hub.verify("pos-compact"))
                    (target / "model.onnx").write_bytes(b"corrupt")
                    self.assertFalse(hub.verify("pos-compact"))


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

    def test_batch_and_task_validation(self):
        pipe = Pipeline(
            pos_pack=str(PACK_ROOT / "pos"),
            ner_pack=str(PACK_ROOT / "ner"),
            ezafe_pack=str(PACK_ROOT / "ezafe"),
        )
        pos = pipe.pos_batch(["کتاب‌های خوب", "خوب است"])
        self.assertEqual(len(pos), 2)
        self.assertEqual(pos[0][0].tag, "NOUN")

        ner = pipe.ner_batch(["فیروزآبادی گفت.", "عربستان جای خوبی است."])
        self.assertEqual(len(ner), 2)
        self.assertTrue(any(entity.label == "PER" for entity in ner[0]))

        ezafe = pipe.ezafe_batch(["کتابخانه مرکزی", "دانشگاه شریف"])
        self.assertEqual(len(ezafe), 2)

        pack = ModelPack(str(PACK_ROOT / "pos"))
        self.assertEqual(len(pack.tag_batch(["کتاب‌های خوب", "خوب است"])), 2)

        with self.assertRaises(ValueError):
            Pipeline(pos_pack=str(PACK_ROOT / "ner"))

    def test_path_inputs(self):
        pack = ModelPack(PACK_ROOT / "pos")
        self.assertEqual(pack.task, "pos")

        pipe = Pipeline(pos_pack=PACK_ROOT / "pos")
        self.assertEqual(pipe.pos("کتاب‌های خوب")[0].tag, "NOUN")

    def test_cli_serve_pack(self):
        output = StringIO()
        request = json.dumps({"command": "pos", "text": "کتاب‌های خوب"}) + "\n"
        with patch("sys.stdin", StringIO(request)), redirect_stdout(output):
            code = main(["serve", "--pos-pack", str(PACK_ROOT / "pos")])
        self.assertEqual(code, 0)
        response = json.loads(output.getvalue().strip())
        self.assertTrue(response["ok"])
        self.assertEqual(response["result"][0]["tag"], "NOUN")

    def test_cli_pack(self):
        output = StringIO()
        with redirect_stdout(output):
            code = main(["pos", "--pack", str(PACK_ROOT / "pos"), "کتاب‌های خوب"])
        self.assertEqual(code, 0)
        self.assertIn("NOUN", output.getvalue())


if __name__ == "__main__":
    unittest.main()
