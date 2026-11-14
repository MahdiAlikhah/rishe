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
DEPPARSE_PACK = (
    Path(os.environ["RISHE_TEST_DEPPARSE_PACK"])
    if os.environ.get("RISHE_TEST_DEPPARSE_PACK")
    else None
)
TINY_PACK = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "tiny-pack"


class TinyPackTest(unittest.TestCase):
    def test_tiny_pack(self):
        pack = ModelPack(TINY_PACK)
        self.assertEqual(pack.task, "pos")
        labels = [(item.text, item.label) for item in pack.tags("good bad")]
        self.assertEqual(labels, [("good", "A"), ("bad", "B")])
        self.assertEqual(pack.tags("good bad")[0].start, 0)

        pipe = Pipeline(pos_pack=TINY_PACK)
        self.assertEqual(pipe.pos("good bad")[1].tag, "B")

    def test_cli_serve_tiny_pack(self):
        output = StringIO()
        request = json.dumps({"command": "pos", "text": "good bad"}) + "\n"
        with patch("sys.stdin", StringIO(request)), redirect_stdout(output):
            code = main(["serve", "--pos-pack", str(TINY_PACK)])
        self.assertEqual(code, 0)
        response = json.loads(output.getvalue().strip())
        self.assertTrue(response["ok"])
        self.assertEqual(response["result"][1]["tag"], "B")


class PackCacheTest(unittest.TestCase):
    def test_pack_cache(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                self.assertIsNone(hub.cached("pos-compact"))
                self.assertEqual(hub.PACKS["depparse-compact"], "depparse")
                target = Path(root) / "models" / "pos-compact"
                target.mkdir(parents=True)
                for name in hub.PACK_REQUIRED:
                    (target / name).write_bytes(b"x")
                self.assertEqual(hub.cached("pos-compact"), target)
                self.assertEqual(hub.path("pos-compact"), target)
                self.assertEqual(hub.info("pos-compact")["path"], str(target))
                self.assertTrue(hub.remove("pos-compact"))
                self.assertFalse(hub.remove("pos-compact"))
                self.assertIsNone(hub.cached("pos-compact"))
                with self.assertRaises(ValueError):
                    hub.cached("unknown")
                with self.assertRaises(FileNotFoundError):
                    hub.path("ner-compact")

    def test_remove_and_info_see_a_partial_model_download(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                models = Path(root) / "models"
                models.mkdir(parents=True)
                part = models / "rishe-embed-fa-300.vec.part"
                part.write_bytes(b"half a download")

                # info reports the bytes on disk instead of claiming the model is absent.
                info = hub.info("rishe-embed-fa-300")
                self.assertIsNotNone(info)
                self.assertTrue(info["partial"])
                self.assertEqual(info["path"], str(part))
                self.assertEqual(info["bytes"], len(b"half a download"))

                # remove deletes the part file, so the disk space is actually freed.
                self.assertTrue(hub.remove("rishe-embed-fa-300"))
                self.assertFalse(part.exists())
                self.assertIsNone(hub.info("rishe-embed-fa-300"))
                self.assertFalse(hub.remove("rishe-embed-fa-300"))

    def test_info_always_carries_the_partial_key(self):
        """A caller writes one check, not `info.get("partial", False)`."""
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                models = Path(root) / "models"
                models.mkdir(parents=True)
                cached = models / "rishe-embed-fa-300.vec"
                cached.write_bytes(b"a cached model")

                info = hub.info("rishe-embed-fa-300")
                self.assertIsNotNone(info)
                self.assertIn("partial", info)
                self.assertFalse(info["partial"])
                self.assertEqual(
                    sorted(info), ["bytes", "kind", "name", "partial", "path"]
                )

    def test_pull_refuses_a_name_that_could_escape_the_cache(self):
        """The inferred filename is joined onto the cache dir, so it must be one name."""
        # A forward-slash traversal collapses to a plain name, which is safe.
        self.assertEqual(hub._safe_filename("../../escaped.bin"), "escaped.bin")
        # A backslash is only a separator on Windows, so it is refused outright
        # rather than collapsed: collapsing would make the same URL behave
        # differently per platform. This matches `infer_filename` in src/hub.rs.
        for name in ("..\\..\\escaped.bin", "a\\b.bin", "C:\\x.bin", "a:b.bin"):
            with self.assertRaises(ValueError, msg=name):
                hub._safe_filename(name)
        # A name with nothing usable in it is refused rather than guessed.
        for name in ("", ".", ".."):
            with self.assertRaises(ValueError, msg=name):
                hub._safe_filename(name)

    def test_pull_drops_a_query_string_from_the_name(self):
        self.assertEqual(hub._safe_filename("model.vec?download=true"), "model.vec")
        self.assertEqual(hub._safe_filename("model.vec#frag"), "model.vec")
        self.assertEqual(hub._safe_filename("a/b/model.vec"), "model.vec")

    def test_remove_and_info_see_a_partial_pack(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                pack = Path(root) / "models" / "pos-compact"
                pack.mkdir(parents=True)
                (pack / "labels.json").write_bytes(b"{}")

                # A half-pulled pack directory is not "missing": it occupies disk.
                info = hub.info("pos-compact")
                self.assertIsNotNone(info)
                self.assertTrue(info["partial"])
                self.assertEqual(info["kind"], "pack")
                self.assertEqual(info["bytes"], len(b"{}"))

                self.assertTrue(hub.remove("pos-compact"))
                self.assertFalse(pack.exists())
                self.assertIsNone(hub.info("pos-compact"))

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


class ManifestSchemaTest(unittest.TestCase):
    def test_absent_schema_means_one(self):
        self.assertEqual(hub._manifest_schema({}), 1)
        self.assertEqual(hub._manifest_schema({"tasks": {}}), 1)

    def test_schema_one_is_accepted(self):
        self.assertEqual(hub._manifest_schema({"schema": 1}), hub.MANIFEST_SCHEMA)

    def test_newer_schema_is_refused(self):
        with self.assertRaises(ValueError) as caught:
            hub._manifest_schema({"schema": 2})
        message = str(caught.exception)
        self.assertIn("schema 2", message)
        self.assertIn("reads schema 1", message)

    def test_non_integer_schema_is_refused(self):
        for schema in ("1", 1.5, True, None):
            with self.subTest(schema=schema), self.assertRaises(ValueError):
                hub._manifest_schema({"schema": schema})

    def test_verify_refuses_a_newer_manifest_schema(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):
                target = Path(root) / "models" / "pos-compact"
                target.mkdir(parents=True)
                for name in hub.PACK_REQUIRED:
                    (target / name).write_bytes(b"x")
                manifest_path = Path(root) / "models" / "rishe-models-manifest.json"
                manifest_path.write_text(
                    json.dumps({"schema": 2, "tasks": {}}), encoding="utf-8"
                )
                digest = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
                with patch.object(hub, "PACK_MANIFEST_SHA256", digest):
                    with self.assertRaisesRegex(ValueError, "schema 2"):
                        hub.verify("pos-compact")

    def test_pull_pack_refuses_a_newer_manifest_schema(self):
        with tempfile.TemporaryDirectory() as root:
            with patch.dict(os.environ, {"RISHE_HOME": root}):

                def fake_download(url, target, sha256, *, force=False):
                    path = Path(target)
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(
                        json.dumps({"schema": 2, "tasks": {}}), encoding="utf-8"
                    )
                    return path

                with patch.object(hub, "_download", side_effect=fake_download):
                    with self.assertRaisesRegex(ValueError, "schema 2"):
                        hub._pull_pack("pos-compact")


@unittest.skipUnless(
    DEPPARSE_PACK is not None and DEPPARSE_PACK.is_dir(),
    "RISHE_TEST_DEPPARSE_PACK is not set",
)
class DependencyParseTest(unittest.TestCase):
    def test_depparse(self):
        pipe = Pipeline(depparse_pack=DEPPARSE_PACK)
        dependencies = pipe.depparse("کتاب‌های خوب را به دوستانم می‌دهم.")
        self.assertTrue(dependencies)
        self.assertEqual(
            sum(item.head is None for item in dependencies),
            1,
        )
        for item in dependencies:
            if item.head is not None:
                self.assertLess(item.head, len(dependencies))
        batch = pipe.depparse_batch(["کتاب خوب است.", "به تهران رفت."])
        self.assertEqual(len(batch), 2)

    def test_cli_depparse(self):
        output = StringIO()
        with redirect_stdout(output):
            code = main(
                [
                    "depparse",
                    "--pack",
                    str(DEPPARSE_PACK),
                    "--json",
                    "کتاب‌های خوب را به دوستانم می‌دهم.",
                ]
            )
        self.assertEqual(code, 0)
        rows = json.loads(output.getvalue())
        self.assertEqual(sum(row["head"] is None for row in rows), 1)
        self.assertTrue(all("relation" in row for row in rows))


@unittest.skipUnless(PACK_ROOT is not None and PACK_ROOT.is_dir(), "RISHE_TEST_PACK is not set")
class ModelPackTest(unittest.TestCase):
    def test_pos(self):
        pack = ModelPack(str(PACK_ROOT / "pos"))
        self.assertEqual(pack.task, "pos")
        self.assertEqual(pack.max_len, 192)
        self.assertEqual(len(pack.labels()), 16)
        words = ["کتاب‌های", "خوب", "را", "به", "دوستانم", "می‌دهم"]
        result = pack.tags_from_words(words)
        self.assertEqual([word for word, _ in result], words)
        self.assertEqual(result[0][1], "NOUN")
        self.assertEqual(result[1][1], "ADJ")
        self.assertEqual(result[-1][1], "VERB")
        self.assertTrue(pack.tags("کتاب‌های خوب"))

    def test_ner(self):
        pack = ModelPack(str(PACK_ROOT / "ner"))
        self.assertEqual(pack.task, "ner")
        self.assertEqual(len(pack.labels()), 21)
        result = pack.tags_from_words(["سرلشگر", "فیروزآبادی", "به", "پادشاه", "عربستان", "گفت"])
        self.assertEqual(len(result), 6)
        self.assertTrue(all(label in pack.labels() for _, label in result))
        self.assertTrue(any(label != "O" for _, label in result))

    def test_ezafe(self):
        pack = ModelPack(str(PACK_ROOT / "ezafe"))
        self.assertEqual(pack.task, "ezafe")
        self.assertEqual(pack.max_len, 128)
        result = pack.tags_from_words(["کتابخانه", "مرکزی", "دانشگاه", "شریف"])
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
        self.assertEqual(len(pack.tags_batch(["کتاب‌های خوب", "خوب است"])), 2)

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
