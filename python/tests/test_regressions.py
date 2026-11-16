import unittest

from rishe import Pipeline, parse_date


class CoreRegressionTest(unittest.TestCase):
    def setUp(self):
        self.pipe = Pipeline()

    def test_arabic_letters_and_digits(self):
        self.assertEqual(self.pipe.normalize("مي‌كنم 2"), "می‌کنم ۲")

    def test_zwnj_stays_one_token(self):
        self.assertEqual(self.pipe.tokenize("کتاب‌ها"), ["کتاب‌ها"])
        self.assertEqual(self.pipe.tokenize("می‌روم"), ["می‌روم"])

    def test_sentence_punctuation(self):
        self.assertEqual(self.pipe.sent_tokenize("یک. دو! سه؟"), ["یک.", "دو!", "سه؟"])

    def test_decimal_and_abbreviation_do_not_split(self):
        self.assertEqual(self.pipe.sent_tokenize("عدد ۳.۱۴ است."), ["عدد ۳.۱۴ است."])
        self.assertEqual(self.pipe.sent_tokenize("د. احمدی آمد."), ["د. احمدی آمد."])

    def test_html_masker_keeps_escaped_markup(self):
        self.assertEqual(self.pipe.mask("&lt;b&gt;سلام&lt;/b&gt;", ["html"]), "<b>سلام</b>")
        self.assertEqual(self.pipe.mask("a &amp; b", ["html"]), "a & b")
        self.assertEqual(self.pipe.mask("<p>متن</p>", ["html"]), "متن")

    def test_html_masker_does_not_decode_control_characters(self):
        for ref in ("&#0;", "&#1;", "&#x1f;", "&#127;", "&#xFFFE;", "&#xFFFF;"):
            self.assertEqual(self.pipe.mask(ref, ["html"]), ref)
        self.assertEqual(self.pipe.mask("a&#9;b", ["html"]), "a\tb")

    def test_gregorian_iso_is_not_a_jalali_date(self):
        self.assertIsNone(parse_date("2023-08-03"))
        date = parse_date("۱۴۰۲/۰۵/۱۲")
        self.assertIsNone(parse_date(date.iso))
        self.assertEqual(parse_date("۱۲ مرداد ۲۰۰۰").year, 2000)


if __name__ == "__main__":
    unittest.main()
