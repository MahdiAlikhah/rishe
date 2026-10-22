import unittest

from rishe import (
    Lemmatizer,
    Normalizer,
    SentenceTokenizer,
    Stemmer,
    TokenSplitter,
    WordTokenizer,
    sent_tokenize,
    word_tokenize,
)


class HazmCompatTest(unittest.TestCase):
    def test_normalizer(self):
        self.assertEqual(Normalizer().normalize("مي كنم"), "می‌کنم")
        self.assertEqual(Normalizer(correct_spacing=True).normalize("سلام   دنیا"), "سلام دنیا")
        self.assertEqual(
            Normalizer(remove_diacritics=True).remove_diacritics("کُجا نِشانِ"),
            "کجا نشان",
        )

    def test_word_tokenizer_matches_hazm_defaults(self):
        self.assertEqual(WordTokenizer().tokenize("رفته است"), ["رفته_است"])
        self.assertEqual(
            WordTokenizer(join_verb_parts=False).tokenize("رفته است"),
            ["رفته", "است"],
        )
        self.assertEqual(word_tokenize("رفته است"), ["رفته_است"])
        self.assertEqual(
            WordTokenizer(separate_emoji=True).tokenize("سلام😊دنیا"),
            ["سلام", "😊", "دنیا"],
        )

    def test_lemmatizer_returns_hazm_style_output(self):
        self.assertEqual(Lemmatizer().lemmatize("می‌روم"), "رفت#رو")
        self.assertEqual(Lemmatizer().lemmatize("کتاب"), "کتاب")

    def test_stemmer_and_splitter(self):
        self.assertEqual(Stemmer().stem("کتاب‌ها"), "کتاب")
        self.assertEqual(TokenSplitter().split_token_words("کتاب‌ها"), [("کتاب", "ها")])
        self.assertEqual(TokenSplitter().split_token_words("کتاب"), [])

    def test_sentence_tokenizer(self):
        expected = ["یک.", "دو."]
        self.assertEqual(SentenceTokenizer().tokenize("یک. دو."), expected)
        self.assertEqual(sent_tokenize("یک. دو."), expected)

    def test_custom_lexicon_files_are_rejected(self):
        with self.assertRaises(NotImplementedError):
            WordTokenizer(words_file="words.dat")
        with self.assertRaises(NotImplementedError):
            Lemmatizer(verbs_file="verbs.dat")


if __name__ == "__main__":
    unittest.main()
