import tempfile
import unittest
from pathlib import Path

from rishe.corpora import ConlluReader, PeymaReader, PlainTextReader


CONLLU = """\
# sent_id = 1
# text = کتاب‌ها را دیدم.
1-2\tکتاب‌ها\t_\t_\t_\t_\t_\t_\t_\t_
1\tکتاب\tکتاب\tNOUN\tN\tNumber=Plur\t3\tnsubj\t_\t_
2\tها\t_\tNOUN\tN\t_\t_\t_\t_\t_
3\tرا\tرا\tADP\tP\t_\t_\t_\t_\t_
4\tدیدم\tدید\tVERB\tV\tTense=Past\t0\troot\t_\tSpaceAfter=No
5\t.\t.\tPUNCT\tP\t_\t_\t_\t_\t_
"""


class CorpusReadersTest(unittest.TestCase):
    def test_conllu(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.conllu"
            path.write_text(CONLLU, encoding="utf-8")
            sentences = list(ConlluReader(path))

        self.assertEqual(len(sentences), 1)
        sentence = sentences[0]
        self.assertEqual(sentence.id, "1")
        self.assertEqual(sentence.text, "کتاب‌ها را دیدم.")
        self.assertEqual(len(sentence.tokens), 6)
        self.assertEqual(sentence.tokens[0].id, "1-2")
        self.assertEqual(sentence.tokens[1].lemma, "کتاب")
        self.assertEqual(sentence.tokens[1].upos, "NOUN")
        self.assertEqual(sentence.tokens[1].feats, {"Number": "Plur"})
        self.assertEqual(sentence.tokens[4].deprel, "root")
        self.assertEqual(sentence.tokens[4].misc, {"SpaceAfter": "No"})
        self.assertIsNone(sentence.tokens[2].lemma)

    def test_peyma(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.txt"
            path.write_text("سلام|S|O\nدنیا|S|O\n\nخداحافظ\n", encoding="utf-8")
            sentences = list(PeymaReader(path))

        self.assertEqual([s.text for s in sentences], ["سلام دنیا", "خداحافظ"])
        self.assertTrue(all(not s.tokens for s in sentences))

    def test_plain_text(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.txt"
            path.write_text("یک\n\nدو\nسه\n", encoding="utf-8")
            sentences = list(PlainTextReader(path))

        self.assertEqual([s.text for s in sentences], ["یک", "دو", "سه"])


if __name__ == "__main__":
    unittest.main()
