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

import rishe
from rishe import cli
from rishe.cli import main

MODEL = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "mini-embed.vec"


class CliTest(unittest.TestCase):
    def run_cli(self, argv):
        output = io.StringIO()
        with redirect_stdout(output):
            code = main(argv)
        return code, output.getvalue().strip()

    def test_stdin_keeps_the_document_and_drops_the_shell_newline(self):
        """`trim()` used to delete the blank line a heredoc starts with."""
        with mock.patch("sys.stdin", io.StringIO("\n\nمتن\n")):
            self.assertEqual(cli._read_text([]), "\n\nمتن")
        with mock.patch("sys.stdin", io.StringIO("  متن  \r\n")):
            self.assertEqual(cli._read_text([]), "  متن  ")

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

    def test_mask_rejects_the_removed_stages_flag(self):
        # `--stages` was the deprecated spelling; 1.0.0 removed it.
        with self.assertRaises(SystemExit) as caught:
            main(["mask", "x", "--stages", "url"])
        self.assertEqual(caught.exception.code, 2)

    def test_several_positional_words_are_joined(self):
        """The Rust binary joins the words; Python used to exit 2."""
        code, output = self.run_cli(["normalize", "مي", "كنم"])
        self.assertEqual(code, 0)
        self.assertEqual(output, "می‌کنم")

    def test_every_pack_command_requires_pack(self):
        """pos/ner/ezafe fell back to the Hub cache while depparse required
        `--pack`; all four now follow the Rust rule."""
        for command in ("pos", "ner", "ezafe", "depparse"):
            code, _ = self.run_cli([command, "متن"])
            self.assertEqual(code, 1, command)

    def test_a_byte_order_mark_on_stdin_is_ignored(self):
        """`utf-8` keeps the BOM as part of the first word, so `rishe date` failed."""
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "date"],
            input="\ufeff1402/05/12".encode("utf-8"),
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))
        self.assertEqual(result.stdout.decode("utf-8").strip(), "2023-08-03")

    def test_invalid_utf8_on_stdin_is_an_error_not_a_traceback(self):
        """The stdin read used to sit outside the handler and crash; then it
        silently replaced the bytes while the Rust binary rejected them."""
        for command in ("date", "tokenize", "normalize"):
            result = subprocess.run(
                [sys.executable, "-m", "rishe.cli", command],
                input=b"\xff\xfe\xff",
                capture_output=True,
                check=False,
            )
            self.assertNotIn(b"Traceback", result.stderr, command)
            self.assertEqual(result.returncode, 1, command)

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

    def test_similar_unknown_word_exits_1(self):
        code, _ = self.run_cli(
            ["similar", "ناموجود", "--model", str(MODEL)]
        )
        self.assertEqual(code, 1)

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

    def test_serve_treats_a_wrong_typed_field_as_absent(self):
        # docs/serve-schema.md: a field of the wrong JSON type counts as absent.
        # The Rust binary behaves this way; this server must agree with it.
        text = "به info@a.co بنویس"
        default = rishe.mask(text)
        cases = [
            ({"command": "mask", "text": text, "maskers": "email"}, default),
            (
                {"command": "mask", "text": text, "maskers": [3, "email"]},
                rishe.mask(text, maskers=["email"]),
            ),
            ({"command": "mask", "text": text, "mask_token": 7}, default),
        ]
        payload = "".join(json.dumps(request) + "\n" for request, _ in cases)
        output = io.StringIO()
        with mock.patch("sys.stdin", io.StringIO(payload)), redirect_stdout(output):
            code = main(["serve"])
        self.assertEqual(code, 0)
        responses = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual(len(responses), len(cases))
        for response, (_, expected) in zip(responses, cases):
            self.assertTrue(response["ok"], response)
            self.assertEqual(response["result"], expected)

    def test_serve_similar_accepts_a_null_count(self):
        # `n` of the wrong type is absent, so the documented default of 5 applies.
        request = json.dumps({"command": "similar", "word": "کتاب", "n": None}) + "\n"
        output = io.StringIO()
        with mock.patch("sys.stdin", io.StringIO(request)), redirect_stdout(output):
            code = main(["serve", "--model", str(MODEL)])
        self.assertEqual(code, 0)
        response = json.loads(output.getvalue().strip())
        self.assertTrue(response["ok"], response)
        self.assertLessEqual(len(response["result"]), 5)

    def test_hub_path(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "models" / "rishe-embed-fa-300.vec"
            target.parent.mkdir(parents=True)
            target.write_text("1 1\nx 1\n", encoding="utf-8")
            with mock.patch.dict(os.environ, {"RISHE_HOME": root}):
                code, output = self.run_cli(["hub", "path", "rishe-embed-fa-300"])
        self.assertEqual(code, 0)
        self.assertTrue(output.endswith("rishe-embed-fa-300.vec"))


class ExitCodeContractTest(unittest.TestCase):
    """End-to-end exit statuses, shared with the Rust binary (docs/errors.md).

    0 is success (including ``--help``/``--version``), 2 is a usage error that
    argparse rejects, and 1 is every failure after a valid command line.
    """

    def run_cli(self, *argv, env=None):
        environment = dict(os.environ, **(env or {}))
        return subprocess.run(
            [sys.executable, "-m", "rishe.cli", *argv],
            input=b"",
            capture_output=True,
            check=False,
            env=environment,
        )

    def assert_exit(self, expected, *argv, env=None):
        result = self.run_cli(*argv, env=env)
        self.assertEqual(
            result.returncode,
            expected,
            result.stderr.decode("utf-8", "replace"),
        )
        return result

    def test_help_exits_0(self):
        result = self.assert_exit(0, "--help")
        self.assertIn("tokenize", result.stdout.decode("utf-8", "replace"))

    def test_no_arguments_exits_0(self):
        result = self.assert_exit(0)
        self.assertIn("usage", result.stdout.decode("utf-8", "replace").lower())

    def test_version_exits_0(self):
        for flag in ("--version", "-V"):
            result = self.assert_exit(0, flag)
            self.assertTrue(result.stdout.strip(), flag)

    def test_tokenize_exits_0(self):
        result = self.assert_exit(0, "tokenize", "سلام دنیا", "--json")
        self.assertEqual(json.loads(result.stdout.decode("utf-8")), ["سلام", "دنیا"])

    def test_date_exits_0(self):
        result = self.assert_exit(0, "date", "۱۲ مرداد ۱۴۰۲")
        self.assertEqual(result.stdout.decode("utf-8").strip(), "2023-08-03")

    def test_unknown_command_exits_2(self):
        self.assert_exit(2, "bogus")

    def test_unknown_option_exits_2(self):
        self.assert_exit(2, "tokenize", "سلام", "--bogus")

    def test_missing_option_value_exits_2(self):
        self.assert_exit(2, "similar", "کتاب", "--model")

    def test_wrong_argument_count_exits_2(self):
        self.assert_exit(2, "similarity", "کتاب")

    def test_missing_embedding_file_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            missing = Path(root) / "nope.vec"
            result = self.assert_exit(1, "embed", "کتاب", "--model", str(missing))
        self.assertIn("not found", result.stderr.decode("utf-8", "replace"))

    def test_corrupt_embedding_file_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            broken = Path(root) / "broken.vec"
            broken.write_text("this is not a word2vec header\n", encoding="utf-8")
            self.assert_exit(1, "embed", "کتاب", "--model", str(broken))

    def test_out_of_vocabulary_similar_exits_1(self):
        self.assert_exit(1, "similar", "ناموجود", "--model", str(MODEL))

    def test_out_of_vocabulary_similarity_exits_1(self):
        self.assert_exit(1, "similarity", "کتاب", "ناموجود", "--model", str(MODEL))

    def test_invalid_masker_exits_1(self):
        result = self.assert_exit(1, "mask", "سلام", "--maskers", "bogus")
        self.assertIn("unknown masker", result.stderr.decode("utf-8", "replace"))

    def test_top_below_one_is_a_usage_error(self):
        """Rust rejects the same command line with exit 2, not 1."""
        self.assert_exit(2, "similar", "کتاب", "--top", "0", "--model", str(MODEL))
        self.assert_exit(2, "similar", "کتاب", "--top", "-1", "--model", str(MODEL))

    def test_unparseable_date_exits_1(self):
        self.assert_exit(1, "date", "این یک تاریخ نیست")

    def test_missing_pack_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            missing = Path(root) / "missing-pack"
            self.assert_exit(1, "pos", "سلام", "--pack", str(missing))

    def test_serve_startup_failure_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            missing = Path(root) / "nope.vec"
            self.assert_exit(1, "serve", "--model", str(missing))

    def test_hub_verify_missing_model_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            self.assert_exit(
                1,
                "hub",
                "verify",
                "rishe-embed-fa-300-v2",
                env={"RISHE_HOME": root},
            )

    def test_hub_unknown_name_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            self.assert_exit(
                1, "hub", "path", "not-a-model", env={"RISHE_HOME": root}
            )

    def test_hub_pull_unknown_name_exits_1(self):
        with tempfile.TemporaryDirectory() as root:
            self.assert_exit(
                1, "hub", "pull", "not-a-model", env={"RISHE_HOME": root}
            )

    def test_hub_unlock_reports_no_lock(self):
        """An interrupted pull left a lock that refused retries for an hour."""
        with tempfile.TemporaryDirectory() as root:
            env = {"RISHE_HOME": root}
            result = self.run_cli("hub", "unlock", "pos-compact", env=env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), b"no lock")
            lock = Path(root) / "models" / "pos-compact.lock"
            lock.parent.mkdir(parents=True, exist_ok=True)
            lock.write_bytes(b"1234 0")
            result = self.run_cli("hub", "unlock", "pos-compact", env=env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), b"unlocked")
            self.assertFalse(lock.exists())

    def test_hub_list_rejects_an_extra_word_like_the_rust_binary(self):
        """`hub list extra` used to be accepted here and rejected there."""
        with tempfile.TemporaryDirectory() as root:
            self.assert_exit(2, "hub", "list", "extra", env={"RISHE_HOME": root})

    def test_hub_pull_refuses_a_pin_for_a_task_pack(self):
        # A pack is verified against the registry manifest, so a caller digest has
        # nothing to apply to; ignoring it read as a pin that never happened.
        with tempfile.TemporaryDirectory() as root:
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rishe.cli",
                    "hub",
                    "pull",
                    "pos-compact",
                    "--sha256",
                    "0" * 64,
                ],
                capture_output=True,
                check=False,
                env={**os.environ, "RISHE_HOME": root},
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn(b"task packs do not accept", result.stderr)

    def test_serve_does_not_report_request_errors_as_exit_codes(self):
        request = json.dumps({"command": "nope"}).encode("utf-8") + b"\n"
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "serve"],
            input=request,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        self.assertFalse(json.loads(result.stdout.decode("utf-8"))["ok"])

    def test_depparse_without_pack_exits_1(self):
        result = self.assert_exit(1, "depparse", "سلام")
        self.assertIn(
            "depparse needs --pack", result.stderr.decode("utf-8", "replace")
        )

    def test_depparse_pack_without_value_exits_2(self):
        self.assert_exit(2, "depparse", "سلام", "--pack")


class ServeProtocolTest(unittest.TestCase):
    """`rishe serve` speaks protocol 1 (docs/serve-schema.md)."""

    def serve(self, *lines, options=()):
        payload = "".join(f"{line}\n" for line in lines).encode("utf-8")
        return subprocess.run(
            [sys.executable, "-m", "rishe.cli", "serve", *options],
            input=payload,
            capture_output=True,
            check=False,
        )

    def serve_one(self, request, *options):
        result = self.serve(request, options=options)
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        responses = result.stdout.decode("utf-8").splitlines()
        self.assertEqual(len(responses), 1, responses)
        return json.loads(responses[0])

    def test_mask_request_accepts_the_legacy_stages_field(self):
        # docs/serve-schema.md: protocol 1 still accepts "stages" for 0.x clients.
        response = self.serve_one(
            '{"command":"mask","text":"info@a.co","stages":["email"]}'
        )
        self.assertEqual(response, {"ok": True, "result": "<EMAIL>"})

    def cli_version(self):
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "--version"],
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        return result.stdout.decode("utf-8").split()[-1]

    def test_version_handshake_shape(self):
        response = self.serve_one('{"command":"version"}')
        self.assertEqual(
            response,
            {
                "ok": True,
                "result": {
                    "name": "rishe",
                    "protocol": 1,
                    "version": self.cli_version(),
                },
            },
        )

    def test_date_command_matches_the_cli_payload(self):
        response = self.serve_one('{"command":"date","text":"1402/05/12"}')
        self.assertEqual(
            response,
            {
                "ok": True,
                "result": {
                    "jalali": {"year": 1402, "month": 5, "day": 12},
                    "gregorian": {"year": 2023, "month": 8, "day": 3},
                    "iso": "2023-08-03",
                },
            },
        )

    def test_date_command_failure_is_an_error_envelope(self):
        response = self.serve_one('{"command":"date","text":"not a date"}')
        self.assertFalse(response["ok"])
        self.assertNotIn("result", response)
        self.assertIn("cannot parse", response["error"])

    def test_similarity_without_b_is_an_error_envelope(self):
        response = self.serve_one(
            '{"command":"similarity","a":"کتاب"}', "--model", str(MODEL)
        )
        self.assertFalse(response["ok"])
        self.assertNotIn("result", response)
        self.assertEqual(response["error"], "similarity needs `a` and `b`")

    def test_non_string_text_is_treated_as_absent(self):
        response = self.serve_one('{"command":"tokenize","text":5}')
        self.assertEqual(response, {"ok": True, "result": []})

    def test_serve_accepts_comma_separated_and_space_separated_maskers(self):
        for value in ("url,email", "email"):
            with self.subTest(maskers=value):
                response = self.serve_one(
                    '{"command":"mask","text":"info@a.co"}', "--maskers", value
                )
                self.assertEqual(response, {"ok": True, "result": "<EMAIL>"})

    def test_rejects_an_oversized_line_and_keeps_serving(self):
        # An unbounded line read grows its buffer until it finds a newline, so a
        # client that never sends one grows the server without bound. The
        # oversized line gets the error envelope, its remainder is discarded, and
        # the next real request is still served (docs/serve-schema.md).
        request = (
            b"x" * ((1 << 20) + 1)
            + b"\n"
            + '{"command":"tokenize","text":"کتاب خوب است"}\n'.encode("utf-8")
        )
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "serve"],
            input=request,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        responses = result.stdout.decode("utf-8").splitlines()
        self.assertEqual(len(responses), 2, responses)
        first = json.loads(responses[0])
        self.assertFalse(first["ok"])
        self.assertIn("too long", first["error"])
        second = json.loads(responses[1])
        self.assertTrue(second["ok"])
        self.assertEqual(second["result"], ["کتاب", "خوب", "است"])

    def test_non_utf8_line_is_answered_and_the_server_keeps_going(self):
        request = b"\xff\xfe\n" + b'{"command":"version"}\n'
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "serve"],
            input=request,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        responses = result.stdout.decode("utf-8").splitlines()
        self.assertEqual(len(responses), 2, responses)
        first = json.loads(responses[0])
        self.assertFalse(first["ok"])
        self.assertIn("UTF-8", first["error"])
        second = json.loads(responses[1])
        self.assertTrue(second["ok"])
        self.assertEqual(second["result"]["protocol"], 1)

    def test_lone_surrogate_line_is_answered_and_the_server_keeps_going(self):
        # A client can send a lone surrogate escape; echoing it back used to raise
        # UnicodeEncodeError outside the handler and kill the loop with no reply.
        request = b'{"command":"\\ud800"}\n{"command":"version"}\n'
        result = subprocess.run(
            [sys.executable, "-m", "rishe.cli", "serve"],
            input=request,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        responses = result.stdout.decode("utf-8").splitlines()
        self.assertEqual(len(responses), 2, responses)
        self.assertFalse(json.loads(responses[0])["ok"])
        self.assertTrue(json.loads(responses[1])["ok"])

    def test_errors_and_blank_lines_keep_the_stream_in_sync(self):
        result = self.serve(
            '{"command":"nope"}',
            "",
            '{"command":"version"}',
            "not json",
            '{"command":"tokenize","text":"سلام"}',
        )
        self.assertEqual(
            result.returncode, 0, result.stderr.decode("utf-8", "replace")
        )
        responses = [
            json.loads(line) for line in result.stdout.decode("utf-8").splitlines()
        ]
        self.assertEqual(len(responses), 4)
        self.assertFalse(responses[0]["ok"])
        self.assertEqual(responses[1]["result"]["protocol"], 1)
        self.assertFalse(responses[2]["ok"])
        self.assertEqual(responses[3], {"ok": True, "result": ["سلام"]})


if __name__ == "__main__":
    unittest.main()
