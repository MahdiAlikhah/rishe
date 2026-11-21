import hashlib
import http.server
import json
import os
import tempfile
import threading
import unittest
from pathlib import Path

from rishe import hub

MODEL = b"model-bytes" * 64
TOKENIZER = b'{"tokenizer": true}'
LABELS = b'{"labels": ["X"]}'
MANIFEST = json.dumps(
    {
        "schema": 1,
        "tasks": {
            "pos": {
                "files": {
                    "model.onnx": {"sha256": hashlib.sha256(MODEL).hexdigest()},
                    "tokenizer.json": {"sha256": hashlib.sha256(TOKENIZER).hexdigest()},
                    "labels.json": {"sha256": hashlib.sha256(LABELS).hexdigest()},
                }
            }
        },
    }
).encode()


class PackHandler(http.server.BaseHTTPRequestHandler):
    """Serve the three pack files plus the manifest the hub expects."""

    hits = {}
    # Overridden by tests that need a different manifest body.
    manifest = MANIFEST

    def do_GET(self):
        name = self.path.lstrip("/").split("/")[-1]
        body = {
            "manifest.json": type(self).manifest,
            "model.onnx": MODEL,
            "tokenizer.json": TOKENIZER,
            "labels.json": LABELS,
        }.get(name)
        if body is None:
            self.send_error(404)
            return
        type(self).hits[name] = type(self).hits.get(name, 0) + 1
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


class PackPullTest(unittest.TestCase):
    def setUp(self):
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), PackHandler)
        thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)

        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.addCleanup(os.environ.pop, "RISHE_HOME", None)
        self.addCleanup(os.environ.pop, "RISHE_MODEL_BASE", None)
        os.environ["RISHE_HOME"] = root.name
        os.environ["RISHE_MODEL_BASE"] = f"http://127.0.0.1:{self.server.server_port}"

        self.addCleanup(setattr, hub, "PACK_MANIFEST_SHA256", hub.PACK_MANIFEST_SHA256)
        hub.PACK_MANIFEST_SHA256 = hashlib.sha256(MANIFEST).hexdigest()
        PackHandler.hits = {}

    def test_a_full_looking_but_wrong_pack_is_downloaded_again(self):
        """The pull returned any directory holding the three file names, so a
        stale or half-written pack was kept while `verify` called it invalid."""
        target = hub._pull_pack("pos-compact")
        (target / "model.onnx").write_bytes(b"corrupted")
        PackHandler.hits = {}

        again = hub._pull_pack("pos-compact")
        self.assertEqual(again, target)
        self.assertEqual((target / "model.onnx").read_bytes(), MODEL)
        self.assertIn("model.onnx", PackHandler.hits)
        self.assertTrue(hub.verify("pos-compact"))

    def test_a_verified_pack_is_not_downloaded_again(self):
        hub._pull_pack("pos-compact")
        PackHandler.hits = {}
        hub._pull_pack("pos-compact")
        self.assertEqual(PackHandler.hits, {})

    def test_a_cached_manifest_is_not_downloaded_again(self):
        """The manifest is sha-pinned; re-fetching it each pull fails offline."""
        hub._pull_pack("pos-compact")
        (hub.cache_dir() / "pos-compact" / "model.onnx").write_bytes(b"corrupted")
        PackHandler.hits = {}
        hub._pull_pack("pos-compact")
        self.assertIn("model.onnx", PackHandler.hits)
        self.assertNotIn("manifest.json", PackHandler.hits)


class StandalonePackTest(unittest.TestCase):
    """A pack with its own manifest, which must not disturb the task packs."""

    # A standalone pack serves the `sentence` task, not `pos`.
    MANIFEST = json.dumps(
        {
            "schema": 1,
            "tasks": {
                "sentence": {
                    "files": {
                        "model.onnx": {"sha256": hashlib.sha256(MODEL).hexdigest()},
                        "tokenizer.json": {"sha256": hashlib.sha256(TOKENIZER).hexdigest()},
                        "labels.json": {"sha256": hashlib.sha256(LABELS).hexdigest()},
                    }
                }
            },
        }
    ).encode()

    def setUp(self):
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), PackHandler)
        thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)

        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.addCleanup(os.environ.pop, "RISHE_HOME", None)
        os.environ["RISHE_HOME"] = root.name

        self.addCleanup(setattr, PackHandler, "manifest", PackHandler.manifest)
        PackHandler.manifest = self.MANIFEST

        base = f"http://127.0.0.1:{self.server.server_port}"
        self.addCleanup(
            setattr, hub, "STANDALONE_PACKS", dict(hub.STANDALONE_PACKS)
        )
        hub.STANDALONE_PACKS = {
            "test-vector-pack": (
                "sentence",
                base,
                hashlib.sha256(self.MANIFEST).hexdigest(),
            )
        }
        PackHandler.hits = {}

    def test_a_standalone_pack_is_recognised(self):
        self.assertTrue(hub.is_pack("test-vector-pack"))
        self.assertTrue(hub.is_pack("pos-compact"))
        self.assertFalse(hub.is_pack("rishe-embed-fa-300-v2"))
        self.assertEqual(hub._pack_task("test-vector-pack"), "sentence")
        self.assertEqual(hub._pack_task("pos-compact"), "pos")

    def test_a_standalone_pack_reads_its_own_manifest(self):
        # Sharing the task-pack manifest slot would let two packs with different
        # pins overwrite each other in one cache file.
        self.assertNotEqual(
            hub._pack_manifest_name("test-vector-pack"),
            hub._pack_manifest_name("pos-compact"),
        )
        self.assertEqual(
            hub._pack_manifest_name("test-vector-pack"),
            "test-vector-pack-manifest.json",
        )

    def test_a_standalone_pack_does_not_pollute_the_shared_manifest(self):
        """Publishing a new pack must not overwrite the pinned shared manifest."""
        hub._pull_pack("test-vector-pack")
        self.assertFalse(
            (hub.cache_dir() / "rishe-models-manifest.json").is_file(),
            "the shared manifest must not be fetched for a standalone pack",
        )
        self.assertTrue(
            (hub.cache_dir() / "test-vector-pack-manifest.json").is_file()
        )

    def test_a_standalone_pack_file_url_skips_the_task_segment(self):
        # Task packs serve `<base>/<task>/<file>`; a standalone base is already
        # the pack directory, so adding the task 404s every file.
        url = hub._pack_file_url("test-vector-pack", "model.onnx")
        self.assertTrue(url.endswith("/model.onnx"), url)
        self.assertNotIn("/sentence/", url)
        self.assertIn("/pos/", hub._pack_file_url("pos-compact", "model.onnx"))

    def test_a_standalone_pack_pulls_and_verifies(self):
        target = hub._pull_pack("test-vector-pack")
        self.assertTrue((target / "model.onnx").is_file())
        self.assertTrue(hub.verify("test-vector-pack"))
        # A corrupted file is caught, like any other pack.
        (target / "model.onnx").write_bytes(b"corrupted")
        self.assertFalse(hub.verify("test-vector-pack"))

    def test_the_four_task_packs_still_use_the_shared_manifest(self):
        for name in ("pos-compact", "ner-compact", "ezafe-compact", "depparse-compact"):
            self.assertEqual(
                hub._pack_manifest_name(name), "rishe-models-manifest.json"
            )
            self.assertEqual(hub._pack_manifest_digest(name), hub.PACK_MANIFEST_SHA256)


if __name__ == "__main__":
    unittest.main()
