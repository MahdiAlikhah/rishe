import unittest

import rishe
from rishe import hub


class FrontDoorTest(unittest.TestCase):
    def test_pipeline_is_shared(self):
        self.assertIs(rishe.pipeline(), rishe.pipeline())
        self.assertIsInstance(rishe.pipeline(), rishe.Pipeline)

    def test_normalize(self):
        self.assertEqual(rishe.normalize("مي كنم"), "می‌کنم")
        self.assertEqual(rishe.normalize("مي كنم", stages=["digits"]), "مي كنم")

    def test_tokenize_does_not_normalize(self):
        # Tokenizing is a primitive step; normalization is its own call.
        self.assertEqual(rishe.tokenize("سلام دنیا"), ["سلام", "دنیا"])
        self.assertEqual(rishe.tokenize("مي كنم"), ["مي", "كنم"])
        self.assertEqual(rishe.tokenize(rishe.normalize("مي كنم")), ["می‌کنم"])
        self.assertEqual(rishe.process("مي كنم").tokens, ["می‌کنم"])

    def test_tokenize_options(self):
        self.assertEqual(
            rishe.tokenize("کتاب", replace_links=True), rishe.tokenize("کتاب")
        )
        with self.assertRaises(TypeError):
            rishe.tokenize("کتاب", colour=True)

    def test_tokenize_spans(self):
        spans = rishe.tokenize_spans("سلام دنیا")
        self.assertEqual([token for token, _, _ in spans], ["سلام", "دنیا"])
        self.assertEqual(spans[0][1:], (0, 4))

    def test_sent_tokenize(self):
        self.assertEqual(
            rishe.sent_tokenize("سلام. دنیا خوب است."),
            ["سلام.", "دنیا خوب است."],
        )

    def test_process_and_batch(self):
        doc = rishe.process("کتاب خوب است")
        self.assertTrue(doc.tokens)
        self.assertEqual(len(rishe.process_batch(["سلام", "دنیا"])), 2)

    def test_process_rejects_an_unknown_stage(self):
        # A typo used to be ignored, which silently changed the output.
        with self.assertRaises(ValueError) as caught:
            rishe.process("کتاب خوب است", stages=["toknize"])
        self.assertIn("unknown process stage: toknize", str(caught.exception))

    def test_process_stages_subset_and_empty_list(self):
        self.assertEqual(
            rishe.process("مي كنم", stages=[]).formal_text,
            rishe.process("مي كنم").formal_text,
        )
        self.assertIsInstance(rishe.process("مي كنم", stages=["normalize"]).tokens, list)

    def test_lemmatize_and_stem(self):
        self.assertEqual(rishe.lemmatize("می‌روم"), "رفت")
        self.assertTrue(rishe.stem("کتاب‌ها"))

    def test_mask(self):
        self.assertEqual(
            rishe.mask("به https://a.b برو", maskers=["url"]), "به <URL> برو"
        )
        masked = rishe.mask("سال ۱۴۰۲", maskers=["digits"], mask_token="N")
        self.assertIn("N", masked)
        self.assertNotIn("۱۴۰۲", masked)
        self.assertEqual(len(rishe.DEFAULT_MASKERS), 10)

    def test_keywords(self):
        self.assertLessEqual(len(rishe.keywords("کتاب خوب و مفید است", n=2)), 2)
        self.assertLessEqual(
            len(rishe.keywords("کتاب خوب و مفید است", n=2, method="textrank")), 2
        )
        with self.assertRaises(ValueError):
            rishe.keywords("کتاب", method="magic")

    def test_deprecated_word_tokenize(self):
        with self.assertWarns(DeprecationWarning):
            self.assertEqual(rishe.word_tokenize("سلام دنیا"), ["سلام", "دنیا"])
        # 0.10 behaviour: normalize first and join verb parts, like the compat class.
        with self.assertWarns(DeprecationWarning):
            self.assertEqual(rishe.word_tokenize("رفته است"), ["رفته_است"])


class DeprecatedSurfaceTest(unittest.TestCase):
    """Aliases added in 0.11.0 must warn and keep working until 1.0.0."""

    def test_pipeline_aliases_warn(self):
        pipe = rishe.pipeline()
        with self.assertWarns(DeprecationWarning):
            self.assertEqual(pipe.word_tokenize("مي كنم"), ["می‌کنم"])
        with self.assertWarns(DeprecationWarning):
            self.assertEqual(pipe.mask_stages("به https://a.b برو", ["url"]), "به <URL> برو")
        with self.assertWarns(DeprecationWarning):
            self.assertEqual(pipe.normalize_stages("12", ["digits"]), "۱۲")
        with self.assertWarns(DeprecationWarning):
            self.assertIsNotNone(pipe.process_stages("کتاب", ["normalize"]))
        with self.assertWarns(DeprecationWarning):
            self.assertEqual(len(pipe.pipe(["سلام"])), 1)
        with self.assertWarns(DeprecationWarning):
            lemma, present = pipe.lemma_full("می‌روم")
            self.assertEqual(lemma, "رفت")
            self.assertIsNotNone(present or "x")

    def test_canonical_names_do_not_warn(self):
        import warnings

        pipe = rishe.pipeline()
        with warnings.catch_warnings():
            warnings.simplefilter("error", DeprecationWarning)
            self.assertEqual(pipe.tokenize("مي كنم"), ["مي", "كنم"])
            self.assertEqual(pipe.process("مي كنم").tokens, ["می‌کنم"])
            self.assertEqual(pipe.mask("به https://a.b برو", ["url"]), "به <URL> برو")
            self.assertEqual(pipe.normalize("12", ["digits"]), "۱۲")
            self.assertIsNotNone(pipe.process("کتاب"))
            self.assertEqual(len(pipe.process_batch(["سلام"])), 1)
            self.assertEqual(pipe.lemmatize_verb("می‌روم")[0], "رفت")


class HubUnificationTest(unittest.TestCase):
    def test_pack_and_model_share_the_same_verb(self):
        self.assertTrue(hub.is_pack("pos-compact"))
        self.assertFalse(hub.is_pack("rishe-embed-fa-300-v2"))
        self.assertEqual(hub.PACKS["depparse-compact"], "depparse")

    def test_deprecated_pack_aliases_warn(self):
        with self.assertWarns(DeprecationWarning):
            with self.assertRaises(ValueError):
                hub.cached_pack("unknown")
        with self.assertWarns(DeprecationWarning):
            with self.assertRaises(ValueError):
                hub.path_pack("unknown")

    def test_unknown_name_lists_models_and_packs(self):
        with self.assertRaises(ValueError) as caught:
            hub.cached("nope")
        message = str(caught.exception)
        self.assertIn("models:", message)
        self.assertIn("packs:", message)


if __name__ == "__main__":
    unittest.main()
