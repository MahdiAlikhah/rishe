import unittest

from rishe import Pipeline


class CoreRegressionTest(unittest.TestCase):
    def setUp(self):
        self.pipe = Pipeline()

    def test_arabic_letters_and_digits(self):
        self.assertEqual(self.pipe.normalize("مي‌كنم 2"), "می‌کنم ۲")

    def test_zwnj_stays_one_token(self):
        self.assertEqual(self.pipe.word_tokenize("کتاب‌ها"), ["کتاب‌ها"])
        self.assertEqual(self.pipe.word_tokenize("می‌روم"), ["می‌روم"])

    def test_sentence_punctuation(self):
        self.assertEqual(self.pipe.sent_tokenize("یک. دو! سه؟"), ["یک.", "دو!", "سه؟"])

    def test_decimal_and_abbreviation_do_not_split(self):
        self.assertEqual(self.pipe.sent_tokenize("عدد ۳.۱۴ است."), ["عدد ۳.۱۴ است."])
        self.assertEqual(self.pipe.sent_tokenize("د. احمدی آمد."), ["د. احمدی آمد."])


if __name__ == "__main__":
    unittest.main()
