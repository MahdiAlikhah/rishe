import io
import json
import unittest
from contextlib import redirect_stdout

from rishe import JalaliDate, parse_date
from rishe.cli import main


class DateTest(unittest.TestCase):
    def test_parse_and_format(self):
        date = parse_date("۱۲ مرداد ۱۴۰۲")
        self.assertIsNotNone(date)
        assert date is not None
        self.assertEqual((date.year, date.month, date.day), (1402, 5, 12))
        self.assertEqual(date.iso, "2023-08-03")
        self.assertEqual(date.to_gregorian(), (2023, 8, 3))
        self.assertEqual(date.format_persian(), "۱۲ مرداد ۱۴۰۲")
        self.assertEqual(date.format_numeric(), "۱۴۰۲/۰۵/۱۲")
        self.assertEqual(repr(date), "JalaliDate(1402, 5, 12)")
        self.assertIsNone(parse_date("not a date"))

    def test_from_gregorian_and_validation(self):
        date = JalaliDate.from_gregorian(2023, 8, 3)
        self.assertIsNotNone(date)
        assert date is not None
        self.assertEqual((date.year, date.month, date.day), (1402, 5, 12))
        with self.assertRaises(ValueError):
            JalaliDate(1402, 12, 30)

    def test_from_gregorian_refuses_days_before_the_window(self):
        self.assertIsNone(JalaliDate.from_gregorian(560, 1, 1))
        self.assertIsNone(JalaliDate.from_gregorian(560, 3, 19))
        date = JalaliDate.from_gregorian(560, 3, 20)
        self.assertIsNotNone(date)
        assert date is not None
        self.assertEqual((date.year, date.month, date.day), (-61, 1, 1))
        self.assertEqual(date.to_gregorian(), (560, 3, 20))

    def test_cli(self):
        for argv in (["date", "۱۲ مرداد ۱۴۰۲"], ["date", "1402/05/12"]):
            output = io.StringIO()
            with redirect_stdout(output):
                code = main(argv)
            self.assertEqual(code, 0)
            self.assertEqual(output.getvalue().strip(), "2023-08-03")

        output = io.StringIO()
        with redirect_stdout(output):
            code = main(["date", "1402/05/12", "--json"])
        self.assertEqual(code, 0)
        self.assertEqual(
            json.loads(output.getvalue()),
            {
                "jalali": {"year": 1402, "month": 5, "day": 12},
                "gregorian": {"year": 2023, "month": 8, "day": 3},
                "iso": "2023-08-03",
            },
        )


if __name__ == "__main__":
    unittest.main()
