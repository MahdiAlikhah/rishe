#![allow(clippy::incompatible_msrv)] // the optional `models` feature requires Rust 1.91 via tract

use std::fs;
use std::path::Path;

use ndarray::Array2;
use rayon::prelude::*;
use tokenizers::Tokenizer as ModelTokenizer;
use tract::prelude::*;

use crate::tokenizer::Tokenizer as WordTokenizer;

tract::impl_ndarray_interop!();

#[cfg_attr(feature = "python", pyo3::pyclass(frozen, module = "rishe._core"))]
#[derive(Debug, Clone)]
pub struct TaggedToken {
    pub text: String,
    pub tag: String,
    pub start: usize,
    pub end: usize,
}

#[cfg_attr(feature = "python", pyo3::pyclass(frozen, module = "rishe._core"))]
#[derive(Debug, Clone)]
pub struct Entity {
    pub text: String,
    pub label: String,
    pub start: usize,
    pub end: usize,
}

pub fn merge_entities(tokens: &[TaggedToken]) -> Vec<Entity> {
    let mut entities = Vec::new();
    let mut current: Option<(String, usize, usize, String)> = None;

    for token in tokens {
        let (kind, label) = token
            .tag
            .split_once('_')
            .or_else(|| token.tag.split_once('-'))
            .unwrap_or(("", token.tag.as_str()));
        if kind != "B" && kind != "I" {
            flush_entity(&mut current, &mut entities);
            continue;
        }

        let same = current
            .as_ref()
            .map(|(_, _, _, current_label)| current_label == label)
            .unwrap_or(false);
        if kind == "B" || !same {
            flush_entity(&mut current, &mut entities);
            current = Some((token.text.clone(), token.start, token.end, label.to_owned()));
        } else if let Some((text, _, end, _)) = current.as_mut() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&token.text);
            *end = token.end;
        }
    }
    flush_entity(&mut current, &mut entities);
    entities
}

fn flush_entity(current: &mut Option<(String, usize, usize, String)>, entities: &mut Vec<Entity>) {
    if let Some((text, start, end, label)) = current.take() {
        entities.push(Entity {
            text,
            label,
            start,
            end,
        });
    }
}

/// A single task pack: tokenizer, labels and ONNX model.
#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
pub struct ModelPack {
    task: String,
    labels: Vec<String>,
    max_len: usize,
    tokenizer: ModelTokenizer,
    runnable: Runnable,
}

impl ModelPack {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(format!("model pack is not a directory: {}", path.display()));
        }

        let labels_path = path.join("labels.json");
        let tokenizer_path = path.join("tokenizer.json");
        let model_path = path.join("model.onnx");
        for (name, file) in [
            ("labels.json", &labels_path),
            ("tokenizer.json", &tokenizer_path),
            ("model.onnx", &model_path),
        ] {
            if !file.is_file() {
                return Err(format!("{name} is missing from {}", path.display()));
            }
        }

        let metadata: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&labels_path)
                .map_err(|error| format!("cannot read {}: {error}", labels_path.display()))?,
        )
        .map_err(|error| format!("cannot parse {}: {error}", labels_path.display()))?;
        let labels: Vec<String> = metadata
            .get("labels")
            .and_then(|value| value.as_array())
            .ok_or_else(|| format!("{} has no labels array", labels_path.display()))?
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect();
        if labels.is_empty() {
            return Err(format!("{} has no labels", labels_path.display()));
        }
        let task = metadata
            .get("task")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_owned();
        let max_len = metadata
            .get("max_len")
            .and_then(|value| value.as_u64())
            .unwrap_or(192) as usize;
        if max_len < 3 {
            return Err(format!(
                "invalid max_len {max_len} in {}",
                labels_path.display()
            ));
        }

        let tokenizer = ModelTokenizer::from_file(&tokenizer_path)
            .map_err(|error| format!("cannot load {}: {error}", tokenizer_path.display()))?;

        let model = tract::onnx()
            .map_err(|error| format!("cannot initialise tract: {error}"))?
            .load(&model_path)
            .map_err(|error| format!("cannot load {}: {error}", model_path.display()))?
            .into_model()
            .map_err(|error| format!("cannot prepare {}: {error}", model_path.display()))?;
        let runtime = tract::runtime_for_name("default")
            .map_err(|error| format!("cannot create tract runtime: {error}"))?;
        let runnable = runtime
            .prepare(model)
            .map_err(|error| format!("cannot prepare {}: {error}", model_path.display()))?;

        Ok(Self {
            task,
            labels,
            max_len,
            tokenizer,
            runnable,
        })
    }

    pub fn task(&self) -> &str {
        &self.task
    }

    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    pub fn max_len(&self) -> usize {
        self.max_len
    }

    pub fn tag_text(&self, text: &str) -> Result<Vec<(String, String)>, String> {
        Ok(self
            .tag_text_items(text)?
            .into_iter()
            .map(|token| (token.text, token.tag))
            .collect())
    }

    pub fn tag_text_items(&self, text: &str) -> Result<Vec<TaggedToken>, String> {
        let tokens = WordTokenizer::tokenize(text);
        let words: Vec<&str> = tokens.iter().map(|token| token.text).collect();
        let tagged = self.tag_words(&words)?;
        let mut cursor = 0;
        let mut result = Vec::with_capacity(tagged.len());

        for (word, tag) in tagged {
            let Some(offset) = tokens[cursor..]
                .iter()
                .position(|token| token.text == word.as_str())
            else {
                continue;
            };
            let token = &tokens[cursor + offset];
            result.push(TaggedToken {
                text: word,
                tag,
                start: text[..token.start].chars().count(),
                end: text[..token.end].chars().count(),
            });
            cursor += offset + 1;
        }
        Ok(result)
    }

    pub fn entities(&self, text: &str) -> Result<Vec<Entity>, String> {
        Ok(merge_entities(&self.tag_text_items(text)?))
    }

    // ponytail: parallel per-sentence; re-export with batch>1 if ONNX batching matters.
    pub fn tag_text_batch(&self, texts: &[String]) -> Result<Vec<Vec<TaggedToken>>, String> {
        texts
            .par_iter()
            .map(|text| self.tag_text_items(text))
            .collect()
    }

    pub fn entities_batch(&self, texts: &[String]) -> Result<Vec<Vec<Entity>>, String> {
        Ok(self
            .tag_text_batch(texts)?
            .iter()
            .map(|tokens| merge_entities(tokens))
            .collect())
    }

    /// Word-level single call.
    pub fn tag_words(&self, words: &[&str]) -> Result<Vec<(String, String)>, String> {
        if words.is_empty() {
            return Ok(Vec::new());
        }

        let cls = self
            .tokenizer
            .token_to_id("[CLS]")
            .ok_or_else(|| "tokenizer has no [CLS] token".to_string())? as i64;
        let sep = self
            .tokenizer
            .token_to_id("[SEP]")
            .ok_or_else(|| "tokenizer has no [SEP] token".to_string())? as i64;
        let unk = self.tokenizer.token_to_id("<unk>");

        let mut ids = Vec::with_capacity(self.max_len);
        let mut positions = Vec::new();
        let mut kept = Vec::new();
        ids.push(cls);
        for word in words {
            let encoding = self
                .tokenizer
                .encode(*word, false)
                .map_err(|error| format!("cannot tokenize {word:?}: {error}"))?;
            let mut pieces = encoding.get_ids().to_vec();
            if pieces.is_empty() {
                match unk {
                    Some(unk) => pieces.push(unk),
                    None => continue,
                }
            }
            if ids.len() + pieces.len() >= self.max_len {
                break;
            }
            positions.push(ids.len());
            ids.extend(pieces.into_iter().map(i64::from));
            kept.push((*word).to_owned());
        }
        if kept.is_empty() {
            return Ok(Vec::new());
        }
        ids.push(sep);

        let shape = (1, ids.len());
        let input_ids = Array2::from_shape_vec(shape, ids)
            .map_err(|error| format!("invalid input shape: {error}"))?
            .tract()
            .map_err(|error| format!("cannot build input tensor: {error}"))?;
        let attention_mask = Array2::<i64>::ones(shape)
            .tract()
            .map_err(|error| format!("cannot build attention mask: {error}"))?;
        let outputs = self
            .runnable
            .run([input_ids, attention_mask])
            .map_err(|error| format!("inference failed: {error}"))?;
        let logits = outputs
            .first()
            .ok_or_else(|| "model returned no outputs".to_string())?
            .ndarray::<f32>()
            .map_err(|error| format!("cannot read logits: {error}"))?;

        let label_count = logits.shape().get(2).copied().unwrap_or(0);
        if label_count != self.labels.len() {
            return Err(format!(
                "model has {label_count} labels, labels.json has {}",
                self.labels.len()
            ));
        }

        let mut result = Vec::with_capacity(kept.len());
        for (word, position) in kept.into_iter().zip(positions) {
            let mut best = 0;
            for label in 1..label_count {
                if logits[[0, position, label]] > logits[[0, position, best]] {
                    best = label;
                }
            }
            result.push((word, self.labels[best].clone()));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, tag: &str, start: usize, end: usize) -> TaggedToken {
        TaggedToken {
            text: text.to_owned(),
            tag: tag.to_owned(),
            start,
            end,
        }
    }

    #[test]
    fn merges_bio_entities() {
        let tokens = [
            token("امیر", "B_PER", 0, 4),
            token("کبیر", "I_PER", 5, 9),
            token("به", "O", 10, 12),
            token("تهران", "B_LOC", 13, 18),
        ];
        let entities = merge_entities(&tokens);
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].text, "امیر کبیر");
        assert_eq!(entities[0].label, "PER");
        assert_eq!((entities[0].start, entities[0].end), (0, 9));
        assert_eq!(entities[1].text, "تهران");
        assert_eq!(entities[1].label, "LOC");
    }

    #[test]
    fn handles_dash_and_broken_i_tags() {
        let tokens = [
            token("الف", "B-PER", 0, 3),
            token("ب", "I-LOC", 4, 5),
            token("پ", "I_LOC", 6, 7),
            token("ت", "O", 8, 9),
        ];
        let entities = merge_entities(&tokens);
        assert_eq!(entities.len(), 2);
        assert_eq!(
            (entities[0].text.as_str(), entities[0].label.as_str()),
            ("الف", "PER")
        );
        assert_eq!(
            (entities[1].text.as_str(), entities[1].label.as_str()),
            ("ب پ", "LOC")
        );
    }

    #[test]
    fn loads_tiny_pack() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let pack = ModelPack::load(&path).expect("load tiny pack");
        assert_eq!(pack.task(), "pos");
        assert_eq!(
            pack.tag_text("good bad").unwrap(),
            vec![
                ("good".to_string(), "A".to_string()),
                ("bad".to_string(), "B".to_string()),
            ]
        );
    }
}
