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

    def do_GET(self):
        name = self.path.lstrip("/").split("/")[-1]
        body = {
            "manifest.json": MANIFEST,
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


if __name__ == "__main__":
    unittest.main()
