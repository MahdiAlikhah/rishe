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

    def test_empty_spell_suggest_and_big_int(self):
        pipeline = rishe.pipeline()
        self.assertEqual(pipeline.spell_suggest(""), [])
        self.assertEqual(pipeline.number_to_words(1000), "یک هزار")
        with self.assertRaises(OverflowError):
            pipeline.number_to_words(10**25)

    def test_possessive_out_of_range_is_none(self):
        pipeline = rishe.pipeline()
        self.assertEqual(pipeline.possessive("کتاب", 0), "کتابم")
        self.assertEqual(pipeline.possessive("دنیا", 0), "دنیایم")
        self.assertIsNone(pipeline.possessive("کتاب", -1))
        self.assertIsNone(pipeline.possessive("کتاب", 6))

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

    def test_process_stage_subset_and_empty_list(self):
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

class RemovedAliasesTest(unittest.TestCase):
    """The 0.11 aliases are gone in 1.0.0: calling one is an AttributeError."""

    def test_removed_pipeline_aliases_are_gone(self):
        pipe = rishe.pipeline()
        for name in (
            "word_tokenize",
            "word_tokenize_with",
            "lemma_full",
            "process_stages",
            "normalize_stages",
            "mask_stages",
            "pipe",
        ):
            self.assertFalse(hasattr(pipe, name), name)

    def test_removed_module_aliases_are_gone(self):
        self.assertFalse(hasattr(rishe, "word_tokenize"))
        for name in ("cached_pack", "path_pack", "pull_pack"):
            self.assertFalse(hasattr(hub, name), name)


class HubUnificationTest(unittest.TestCase):
    def test_pack_and_model_share_the_same_verb(self):
        self.assertTrue(hub.is_pack("pos-compact"))
        self.assertFalse(hub.is_pack("rishe-embed-fa-300-v2"))
        self.assertEqual(hub.PACKS["depparse-compact"], "depparse")

    def test_unknown_name_lists_models_and_packs(self):
        with self.assertRaises(ValueError) as caught:
            hub.cached("nope")
        message = str(caught.exception)
        self.assertIn("models:", message)
        self.assertIn("packs:", message)


if __name__ == "__main__":
    unittest.main()
