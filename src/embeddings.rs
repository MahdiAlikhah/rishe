use crate::tokenizer::Tokenizer;
use fst::{Map, MapBuilder};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::path::Path;

/// Magic prefix of the compact binary embedding format.
const MAGIC: &[u8; 4] = b"RVEC";
/// Binary format version written by this crate.
const VERSION: u16 = 1;
const DTYPE_F32: u8 = 0;
const DTYPE_F16: u8 = 1;
/// `RVEC` magic, version, dtype tag, `u32` dim and `u32` vocab size.
const HEADER_LEN: usize = 15;

/// Storage width of an embedding file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DType {
    F32,
    F16,
}

impl DType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F16 => "f16",
        }
    }

    /// Parse the name accepted on the command line.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "f32" | "float32" => Some(Self::F32),
            "f16" | "float16" | "half" => Some(Self::F16),
            _ => None,
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::F32 => DTYPE_F32,
            Self::F16 => DTYPE_F16,
        }
    }

    /// Bytes per stored value.
    fn width(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
        }
    }
}

/// Encode a `.rvec` header, vocabulary blob and vector payload into `writer`.
fn write_binary<W: Write>(
    writer: &mut W,
    dim: usize,
    words: &[Box<str>],
    vectors: &[f32],
    dtype: DType,
) -> Result<(), String> {
    let dim = u32::try_from(dim).map_err(|_| "embedding dimension is too large".to_string())?;
    let vocab = u32::try_from(words.len()).map_err(|_| "vocabulary is too large".to_string())?;
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&VERSION.to_le_bytes());
    header.push(dtype.tag());
    header.extend_from_slice(&dim.to_le_bytes());
    header.extend_from_slice(&vocab.to_le_bytes());
    writer
        .write_all(&header)
        .map_err(|error| format!("write error: {error}"))?;

    for word in words {
        let length = u16::try_from(word.len()).map_err(|_| {
            format!(
                "word {word:?} is longer than {} bytes and cannot be stored",
                u16::MAX
            )
        })?;
        writer
            .write_all(&length.to_le_bytes())
            .and_then(|()| writer.write_all(word.as_bytes()))
            .map_err(|error| format!("write error: {error}"))?;
    }

    let width = dtype.width();
    let mut buffer = Vec::with_capacity((1 << 16) * width);
    for value in vectors {
        match dtype {
            DType::F32 => buffer.extend_from_slice(&value.to_le_bytes()),
            DType::F16 => buffer.extend_from_slice(&f32_to_f16(*value).to_le_bytes()),
        }
        if buffer.len() >= (1 << 20) {
            writer
                .write_all(&buffer)
                .map_err(|error| format!("write error: {error}"))?;
            buffer.clear();
        }
    }
    writer
        .write_all(&buffer)
        .map_err(|error| format!("write error: {error}"))
}

/// Convert an IEEE 754 binary16 bit pattern to `f32`.
fn f16_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let mantissa = u32::from(bits & 0x03ff);
    let decoded = if exponent == 0 {
        if mantissa == 0 {
            sign
        } else {
            // Subnormal half: renormalize into a normal f32.
            let mut mantissa = mantissa;
            let mut shift = 0_u32;
            while mantissa & 0x0400 == 0 {
                mantissa <<= 1;
                shift += 1;
            }
            mantissa &= 0x03ff;
            let exponent = 127 - 15 - shift + 1;
            sign | (exponent << 23) | (mantissa << 13)
        }
    } else if exponent == 0x1f {
        // Inf or NaN.
        sign | (0xff << 23) | (mantissa << 13)
    } else {
        sign | ((exponent + 127 - 15) << 23) | (mantissa << 13)
    };
    f32::from_bits(decoded)
}

/// Convert `f32` to an IEEE 754 binary16 bit pattern, rounding half to even and
/// saturating to infinity like a hardware conversion.
fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        // Preserve infinity and collapse NaN payloads into a quiet NaN.
        let nan = if mantissa == 0 {
            0
        } else {
            0x200 | ((mantissa >> 13) as u16)
        };
        return sign | 0x7c00 | nan;
    }
    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        let shift = (14 - half_exponent) as u32;
        if shift > 24 {
            return sign;
        }
        let significand = mantissa | 0x80_0000;
        let half = significand >> shift;
        let remainder = significand & ((1 << shift) - 1);
        let halfway = 1_u32 << (shift - 1);
        let rounds_up = remainder > halfway || (remainder == halfway && half & 1 == 1);
        return sign | (half as u16 + u16::from(rounds_up));
    }
    let half = ((half_exponent as u32) << 10) | (mantissa >> 13);
    let remainder = mantissa & 0x1fff;
    let rounds_up = remainder > 0x1000 || (remainder == 0x1000 && half & 1 == 1);
    sign | (half as u16 + u16::from(rounds_up))
}

/// Static word embeddings loaded from a fastText/word2vec `.vec` text file or a
/// compact binary `.rvec` file.
///
/// The vocabulary is an FST mapping word -> row index; vectors live in one flat
/// row-major `f32` buffer, so lookups are O(word length) and the model is shared
/// read-only across threads. Binary files are decoded to `f32` on load; storing
/// them in `f16` only shrinks the file on disk.
#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
pub struct Embeddings {
    dim: usize,
    vectors: Vec<f32>,
    norms: Vec<f32>,
    words: Vec<Box<str>>,
    index: Map<Vec<u8>>,
    dtype: DType,
}

impl Embeddings {
    /// Load an embedding pack from a text `.vec` or binary `.rvec` file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let mut file =
            File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        let mut magic = [0_u8; 4];
        let read = file
            .read(&mut magic)
            .map_err(|error| format!("read error: {error}"))?;
        if read == 4 && &magic == MAGIC {
            file.rewind()
                .map_err(|error| format!("read error: {error}"))?;
            Self::load_binary(file)
        } else {
            let file = File::open(path)
                .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
            Self::load_text(file)
        }
    }

    /// Write this model as a compact binary `.rvec` file in the given storage
    /// width. The vocabulary order and row order are preserved, so an `f16`
    /// file decodes to values very close to the original `f32` ones.
    pub fn save(&self, path: impl AsRef<Path>, dtype: DType) -> Result<(), String> {
        let path = path.as_ref();
        let file = File::create(path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        let mut writer = std::io::BufWriter::with_capacity(1 << 20, file);
        write_binary(&mut writer, self.dim, &self.words, &self.vectors, dtype).and_then(|()| {
            writer
                .flush()
                .map_err(|error| format!("write error: {error}"))
        })
    }

    /// Storage width of the file this model was loaded from.
    pub fn dtype(&self) -> DType {
        self.dtype
    }

    fn load_text(file: File) -> Result<Self, String> {
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
            dtype: DType::F32,
        })
    }

    /// Read a compact binary `.rvec` file produced by [`Embeddings::save`].
    fn load_binary(mut file: File) -> Result<Self, String> {
        let mut header = [0_u8; HEADER_LEN];
        file.read_exact(&mut header)
            .map_err(|error| format!("read error: {error}"))?;
        if &header[..4] != MAGIC {
            return Err("not an .rvec file (missing RVEC magic)".to_string());
        }
        let version = u16::from_le_bytes([header[4], header[5]]);
        if version != VERSION {
            return Err(format!(
                "unsupported .rvec version {version}, this build reads version {VERSION}"
            ));
        }
        let dtype = match header[6] {
            DTYPE_F32 => DType::F32,
            DTYPE_F16 => DType::F16,
            other => return Err(format!("unknown .rvec dtype tag {other}")),
        };
        let dim = u32::from_le_bytes([header[7], header[8], header[9], header[10]]) as usize;
        let vocab = u32::from_le_bytes([header[11], header[12], header[13], header[14]]) as usize;
        if dim == 0 {
            return Err("embedding dimension must be positive".to_string());
        }
        if vocab == 0 {
            return Err("embedding file contains no vectors".to_string());
        }
        let count = vocab
            .checked_mul(dim)
            .ok_or_else(|| "embedding file is too large for available memory".to_string())?;

        let mut words: Vec<Box<str>> = Vec::new();
        words
            .try_reserve(vocab)
            .map_err(|_| "embedding file is too large for available memory".to_string())?;
        let mut scratch = vec![0_u8; u16::MAX as usize + 1];
        for _ in 0..vocab {
            let mut length = [0_u8; 2];
            file.read_exact(&mut length)
                .map_err(|error| format!("read error: {error}"))?;
            let length = u16::from_le_bytes(length) as usize;
            file.read_exact(&mut scratch[..length])
                .map_err(|error| format!("read error: {error}"))?;
            let word = std::str::from_utf8(&scratch[..length]).map_err(|_| {
                "embedding file contains a word that is not valid UTF-8".to_string()
            })?;
            words.push(word.into());
        }

        let mut vectors: Vec<f32> = Vec::new();
        vectors
            .try_reserve(count)
            .map_err(|_| "embedding file is too large for available memory".to_string())?;
        let width = match dtype {
            DType::F32 => 4,
            DType::F16 => 2,
        };
        // Decode in chunks so the raw bytes never sit in memory next to the
        // fully decoded buffer.
        let mut bytes = vec![0_u8; (1 << 20) * width];
        let mut remaining = count;
        while remaining > 0 {
            let take = remaining.min(bytes.len() / width);
            let chunk = &mut bytes[..take * width];
            file.read_exact(chunk)
                .map_err(|error| format!("read error: {error}"))?;
            match dtype {
                DType::F32 => vectors.extend(
                    chunk
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|&raw| f32::from_le_bytes(raw)),
                ),
                DType::F16 => vectors.extend(
                    chunk
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&raw| f16_to_f32(u16::from_le_bytes(raw))),
                ),
            }
            remaining -= take;
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
            dtype,
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

    /// Scratch file inside `target/`, which is writable in every test runner.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-test-tmp");
        std::fs::create_dir_all(&dir).expect("create test scratch dir");
        dir.join(name)
    }

    #[test]
    fn test_load_lookup_and_math() {
        let path = scratch("embeddings_load.vec");
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
        let path = scratch("embeddings_bad.vec");
        std::fs::write(&path, "not a header\n").unwrap();
        assert!(Embeddings::load(&path).is_err());

        std::fs::write(&path, "1 3\nکتاب 1 0\n").unwrap();
        assert!(Embeddings::load(&path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_binary_round_trip() {
        let text = scratch("embeddings_round.vec");
        std::fs::write(&text, TINY).unwrap();
        let embeddings = Embeddings::load(&text).unwrap();
        assert_eq!(embeddings.dtype(), DType::F32);

        let binary = scratch("embeddings_round.rvec");
        embeddings.save(&binary, DType::F32).unwrap();
        let reloaded = Embeddings::load(&binary).unwrap();
        assert_eq!(reloaded.dtype(), DType::F32);
        assert_eq!(reloaded.dim(), embeddings.dim());
        assert_eq!(reloaded.len(), embeddings.len());
        for word in ["کتاب", "خانه", "مدرسه", "سلام", "دنیا", "مردم"] {
            assert_eq!(reloaded.vector(word), embeddings.vector(word));
        }
        assert_eq!(
            reloaded.most_similar("کتاب", 8).len(),
            embeddings.most_similar("کتاب", 8).len()
        );

        let half_path = scratch("embeddings_round_half.rvec");
        embeddings.save(&half_path, DType::F16).unwrap();
        let half_bytes = std::fs::metadata(&half_path).unwrap().len();
        let full_bytes = std::fs::metadata(&binary).unwrap().len();
        assert!(
            half_bytes < full_bytes,
            "f16 file {half_bytes} vs f32 {full_bytes}"
        );

        let half = Embeddings::load(&half_path).unwrap();
        assert_eq!(half.dtype(), DType::F16);
        assert_eq!(half.len(), embeddings.len());
        for word in ["کتاب", "خانه", "مدرسه", "سلام", "دنیا", "مردم"] {
            let original = embeddings.vector(word).unwrap();
            let decoded = half.vector(word).unwrap();
            assert_eq!(decoded.len(), original.len());
            for (left, right) in original.iter().zip(decoded) {
                assert!(
                    (left - right).abs() <= 1e-3 * left.abs().max(1.0),
                    "{word}: {left} vs {right}"
                );
            }
        }
        let original = embeddings.similarity("کتاب", "خانه").unwrap();
        let decoded = half.similarity("کتاب", "خانه").unwrap();
        assert!((original - decoded).abs() < 1e-3);
        std::fs::remove_file(&text).ok();
        std::fs::remove_file(&binary).ok();
        std::fs::remove_file(&half_path).ok();
    }

    /// DO NOT EDIT this fixture: it pins `.rvec` version 1; the writer may
    /// evolve, this reader must always load it.
    const RVEC_V1_FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rvec-v1-f16.rvec"
    );

    #[test]
    fn rvec_v1_fixture_loads_forever() {
        let embeddings = Embeddings::load(RVEC_V1_FIXTURE).expect("load pinned .rvec v1 fixture");

        assert_eq!(embeddings.dim(), 3);
        assert_eq!(embeddings.len(), 5);
        assert_eq!(embeddings.dtype(), DType::F16);

        // Vocabulary order is the order stored in the file, so this pins the
        // u16-length-prefixed UTF-8 word encoding.
        let expected_words = ["کتاب", "خانه", "مدرسه", "سلام", "دنیا"];
        let words: Vec<&str> = embeddings.words.iter().map(|word| &**word).collect();
        assert_eq!(words, expected_words.to_vec());

        // Rows are pinned to the f16 encoding of the original f32 values.
        let expected_rows: [[f32; 3]; 5] = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.6, 0.8, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.5],
        ];
        for (word, expected) in expected_words.iter().zip(expected_rows) {
            let row = embeddings.vector(word).expect("fixture word is present");
            assert_eq!(row.len(), 3);
            for (actual, expected) in row.iter().zip(expected) {
                assert!(
                    (actual - expected).abs() <= 1e-3 * expected.abs().max(1.0),
                    "{word}: expected {expected}, decoded {actual}"
                );
            }
        }
        assert!(embeddings.contains("مدرسه"));
        assert!(embeddings.vector("ناموجود").is_none());
    }

    #[test]
    fn rvec_v1_fixture_corruption_is_rejected() {
        let fixture = std::fs::read(RVEC_V1_FIXTURE).expect("read pinned .rvec v1 fixture");
        assert_eq!(&fixture[..4], MAGIC);
        let path = scratch("rvec_v1_corrupt.rvec");
        let load_bytes = |bytes: &[u8]| {
            std::fs::write(&path, bytes).unwrap();
            Embeddings::load(&path)
        };

        // Truncated header, vocabulary and payload must return Err, never panic.
        assert!(load_bytes(&fixture[..10]).is_err());
        assert!(load_bytes(&fixture[..20]).is_err());
        assert!(load_bytes(&fixture[..fixture.len() - 1]).is_err());

        let mut wrong_version = fixture.clone();
        wrong_version[4] = 2;
        assert!(load_bytes(&wrong_version).is_err());

        let mut wrong_magic = fixture.clone();
        wrong_magic[0] = b'X';
        assert!(load_bytes(&wrong_magic).is_err());

        // A header that claims a huge vocabulary must be refused, not abort the
        // process while reserving memory for the word list.
        let mut huge_vocab = Vec::new();
        huge_vocab.extend_from_slice(MAGIC);
        huge_vocab.extend_from_slice(&1_u16.to_le_bytes());
        huge_vocab.push(DTYPE_F16);
        huge_vocab.extend_from_slice(&1_u32.to_le_bytes());
        huge_vocab.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(load_bytes(&huge_vocab).is_err());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_rejects_malformed_binaries() {
        fn header(version: u16, dtype: u8, dim: u32, vocab: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(MAGIC);
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.push(dtype);
            bytes.extend_from_slice(&dim.to_le_bytes());
            bytes.extend_from_slice(&vocab.to_le_bytes());
            bytes
        }

        let path = scratch("embeddings_bad.rvec");
        let rejected = |bytes: &[u8]| {
            std::fs::write(&path, bytes).unwrap();
            Embeddings::load(&path).is_err()
        };

        assert!(rejected(b"RVEC\x02\x00\x00"));
        assert!(rejected(&header(VERSION + 1, DTYPE_F16, 4, 2)));
        assert!(rejected(&header(VERSION, 7, 4, 2)));
        assert!(rejected(&header(VERSION, DTYPE_F16, 0, 2)));
        assert!(rejected(&header(VERSION, DTYPE_F16, 4, 0)));
        // A well-formed header with a truncated payload must fail cleanly.
        assert!(rejected(&header(VERSION, DTYPE_F16, 4, 2)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_half_precision_conversion() {
        assert_eq!(f16_to_f32(f32_to_f16(0.0)), 0.0);
        assert_eq!(f16_to_f32(f32_to_f16(-0.0)).to_bits(), (-0.0_f32).to_bits());
        assert_eq!(f16_to_f32(f32_to_f16(1.0)), 1.0);
        assert_eq!(f16_to_f32(f32_to_f16(-2.0)), -2.0);
        assert_eq!(f16_to_f32(f32_to_f16(65_504.0)), 65_504.0);
        assert!(f16_to_f32(f32_to_f16(1.0e9)).is_infinite());
        assert!(f16_to_f32(f32_to_f16(f32::NAN)).is_nan());
        assert!(f16_to_f32(f32_to_f16(f32::INFINITY)).is_infinite());
        for value in [0.1_f32, 0.6, -0.8, std::f32::consts::PI, 1.0e-5, 1.0e-8] {
            let round_trip = f16_to_f32(f32_to_f16(value));
            assert!(
                (value - round_trip).abs() <= 1e-3 * value.abs() + 6.0e-8,
                "{value} -> {round_trip}"
            );
        }
        // Subnormals must survive the trip instead of flushing to zero.
        let tiny = f16_to_f32(0x0001);
        assert!(tiny > 0.0 && tiny < 1.0e-6);
        assert_eq!(f32_to_f16(tiny), 0x0001);
    }

    #[test]
    fn embedding_loaded_pack() {
        let Ok(path) = std::env::var("RISHE_EMBEDDINGS_PACK") else {
            return;
        };
        let embeddings = Embeddings::load(&path).expect("load embedding pack");
        assert_eq!(embeddings.dim(), 300);
        assert!(embeddings.len() >= 30_000);
        assert!(embeddings.contains("کتاب"));
        assert_eq!(embeddings.most_similar("کتاب", 5).len(), 5);
        assert!((embeddings.similarity("کتاب", "کتاب").unwrap() - 1.0).abs() < 1e-5);
        let vector = embeddings.sentence_vector("این کتاب خوب است");
        assert_eq!(vector.len(), 300);
        assert!(vector.iter().any(|value| *value != 0.0));
    }
}
