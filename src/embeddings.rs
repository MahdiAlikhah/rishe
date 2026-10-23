use crate::tokenizer::Tokenizer;
use fst::{Map, MapBuilder};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Static word embeddings loaded from a fastText/word2vec `.vec` text file.
///
/// The vocabulary is an FST mapping word -> row index; vectors live in one flat
/// row-major buffer. Lookups are O(word length) and the whole model is shared
/// read-only across threads.
#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
pub struct Embeddings {
    dim: usize,
    vectors: Vec<f32>,
    norms: Vec<f32>,
    words: Vec<Box<str>>,
    index: Map<Vec<u8>>,
}

impl Embeddings {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let file =
            File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        let reader = BufReader::with_capacity(1 << 20, file);
        let mut lines = reader.lines();

        let header = lines
            .next()
            .ok_or_else(|| "empty embedding file".to_string())?
            .map_err(|error| format!("read error: {error}"))?;
        let mut header_fields = header.split_ascii_whitespace();
        let declared = header_fields
            .next()
            .and_then(|value| value.parse::<usize>().ok());
        let dim = header_fields
            .next()
            .and_then(|value| value.parse::<usize>().ok());
        let (Some(declared), Some(dim)) = (declared, dim) else {
            return Err(
                "expected a '<vocab> <dim>' header on the first line of the .vec file".to_string(),
            );
        };
        if dim == 0 {
            return Err("embedding dimension must be positive".to_string());
        }

        let mut words: Vec<Box<str>> = Vec::with_capacity(declared);
        let mut vectors: Vec<f32> = Vec::new();
        vectors
            .try_reserve(declared.saturating_mul(dim))
            .map_err(|_| "embedding file is too large for available memory".to_string())?;

        for (line_number, line) in lines.enumerate() {
            let line = line.map_err(|error| format!("read error: {error}"))?;
            let mut fields = line.split_ascii_whitespace();
            let Some(word) = fields.next() else {
                continue;
            };
            let row_start = vectors.len();
            for field in fields {
                let value = field.parse::<f32>().map_err(|_| {
                    format!(
                        "line {}: expected a float, found {field:?}",
                        line_number + 2
                    )
                })?;
                vectors.push(value);
            }
            if vectors.len() - row_start != dim {
                return Err(format!(
                    "line {}: expected {dim} values for {word:?}, found {}",
                    line_number + 2,
                    vectors.len() - row_start
                ));
            }
            words.push(word.into());
        }

        if words.is_empty() {
            return Err("embedding file contains no vectors".to_string());
        }

        let mut order: Vec<u32> = (0..words.len() as u32).collect();
        order.sort_unstable_by(|&a, &b| words[a as usize].cmp(&words[b as usize]));

        let mut builder = MapBuilder::new(Vec::<u8>::with_capacity(words.len() * 16))
            .map_err(|error| format!("fst error: {error}"))?;
        let mut previous: Option<&str> = None;
        for &index in &order {
            let word = &*words[index as usize];
            if previous == Some(word) {
                continue;
            }
            builder
                .insert(word.as_bytes(), index as u64)
                .map_err(|error| format!("fst error: {error}"))?;
            previous = Some(word);
        }
        let index = Map::new(
            builder
                .into_inner()
                .map_err(|error| format!("fst error: {error}"))?,
        )
        .map_err(|error| format!("fst error: {error}"))?;

        let norms = vectors
            .chunks_exact(dim)
            .map(|row| dot(row, row).sqrt())
            .collect();

        Ok(Self {
            dim,
            vectors,
            norms,
            words,
            index,
        })
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    #[inline]
    fn row(&self, index: u64) -> &[f32] {
        let start = index as usize * self.dim;
        &self.vectors[start..start + self.dim]
    }

    pub fn contains(&self, word: &str) -> bool {
        self.index.contains_key(word)
    }

    pub fn vector(&self, word: &str) -> Option<&[f32]> {
        self.index.get(word).map(|index| self.row(index))
    }

    pub fn similarity(&self, first: &str, second: &str) -> Option<f32> {
        let first_index = self.index.get(first)?;
        let second_index = self.index.get(second)?;
        let denominator = self.norms[first_index as usize] * self.norms[second_index as usize];
        if denominator == 0.0 {
            return Some(0.0);
        }
        Some(dot(self.row(first_index), self.row(second_index)) / denominator)
    }

    /// Mean of the vectors of all in-vocabulary tokens. OOV tokens are skipped.
    pub fn sentence_vector(&self, text: &str) -> Vec<f32> {
        let mut sum = vec![0.0f32; self.dim];
        let mut count = 0usize;
        for token in Tokenizer::tokenize(text) {
            let Some(index) = self.index.get(token.text) else {
                continue;
            };
            for (slot, value) in sum.iter_mut().zip(self.row(index)) {
                *slot += value;
            }
            count += 1;
        }
        if count > 1 {
            let divisor = count as f32;
            for slot in &mut sum {
                *slot /= divisor;
            }
        }
        sum
    }

    pub fn sentence_vectors(&self, texts: &[String]) -> Vec<Vec<f32>> {
        texts
            .par_iter()
            .map(|text| self.sentence_vector(text))
            .collect()
    }

    pub fn sentence_similarity(&self, first: &str, second: &str) -> f32 {
        let first = self.sentence_vector(first);
        let second = self.sentence_vector(second);
        let denominator = dot(&first, &first).sqrt() * dot(&second, &second).sqrt();
        if denominator == 0.0 {
            return 0.0;
        }
        dot(&first, &second) / denominator
    }

    pub fn most_similar(&self, word: &str, n: usize) -> Vec<(String, f32)> {
        if n == 0 {
            return Vec::new();
        }
        let Some(query_index) = self.index.get(word) else {
            return Vec::new();
        };
        let query_norm = self.norms[query_index as usize];
        if query_norm == 0.0 {
            return Vec::new();
        }

        // ponytail: linear scan over the vocabulary; fine for interactive use,
        // move to an ANN index if this ever lands on a request hot path.
        let query = self.row(query_index);
        let mut heap: BinaryHeap<std::cmp::Reverse<Candidate<'_>>> =
            BinaryHeap::with_capacity(n + 1);
        for (index, candidate) in self.words.iter().enumerate() {
            if index as u64 == query_index || self.norms[index] == 0.0 {
                continue;
            }
            let score = dot(query, self.row(index as u64)) / (query_norm * self.norms[index]);
            let candidate = Candidate {
                score,
                word: candidate.as_ref(),
            };
            if heap.len() < n {
                heap.push(std::cmp::Reverse(candidate));
            } else if let Some(std::cmp::Reverse(smallest)) = heap.peek() {
                if score > smallest.score {
                    heap.pop();
                    heap.push(std::cmp::Reverse(candidate));
                }
            }
        }

        let mut out: Vec<(String, f32)> = heap
            .into_iter()
            .map(|std::cmp::Reverse(candidate)| (candidate.word.to_owned(), candidate.score))
            .collect();
        out.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        out
    }
}

#[derive(PartialEq)]
struct Candidate<'a> {
    score: f32,
    word: &'a str,
}

impl Eq for Candidate<'_> {}

impl PartialOrd for Candidate<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
    }
}

#[inline]
fn dot(first: &[f32], second: &[f32]) -> f32 {
    first.iter().zip(second).map(|(a, b)| a * b).sum::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY: &str = "6 3\n\
کتاب 1 0 0\n\
خانه 0 1 0\n\
مدرسه 0.6 0.8 0\n\
سلام 0 0 1\n\
دنیا 0 0 0.5\n\
مردم 1 1 0\n";

    #[test]
    fn test_load_lookup_and_math() {
        let path = std::env::temp_dir().join(format!(
            "rishe_embeddings_{}_{}.vec",
            std::process::id(),
            line!()
        ));
        std::fs::write(&path, TINY).unwrap();
        let embeddings = Embeddings::load(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(embeddings.len(), 6);
        assert_eq!(embeddings.dim(), 3);
        assert_eq!(embeddings.vector("کتاب"), Some([1.0, 0.0, 0.0].as_slice()));
        assert!(embeddings.vector("ناموجود").is_none());
        assert!(embeddings.contains("خانه"));
        assert!(!embeddings.contains("ناموجود"));

        let orthogonal = embeddings.similarity("کتاب", "خانه").unwrap();
        assert!(orthogonal.abs() < 1e-6);
        let diagonal = embeddings.similarity("کتاب", "مردم").unwrap();
        assert!((diagonal - 0.707_106_77).abs() < 1e-5);

        assert_eq!(embeddings.sentence_vector("کتاب خانه"), [0.5, 0.5, 0.0]);
        assert_eq!(embeddings.sentence_vector("کتاب ناموجود"), [1.0, 0.0, 0.0]);
        assert_eq!(embeddings.sentence_vector("ناموجود"), [0.0, 0.0, 0.0]);
        assert!(embeddings.sentence_similarity("کتاب", "کتاب") > 0.999_9);
        assert_eq!(embeddings.sentence_similarity("کتاب", "سلام"), 0.0);

        let nearest = embeddings.most_similar("کتاب", 2);
        assert_eq!(nearest.len(), 2);
        assert_eq!(nearest[0].0, "مردم");
        assert!((nearest[0].1 - 0.707_106_77).abs() < 1e-5);
        assert_eq!(nearest[1].0, "مدرسه");
        assert!(embeddings.most_similar("ناموجود", 2).is_empty());

        let batch = embeddings.sentence_vectors(&["کتاب".into(), "خانه".into()]);
        assert_eq!(batch, vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]]);
    }

    #[test]
    fn test_rejects_malformed_files() {
        let path = std::env::temp_dir().join(format!(
            "rishe_embeddings_bad_{}_{}.vec",
            std::process::id(),
            line!()
        ));
        std::fs::write(&path, "not a header\n").unwrap();
        assert!(Embeddings::load(&path).is_err());

        std::fs::write(&path, "1 3\nکتاب 1 0\n").unwrap();
        assert!(Embeddings::load(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}
