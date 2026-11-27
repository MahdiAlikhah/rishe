import threading
import time
import unittest

from rishe import JalaliDate, Pipeline, parse_date


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

    def test_jalali_date_compares_by_value(self):
        """Two equal dates were different objects, so `==` said False."""
        first = JalaliDate(1402, 5, 12)
        second = JalaliDate(1402, 5, 12)
        self.assertEqual(first, second)
        self.assertEqual(hash(first), hash(second))
        self.assertNotEqual(first, JalaliDate(1402, 5, 13))
        self.assertLess(first, JalaliDate(1402, 5, 13))
        self.assertGreaterEqual(second, first)
        self.assertEqual({first: "x"}[second], "x")

    def test_word_final_ha_keeps_its_consonant(self):
        """`ح` was grouped with `ه`, so `صبح` became `сба` with a vowel."""
        self.assertEqual(self.pipe.to_tajik("صبح"), "сбҳ")
        self.assertEqual(self.pipe.to_tajik("خانه"), "хона")

    def test_decimal_leading_zeros_are_spoken(self):
        """`3.05` used to be read as `3.5`, dropping the zero in the fraction."""
        self.assertEqual(self.pipe.number_to_words("3.05"), "سه ممیز صفر پنج")
        self.assertEqual(self.pipe.number_to_words("3.5"), "سه ممیز پنج")
        self.assertEqual(self.pipe.number_to_words("3.00"), "سه ممیز صفر صفر")

    def test_future_auxiliary_joins_only_a_verb(self):
        """A `خواهم`-form joined whatever token followed it, punctuation included."""
        self.assertEqual(self.pipe.tokenize("خواهم رفت", join_verb_parts=True), ["خواهم_رفت"])

    def test_spell_correct_survives_a_lone_carriage_return(self):
        """A mid-string `\\r` recursed until the stack overflowed.

        The per-line split guarded on `\\r` but split only on `\\n`, so one line
        came back, the `\\r` survived, and the recursive call got the same input
        forever. Correcting it crashes the process rather than raising, so this
        test only has to run: crash means the bug is back.
        """
        self.assertEqual(self.pipe.spell_correct("a\rb"), "a b")
        self.assertEqual(self.pipe.spell_correct("\rآ"), "آ")
        self.assertEqual(self.pipe.spell_correct("آ\rب"), "آ ب")
        # A real CRLF line ending still round-trips.
        self.assertEqual(self.pipe.spell_correct("سلام\r\nدنیا"), "سلام\r\nدنیا")
        self.assertEqual(
            self.pipe.tokenize("او خواهد .", join_verb_parts=True), ["او", "خواهد", "."]
        )
        self.assertEqual(
            self.pipe.tokenize("خواهم که بروم", join_verb_parts=True), ["خواهم", "که", "بروم"]
        )

    def test_emoji_and_sigil_edges(self):
        """Star, watch and alarm were missed while circled digits were deleted;
        a lone `#`/`@` was masked; a `#` inside a word became a tag."""
        for ch in ["\u2b50", "\u231a", "\u23f0"]:
            self.assertEqual(self.pipe.mask("x" + ch + "y", ["emoji"]), "xy", hex(ord(ch)))
        for ch in ["\u2776", "\u2777"]:
            self.assertEqual(self.pipe.mask("x" + ch + "y", ["emoji"]), "x" + ch + "y", hex(ord(ch)))
        for text in ["# ", "@ ", "خالص # و @"]:
            self.assertEqual(self.pipe.mask(text, ["mention"]), text)
            self.assertEqual(self.pipe.mask(text, ["hashtag"]), text)
        self.assertEqual(self.pipe.mask("#tag خبر", ["hashtag"]), "خبر")
        self.assertEqual(self.pipe.mask("@user سلام", ["mention"]), "سلام")
        family = "\U0001f468\u200d\U0001f469\u200d\U0001f467"
        self.assertEqual(self.pipe.mask("x" + family + "y", ["emoji"]), "xy")

    def test_url_masker_keeps_the_sentence_punctuation(self):
        """The URL scan ran to the next space, so `(https://a.com)` lost its `)`,
        and `www.`/`ftp://` were not URLs at all."""
        self.assertEqual(self.pipe.mask("(https://a.com) بعد", ["url"]), "(<URL>) بعد")
        self.assertEqual(self.pipe.mask("«https://a.com»", ["url"]), "«<URL>»")
        self.assertEqual(self.pipe.mask("به https://a.com، برو", ["url"]), "به <URL>، برو")
        self.assertEqual(self.pipe.mask("پایان https://a.com.", ["url"]), "پایان <URL>.")
        self.assertEqual(self.pipe.mask("www.example.com", ["url"]), "<URL>")
        self.assertEqual(self.pipe.mask("ftp://files.example.com", ["url"]), "<URL>")
        self.assertEqual(self.pipe.mask("abchttps://a.com", ["url"]), "abchttps://a.com")

    def test_ya_standard_only_rewrites_at_a_word_end(self):
        """`ه‌ی` is the word-final ending only at a word end; `خانه‌یی` used to
        become `خانۀی`, which is neither spelling."""
        zwnj = "\u200c"
        self.assertEqual(self.pipe.normalize("خانه" + zwnj + "یی", ["ya_standard"]), "خانه" + zwnj + "یی")
        self.assertEqual(self.pipe.normalize("خانه" + zwnj + "ی", ["ya_standard"]), "خانۀ")
        self.assertEqual(self.pipe.normalize("خانه" + zwnj + "ی ما", ["ya_standard"]), "خانۀ ما")
        self.assertEqual(self.pipe.normalize("خانۀ ما", ["ya_joda"]), "خانه" + zwnj + "ی ما")

    def test_tables_leave_letters_and_marks_alone(self):
        """A skin tone modifier, a pharyngeal and a glottal stop were rewritten
        to ؟; dotless i, Armenian sheh and Georgian vini were rewritten to digits."""
        for ch in ["\u0295", "\u0294", "\U0001f3fb", "\x08", "\x97", "\x9d"]:
            self.assertEqual(self.pipe.normalize(ch, ["punctuation"]), ch, hex(ord(ch)))
        for ch in ["\u0131", "\u0577", "\u10d5"]:
            self.assertEqual(self.pipe.normalize(ch, ["digits"]), ch, hex(ord(ch)))
        self.assertEqual(self.pipe.normalize("👍🏻؟", ["punctuation"]), "👍🏻؟")
        self.assertEqual(self.pipe.normalize("1١①", ["digits"]), "۱۱۱")

    def test_presentation_forms_reach_the_lexicon(self):
        """Persian from a PDF arrives in Arabic presentation forms; the default
        normalizer used to keep them, so no lexicon or lemma lookup matched."""
        book = "\ufed9\ufe95\ufe8d\ufe8f \ufea5\ufeed\ufe8f"
        self.assertEqual(self.pipe.normalize(book), "کتاب خوب")
        self.assertEqual(self.pipe.process(book).tokens, ["کتاب", "خوب"])
        self.assertEqual(self.pipe.normalize("ة ہ"), "ه ه")

    def test_zwnj_affix_fix_survives_a_rejected_prefix(self):
        """`قدیمی` ends in the `می` of the prefix rule, so `می ` matched and was
        rejected as a prefix — but it had eaten the space ` ها` needs."""
        self.assertEqual(self.pipe.normalize("قدیمی ها"), "قدیمی\u200cها")
        self.assertEqual(self.pipe.normalize("علمی تر"), "علمی\u200cتر")
        self.assertEqual(self.pipe.normalize("سمی ها"), "سمی\u200cها")
        self.assertEqual(self.pipe.normalize("کتاب ها"), "کتاب\u200cها")
        self.assertEqual(self.pipe.normalize("کتاب های من"), "کتاب\u200cهای من")

    def test_html_ignores_prose_between_angle_brackets(self):
        """Any `>` used to close a tag, so Persian prose between `<` and `>` was
        deleted: this came back as "اگر ad باشد"."""
        ref = "اگر a<b و c>d باشد"
        self.assertEqual(self.pipe.mask(ref, ["html"]), ref)
        self.assertEqual(self.pipe.mask("<p>متن</p>", ["html"]), "متن")
        self.assertEqual(
            self.pipe.mask('<a title="سلام">متن</a>', ["html"]), "متن"
        )
        self.assertEqual(self.pipe.mask("<!-- یادداشت -->متن", ["html"]), "متن")

    def test_process_keeps_newline_sentence_boundaries(self):
        """Sentences were split after detokenizing, which had already turned
        newlines into spaces, so two lines came back as one sentence."""
        doc = self.pipe.process("سطر اول\nسطر دوم")
        self.assertEqual(doc.sentences, ["سطر اول", "سطر دوم"])
        # It agrees with splitting the text directly.
        self.assertEqual(doc.sentences, self.pipe.sent_tokenize("سطر اول\nسطر دوم"))
        self.assertEqual(self.pipe.process("خط یک\n\nخط سه").sentences, ["خط یک", "خط سه"])

    def test_ngrams_refuses_a_wide_range(self):
        """`max_n` tracking the token count made the output cubic in the input.

        A 2000-word text at max_n=2000 built 6.7 GB of strings over 153s. The
        cap is checked before any work, so a short text proves it and fails fast
        if the check is ever removed.
        """
        with self.assertRaises(ValueError) as caught:
            self.pipe.ngrams("کتاب خوب است", 1, 2000)
        self.assertIn("max_n must be at most", str(caught.exception))
        # The documented range still works.
        self.assertEqual(
            self.pipe.ngrams("این یک متن", 1, 2),
            ["این", "یک", "متن", "این یک", "یک متن"],
        )

    def test_overlapping_emails_do_not_panic(self):
        self.assertEqual(self.pipe.mask("a@b.cc@d.ee", ["email"]), "<EMAIL>@d.ee")
        self.assertEqual(self.pipe.mask("x@y.com@z.org", ["email"]), "<EMAIL>@z.org")

    def test_non_persian_keeps_separators(self):
        """Deleting a newline joined two lines and deleting a comma joined two words."""
        self.assertEqual(
            self.pipe.mask("سلام\nدنیا,خوب", ["non_persian"]), "سلام\nدنیا,خوب"
        )
        self.assertEqual(
            self.pipe.mask("سلام hello دنیا", ["non_persian"]), "سلام  دنیا"
        )
        self.assertEqual(
            self.pipe.mask("قیمت 123 تومان", ["non_persian"]), "قیمت 123 تومان"
        )

    def test_a_long_suffixed_token_finishes(self):
        doc = self.pipe.process("کتاب" + "مون" * 20000)
        self.assertTrue(doc.tokens)

    def test_long_calls_release_the_gil(self):
        """A background Python thread must keep running during a long Rust call.

        Every one of these held the GIL for its whole duration, so a server
        calling `normalize` blocked every other request in the process. A tick
        count is too weak (the thread runs while the arguments are marshalled),
        so this measures the longest gap between ticks instead.
        """
        text = "سلام دنیا این یک متن آزمایشی است " * 3200  # ~100 KB
        calls = {
            "normalize": lambda: self.pipe.normalize(text),
            "mask": lambda: self.pipe.mask(text, ["html", "url", "email"]),
            "tokenize": lambda: self.pipe.tokenize(text),
            "tokenize_spans": lambda: self.pipe.tokenize_spans(text),
            "sent_tokenize": lambda: self.pipe.sent_tokenize(text),
            "spell_correct": lambda: self.pipe.spell_correct(text),
            "rake": lambda: self.pipe.rake(text, 5, 3),
            "textrank": lambda: self.pipe.textrank(text, 5, 3, 2),
        }
        for name, call in calls.items():
            with self.subTest(method=name):
                stop = threading.Event()
                last = [time.perf_counter()]
                stall = [0.0]

                def spin():
                    while not stop.is_set():
                        now = time.perf_counter()
                        gap = now - last[0]
                        if gap > stall[0]:
                            stall[0] = gap
                        last[0] = now

                worker = threading.Thread(target=spin, daemon=True)
                worker.start()
                time.sleep(0.05)  # let it establish a rhythm
                stall[0] = 0.0
                call()
                stop.set()
                worker.join(timeout=1.0)
                self.assertLess(
                    stall[0],
                    0.025,
                    f"{name} blocked other threads for {stall[0] * 1000:.0f}ms",
                )

    def test_the_transliteration_and_noise_calls_release_the_gil(self):
        """The seven methods above the fold kept holding the GIL.

        `test_long_calls_release_the_gil` only covers the eight text methods that
        already took `py: Python<'_>`. These seven did not, so a server calling
        `to_tajik` blocked every other request — but each call is only a few
        milliseconds on the input that test uses, well under its 25 ms stall
        threshold, so simply adding them there would assert nothing.

        This counts how many times a spinner thread ticks *during one call* and
        compares that to the rate the same spinner reaches while the main thread
        sleeps (the GIL is free then). A call that never releases the GIL blocks
        the spinner for its whole duration and scores near 0%; a released one
        scores tens of percent. Measured on this change: 0% held, 62% released.

        Two earlier designs of this test were wrong, both worth recording. Growing
        the input until one call was long made it flaky: at tens of megabytes the
        "stall" is dominated by pyo3 converting the Python `str` into a Rust `str`
        before `allow_threads` is entered, plus GC pauses over a huge heap, which
        made `persianize_suggest` look like it blocked for 169 ms of a 305 ms call
        and failed CI on all five Python versions. Repeating a small call and
        watching the longest gap was worse: a held GIL only stalls the spinner for
        one call, then the loop boundary frees it again, so that version passed
        even with the fix reverted. Ticks over one call have neither hole.
        """
        text = "سلام دنیا این یک متن آزمایشی است " * 25600  # ~1.7 MB
        calls = {
            "keyboard_noise": self.pipe.keyboard_noise,
            "ocr_noise": self.pipe.ocr_noise,
            "whitespace_noise": self.pipe.whitespace_noise,
            "to_tajik": self.pipe.to_tajik,
            "to_persian": self.pipe.to_persian,
            "persianize": self.pipe.persianize,
            "persianize_suggest": self.pipe.persianize_suggest,
        }
        for name, call in calls.items():
            with self.subTest(method=name):
                share = self._tick_share(lambda: call(text))
                # 0% when held, 40-70% when released. 5% is far from both.
                self.assertGreater(
                    share,
                    0.05,
                    f"{name} let another thread run for only {share:.1%} of the "
                    "call, so it holds the GIL",
                )

    @staticmethod
    def _tick_share(call):
        """Fraction of the ticks a free-running thread gets during `call`.

        1.0 means the GIL was released for the whole call; 0.0 means it was never
        released. The denominator is measured with the main thread asleep, which
        by definition leaves the GIL free.
        """
        counter = [0]
        stop = threading.Event()

        def spin():
            while not stop.is_set():
                counter[0] += 1

        worker = threading.Thread(target=spin, daemon=True)
        worker.start()
        time.sleep(0.02)  # let the spinner reach its full rate

        before = counter[0]
        time.sleep(0.05)
        idle_rate = (counter[0] - before) / 0.05

        before = counter[0]
        started = time.perf_counter()
        call()
        duration = time.perf_counter() - started
        ticks = counter[0] - before

        stop.set()
        worker.join(timeout=1.0)
        return ticks / max(idle_rate * duration, 1.0)


if __name__ == "__main__":
    unittest.main()
