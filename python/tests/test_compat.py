import unittest

from rishe import (
    Lemmatizer,
    Normalizer,
    SentenceTokenizer,
    Stemmer,
    TokenSplitter,
    WordTokenizer,
)
from rishe.compat import sent_tokenize, word_tokenize


class CompatTest(unittest.TestCase):
    def test_normalizer(self):
        self.assertEqual(Normalizer().normalize("مي كنم"), "می‌کنم")
        self.assertEqual(Normalizer(correct_spacing=True).normalize("سلام   دنیا"), "سلام دنیا")
        self.assertEqual(
            Normalizer(remove_diacritics=True).remove_diacritics("کُجا نِشانِ"),
            "کجا نشان",
        )

    def test_normalizer_persian_numbers_can_be_off(self):
        self.assertEqual(Normalizer().normalize("123"), "۱۲۳")
        self.assertEqual(Normalizer(persian_numbers=False).normalize("123"), "123")

    def test_normalizer_rejects_switches_it_cannot_honor(self):
        with self.assertRaises(NotImplementedError):
            Normalizer(unicodes_replacement=False)
        with self.assertRaises(NotImplementedError):
            Normalizer(seperate_mi=False)
        self.assertEqual(
            Normalizer(seperate_mi=False, correct_spacing=False).normalize("می روم"),
            "می روم",
        )

    def test_word_tokenizer_joins_verb_parts(self):
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

    def test_lemmatizer_returns_both_stems(self):
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

    def test_correct_spacing_can_be_turned_off(self):
        """The switch must actually change the output, not be silently ignored."""
        off = Normalizer(correct_spacing=False)
        on = Normalizer(correct_spacing=True)
        for text in ("سلام   دنیا", "کتاب ها"):
            self.assertEqual(on.normalize(text), on.normalize(text))
            self.assertNotEqual(
                on.normalize(text),
                off.normalize(text),
                f"correct_spacing=False changed nothing for {text!r}",
            )
        self.assertEqual(off.normalize("سلام   دنیا"), "سلام   دنیا")


if __name__ == "__main__":
    unittest.main()
