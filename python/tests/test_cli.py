import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from rishe.cli import main

MODEL = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "mini-embed.vec"


class CliTest(unittest.TestCase):
    def run_cli(self, argv):
        output = io.StringIO()
        with redirect_stdout(output):
            code = main(argv)
        return code, output.getvalue().strip()

    def test_normalize(self):
        code, output = self.run_cli(["normalize", "مي كنم"])
        self.assertEqual(code, 0)
        self.assertEqual(output, "می‌کنم")

    def test_tokenize_json(self):
        code, output = self.run_cli(["tokenize", "سلام دنیا", "--json"])
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(output), ["سلام", "دنیا"])

    def test_lemmatize(self):
        code, output = self.run_cli(["lemmatize", "می‌روم", "--json"])
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(output), ["رفت"])

    def test_mask(self):
        code, output = self.run_cli(
            ["mask", "به https://a.b برو", "--maskers", "url"]
        )
        self.assertEqual(code, 0)
        self.assertEqual(output, "به <URL> برو")

    def test_mask_accepts_the_deprecated_stages_flag(self):
        code, output = self.run_cli(
            ["mask", "به https://a.b برو", "--stages", "url"]
        )
        self.assertEqual(code, 0)
        self.assertEqual(output, "به <URL> برو")

    def test_utf8_output_with_non_utf8_stdio(self):
        environment = dict(os.environ, PYTHONIOENCODING="cp1252")
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "normalize", "مي كنم"],
            capture_output=True,
            env=environment,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        self.assertEqual(result.stdout.decode("utf-8").strip(), "می‌کنم")

    def test_embed_vector(self):
        code, output = self.run_cli(
            ["embed", "کتاب", "--model", str(MODEL), "--json"]
        )
        self.assertEqual(code, 0)
        vector = json.loads(output)
        self.assertEqual(len(vector), 8)
        self.assertAlmostEqual(vector[0], 0.07038601, places=5)

    def test_similar_returns_scored_rows(self):
        code, output = self.run_cli(
            ["similar", "کتاب", "--model", str(MODEL), "--top", "2", "--json"]
        )
        self.assertEqual(code, 0)
        rows = json.loads(output)
        self.assertEqual(len(rows), 2)
        self.assertIn("word", rows[0])
        self.assertIn("score", rows[0])

    def test_similarity_identical_words(self):
        code, output = self.run_cli(
            ["similarity", "کتاب", "کتاب", "--model", str(MODEL)]
        )
        self.assertEqual(code, 0)
        self.assertAlmostEqual(float(output), 1.0, places=5)

    def test_similar_unknown_word_exits_2(self):
        code, _ = self.run_cli(
            ["similar", "ناموجود", "--model", str(MODEL)]
        )
        self.assertEqual(code, 2)

    def test_serve_similarity(self):
        request = json.dumps({"command": "similarity", "a": "کتاب", "b": "کتاب"}) + "\n"
        output = io.StringIO()
        with mock.patch("sys.stdin", io.StringIO(request)), redirect_stdout(output):
            code = main(["serve", "--model", str(MODEL)])
        self.assertEqual(code, 0)
        response = json.loads(output.getvalue().strip())
        self.assertTrue(response["ok"])
        self.assertAlmostEqual(response["result"]["score"], 1.0, places=5)

    def test_serve_without_model_reports_error(self):
        request = json.dumps({"command": "similar", "word": "کتاب"}) + "\n"
        output = io.StringIO()
        with mock.patch("sys.stdin", io.StringIO(request)), redirect_stdout(output):
            code = main(["serve"])
        self.assertEqual(code, 0)
        response = json.loads(output.getvalue().strip())
        self.assertFalse(response["ok"])
        self.assertIn("no embedding model", response["error"])

    def test_hub_path(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "models" / "rishe-embed-fa-300.vec"
            target.parent.mkdir(parents=True)
            target.write_text("1 1\nx 1\n", encoding="utf-8")
            with mock.patch.dict(os.environ, {"RISHE_HOME": root}):
                code, output = self.run_cli(["hub", "path", "rishe-embed-fa-300"])
        self.assertEqual(code, 0)
        self.assertTrue(output.endswith("rishe-embed-fa-300.vec"))


if __name__ == "__main__":
    unittest.main()
