import hashlib
import http.server
import tempfile
import threading
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
