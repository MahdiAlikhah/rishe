#![allow(clippy::incompatible_msrv)] // the optional `models` feature requires Rust 1.91 via tract

use std::fs;
use std::path::Path;

use ndarray::Array2;
use tokenizers::Tokenizer as ModelTokenizer;
use tract::prelude::*;

use crate::tokenizer::Tokenizer as WordTokenizer;

tract::impl_ndarray_interop!();

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
        let tokens = WordTokenizer::tokenize(text);
        let words: Vec<&str> = tokens.into_iter().map(|token| token.text).collect();
        self.tag_words(&words)
    }

    /// One sentence per call. Batch when latency matters.
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
