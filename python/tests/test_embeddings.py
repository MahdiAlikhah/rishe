import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from rishe import Embeddings, hub

TINY = """6 3
کتاب 1 0 0
خانه 0 1 0
مدرسه 0.6 0.8 0
سلام 0 0 1
دنیا 0 0 0.5
مردم 1 1 0
"""


class EmbeddingsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory()
        cls.path = Path(cls.directory.name) / "tiny.vec"
        cls.path.write_text(TINY, encoding="utf-8")
        cls.embeddings = Embeddings(str(cls.path))

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def test_shape_and_lookup(self):
        self.assertEqual(len(self.embeddings), 6)
        self.assertEqual(self.embeddings.dim, 3)
        self.assertEqual(self.embeddings.vector("کتاب"), [1.0, 0.0, 0.0])
        self.assertIsNone(self.embeddings.vector("ناموجود"))
        self.assertTrue(self.embeddings.contains("خانه"))
        self.assertFalse(self.embeddings.contains("ناموجود"))

    def test_similarity(self):
        self.assertAlmostEqual(self.embeddings.similarity("کتاب", "خانه"), 0.0, places=6)
        self.assertAlmostEqual(
            self.embeddings.similarity("کتاب", "مردم"), 0.70710677, places=5
        )
        self.assertIsNone(self.embeddings.similarity("کتاب", "ناموجود"))

    def test_sentence_vectors(self):
        self.assertEqual(self.embeddings.sentence_vector("کتاب خانه"), [0.5, 0.5, 0.0])
        self.assertEqual(self.embeddings.sentence_vector("کتاب ناموجود"), [1.0, 0.0, 0.0])
        self.assertEqual(self.embeddings.sentence_vector("ناموجود"), [0.0, 0.0, 0.0])
        self.assertAlmostEqual(
            self.embeddings.sentence_similarity("کتاب خانه", "خانه"), 0.70710677, places=5
        )
        batch = self.embeddings.sentence_vectors(["کتاب", "خانه"])
        self.assertEqual(batch, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])

    def test_most_similar(self):
        nearest = self.embeddings.most_similar("کتاب", 2)
        self.assertEqual([word for word, _ in nearest], ["مردم", "مدرسه"])
        self.assertAlmostEqual(nearest[0][1], 0.70710677, places=5)
        self.assertEqual(self.embeddings.most_similar("ناموجود", 2), [])

    def test_rejects_malformed_file(self):
        with tempfile.NamedTemporaryFile("w", suffix=".vec", delete=False) as file:
            file.write("not a header\n")
            path = file.name
        try:
            with self.assertRaises(OSError):
                Embeddings(path)
        finally:
            os.unlink(path)


class HubTest(unittest.TestCase):
    def test_cache_starts_empty(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.dict(
            os.environ, {"RISHE_HOME": directory}
        ):
            self.assertIsNone(hub.cached("hazm-fasttext-300"))
            with self.assertRaises(FileNotFoundError):
                hub.path("hazm-fasttext-300")
            with self.assertRaises(ValueError):
                hub.cached("unknown-model")

    def test_custom_url_requires_checksum(self):
        with self.assertRaises(ValueError):
            hub.pull("https://example.com/model.vec")


if __name__ == "__main__":
    unittest.main()
