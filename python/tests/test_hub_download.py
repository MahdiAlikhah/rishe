import hashlib
import http.server
import os
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from rishe import hub

PAYLOAD = b"rishe" * 4096


class FlakyHandler(http.server.BaseHTTPRequestHandler):
    attempts = 0

    def do_GET(self):
        type(self).attempts += 1
        if type(self).attempts == 1:
            self.send_error(503)
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(PAYLOAD)))
        self.end_headers()
        self.wfile.write(PAYLOAD)

    def log_message(self, *_args):
        pass


class RangeHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        range_header = self.headers.get("Range")
        start = int(range_header.split("=", 1)[1].split("-", 1)[0]) if range_header else 0
        body = PAYLOAD[start:]
        self.send_response(206 if range_header else 200)
        if range_header:
            self.send_header(
                "Content-Range", f"bytes {start}-{len(PAYLOAD) - 1}/{len(PAYLOAD)}"
            )
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


class NotFoundHandler(http.server.BaseHTTPRequestHandler):
    """Answer every request with 404; the client must not retry a 4xx."""

    hits = 0

    def do_GET(self):
        type(self).hits += 1
        self.send_error(404)

    def log_message(self, *_args):
        pass


class Always416Handler(http.server.BaseHTTPRequestHandler):
    """Answer every request with 416, like a resume of a complete part."""

    hits = 0

    def do_GET(self):
        type(self).hits += 1
        self.send_error(416)

    def log_message(self, *_args):
        pass


class CachePathsTest(unittest.TestCase):
    def test_cache_dir_prefers_home_like_the_rust_binary(self):
        # `Path.home()` reads USERPROFILE on Windows; the Rust binary prefers
        # HOME, and an empty RISHE_HOME is a real path, not "unset".
        with tempfile.TemporaryDirectory() as root:
            env = {"HOME": root, "USERPROFILE": str(Path(root) / "profile")}
            with mock.patch.dict(os.environ, env, clear=False):
                os.environ["RISHE_HOME"] = root
                expected = Path(root) / "models"
                self.assertEqual(hub.cache_dir(), expected)
                os.environ.pop("RISHE_HOME")
                self.assertEqual(
                    hub.cache_dir(), Path(root) / ".cache" / "rishe" / "models"
                )

    def test_remove_clears_a_stale_pack_lock_and_manifest_part(self):
        with tempfile.TemporaryDirectory() as root, mock.patch.dict(
            os.environ, {"RISHE_HOME": root}, clear=False
        ):
            models = Path(root) / "models"
            (models / "pos-compact").mkdir(parents=True)
            manifest = models / "rishe-models-manifest.json"
            manifest.write_text("{}", encoding="utf-8")
            part = Path(str(manifest) + ".part")
            part.write_bytes(b"x")
            lock = Path(str(manifest) + ".lock")
            lock.write_bytes(b"x")
            self.assertTrue(hub.remove("pos-compact"))
            self.assertFalse(part.exists())
            self.assertFalse(lock.exists())


class DownloadTest(unittest.TestCase):
    def serve(self, handler):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(thread.join)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return server

    def test_retry(self):
        server = self.serve(FlakyHandler)
        FlakyHandler.attempts = 0
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            hub._download(
                f"http://127.0.0.1:{server.server_port}/payload.bin",
                target,
                hashlib.sha256(PAYLOAD).hexdigest(),
            )
            self.assertEqual(target.read_bytes(), PAYLOAD)
        self.assertGreaterEqual(FlakyHandler.attempts, 2)

    def test_resume(self):
        server = self.serve(RangeHandler)
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            part = target.with_name(target.name + ".part")
            part.parent.mkdir(parents=True, exist_ok=True)
            part.write_bytes(PAYLOAD[:1000])
            hub._download(
                f"http://127.0.0.1:{server.server_port}/payload.bin",
                target,
                hashlib.sha256(PAYLOAD).hexdigest(),
            )
            self.assertEqual(target.read_bytes(), PAYLOAD)
            self.assertFalse(part.exists())

    def test_existing_corrupt_target_is_replaced(self):
        server = self.serve(RangeHandler)
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            target.write_bytes(b"stale bytes")
            hub._download(
                f"http://127.0.0.1:{server.server_port}/payload.bin",
                target,
                hashlib.sha256(PAYLOAD).hexdigest(),
            )
            self.assertEqual(target.read_bytes(), PAYLOAD)

    def test_a_stale_prefix_on_a_resume_restarts_instead_of_failing(self):
        """A wrong local prefix must not permanently block the pull."""
        server = self.serve(RangeHandler)
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            part = target.with_name(target.name + ".part")
            part.parent.mkdir(parents=True, exist_ok=True)
            # Same length as the real prefix, different bytes: only the digest
            # step can notice, not a 416.
            part.write_bytes(b"X" * 1000)
            hub._download(
                f"http://127.0.0.1:{server.server_port}/payload.bin",
                target,
                hashlib.sha256(PAYLOAD).hexdigest(),
            )
            self.assertEqual(target.read_bytes(), PAYLOAD)
            self.assertFalse(part.exists())
    def test_a_missing_file_is_not_retried(self):
        """`HTTPError` is a `URLError`, so a 404 used to be fetched three times."""
        server = self.serve(NotFoundHandler)
        NotFoundHandler.hits = 0
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaises(RuntimeError) as caught:
                hub._download(
                    f"http://127.0.0.1:{server.server_port}/missing.bin",
                    Path(root) / "missing.bin",
                    hashlib.sha256(PAYLOAD).hexdigest(),
                )
        self.assertEqual(NotFoundHandler.hits, 1)
        self.assertIn("404", str(caught.exception))

    def test_a_custom_sha256_must_be_hex(self):
        """Only the length was checked, so `'z' * 64` reached the network."""
        with self.assertRaises(ValueError) as caught:
            hub.pull("https://example.com/model.vec", sha256="z" * 64)
        self.assertIn("hex", str(caught.exception))

    def test_a_held_lock_refuses_a_second_download(self):
        """Two pulls shared one `.part` file and one renamed it under the other."""
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            (Path(root) / "payload.bin.lock").write_bytes(b"")
            with self.assertRaises(RuntimeError) as caught:
                hub._download("http://127.0.0.1:1/x", target, "0" * 64)
            self.assertIn("another rishe process is downloading", str(caught.exception))
            self.assertFalse((Path(root) / "payload.bin.part").exists())

    def test_a_stale_lock_is_taken_over(self):
        """A crashed process must not block the download forever."""
        server = self.serve(RangeHandler)
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            lock = Path(root) / "payload.bin.lock"
            lock.write_bytes(b"")
            os.utime(lock, (time.time() - 7200,) * 2)
            hub._download(
                f"http://127.0.0.1:{server.server_port}/payload.bin",
                target,
                hashlib.sha256(PAYLOAD).hexdigest(),
            )
            self.assertEqual(target.read_bytes(), PAYLOAD)
            self.assertFalse(lock.exists(), "lock left behind")

    def test_unlock_clears_a_leaked_lock_without_dropping_the_payload(self):
        """A killed pull blocked retries for an hour with no way to clear it."""
        with tempfile.TemporaryDirectory() as root, mock.patch.dict(
            os.environ, {"RISHE_HOME": root}, clear=False
        ):
            models = Path(root) / "models"
            models.mkdir(parents=True)
            payload = models / "rishe-embed-fa-300.vec"
            payload.write_bytes(b"payload")
            lock = models / "rishe-embed-fa-300.vec.lock"
            lock.write_bytes(b"1234 0")
            self.assertTrue(hub.unlock("rishe-embed-fa-300"))
            self.assertFalse(lock.exists())
            self.assertTrue(payload.is_file(), "unlocking must keep the payload")
            self.assertFalse(hub.unlock("rishe-embed-fa-300"), "no lock to clear")
            with self.assertRaises(ValueError):
                hub.unlock("not-a-model")

    def test_a_foreign_lock_is_not_deleted_by_its_previous_owner(self):
        """The old download deleted whatever lock was there, including a new one."""
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            lock = Path(root) / "payload.bin.lock"
            lock.write_bytes(b"other 1")
            # A lock owned by someone else must survive a failed download.
            with self.assertRaises(RuntimeError):
                hub._download("http://127.0.0.1:1/x", target, "0" * 64)
            self.assertTrue(lock.is_file(), "a foreign lock was deleted")

    def test_a_complete_part_is_promoted_instead_of_re_downloaded(self):
        """A 416 on a complete part used to delete the transfer and start over."""
        server = self.serve(Always416Handler)
        Always416Handler.hits = 0
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "payload.bin"
            (Path(root) / "payload.bin.part").write_bytes(PAYLOAD)
            hub._download(
                f"http://127.0.0.1:{server.server_port}/payload.bin",
                target,
                hashlib.sha256(PAYLOAD).hexdigest(),
            )
            self.assertEqual(target.read_bytes(), PAYLOAD)
            self.assertEqual(Always416Handler.hits, 1, "should promote, not refetch")

    def test_a_device_name_is_refused_before_anything_is_written(self):
        """`NUL` downloads and verifies but can never be renamed into place."""
        for name in ("NUL", "nul.vec", "COM1", "LPT9.bin"):
            with self.assertRaises(ValueError):
                hub._safe_filename(name)
        self.assertEqual(hub._safe_filename("nulls.vec"), "nulls.vec")

    def test_a_custom_url_cannot_replace_a_registered_model(self):
        """The cache file of a pinned model must not accept unverified bytes."""
        name = hub.MODELS["rishe-embed-fa-300"].filename
        with self.assertRaises(ValueError) as caught:
            hub.pull(f"http://mirror/{name}", sha256="0" * 64)
        self.assertIn("belongs to the registered model", str(caught.exception))

    def test_only_http_urls_are_downloaded(self):
        """`file:`/`ftp:`/`data:` copied local bytes into the model cache."""
        for url in (
            "file:///etc/passwd",
            "ftp://host/model.vec",
            "data:text/plain,hello",
            "//host/model.vec",
        ):
            with self.assertRaises(ValueError) as caught:
                hub.pull(url, sha256="0" * 64)
            self.assertIn("http", str(caught.exception), url)

    def test_a_backslash_in_a_url_filename_is_refused(self):
        """`Path(...).name` splits on "\\\\" on Windows only, so one URL got two answers.

        `http://x/a\\b.bin` was `b.bin` here and `ValueError` in the Rust binary.
        """
        with self.assertRaises(ValueError):
            hub.pull("http://x/a\\b.bin", sha256="0" * 64)


if __name__ == "__main__":
    unittest.main()
