import io
import json
import unittest
from contextlib import redirect_stdout

from rishe.cli import main


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
            ["mask", "به https://a.b برو", "--stages", "url"]
        )
        self.assertEqual(code, 0)
        self.assertEqual(output, "به <URL> برو")


if __name__ == "__main__":
    unittest.main()
