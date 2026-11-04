#![allow(clippy::incompatible_msrv)] // the optional `models` feature requires Rust 1.91 via tract

use std::fs;
use std::path::Path;

use ndarray::Array2;
use rayon::prelude::*;
use tokenizers::Tokenizer as ModelTokenizer;
use tract::prelude::*;

use crate::tokenizer::Tokenizer as WordTokenizer;

tract::impl_ndarray_interop!();

#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, module = "rishe._core", get_all)
)]
#[derive(Debug, Clone)]
pub struct TaggedToken {
    pub text: String,
    pub tag: String,
    pub start: usize,
    pub end: usize,
}

#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, module = "rishe._core", get_all)
)]
#[derive(Debug, Clone)]
pub struct Entity {
    pub text: String,
    pub label: String,
    pub start: usize,
    pub end: usize,
}

#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, module = "rishe._core", get_all)
)]
/// One word and its dependency head; `head` is zero-based or `None` for the root.
#[derive(Debug, Clone)]
pub struct Dependency {
    pub text: String,
    pub head: Option<usize>,
    pub relation: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy)]
struct ArcEdge {
    from: usize,
    to: usize,
    weight: f64,
    from_original: usize,
    to_original: usize,
}

fn maximum_spanning_tree(scores: &[Vec<f64>]) -> Option<Vec<usize>> {
    let word_count = scores.len();
    if word_count == 0 {
        return Some(Vec::new());
    }
    let root = word_count;
    let mut edges = Vec::with_capacity(word_count * (word_count + 1));
    for (dependent, row) in scores.iter().enumerate() {
        for head in 0..word_count {
            if head != dependent {
                edges.push(ArcEdge {
                    from: head,
                    to: dependent,
                    weight: row[head + 1],
                    from_original: head,
                    to_original: dependent,
                });
            }
        }
        edges.push(ArcEdge {
            from: root,
            to: dependent,
            weight: row[0] - 1_000_000_000.0,
            from_original: root,
            to_original: dependent,
        });
    }
    let parents = maximum_arborescence(word_count + 1, root, &edges)?;
    Some(parents[..word_count].to_vec())
}

fn maximum_arborescence(node_count: usize, root: usize, edges: &[ArcEdge]) -> Option<Vec<usize>> {
    let mut incoming = vec![f64::NEG_INFINITY; node_count];
    let mut parent = vec![usize::MAX; node_count];
    for edge in edges {
        if edge.from != edge.to && edge.weight > incoming[edge.to] {
            incoming[edge.to] = edge.weight;
            parent[edge.to] = edge.from;
        }
    }
    incoming[root] = 0.0;
    parent[root] = root;
    if (0..node_count).any(|node| node != root && parent[node] == usize::MAX) {
        return None;
    }

    let mut component = vec![usize::MAX; node_count];
    let mut visited = vec![usize::MAX; node_count];
    let mut components = 0;
    for start in 0..node_count {
        if start == root {
            continue;
        }
        let mut node = start;
        while node != root && visited[node] != start && component[node] == usize::MAX {
            visited[node] = start;
            node = parent[node];
        }
        if node != root && component[node] == usize::MAX {
            let mut member = parent[node];
            component[node] = components;
            while member != node {
                component[member] = components;
                member = parent[member];
            }
            components += 1;
        }
    }
    if components == 0 {
        return Some(parent);
    }

    for component in &mut component {
        if *component == usize::MAX {
            *component = components;
            components += 1;
        }
    }
    let contracted_root = component[root];
    let mut contracted = Vec::with_capacity(edges.len());
    for edge in edges {
        let from = component[edge.from];
        let to = component[edge.to];
        if from == to {
            continue;
        }
        contracted.push(ArcEdge {
            from,
            to,
            weight: edge.weight - incoming[edge.to],
            from_original: edge.from,
            to_original: edge.to,
        });
    }
    let contracted_parent = maximum_arborescence(components, contracted_root, &contracted)?;
    let mut result = parent;
    for (node, component_parent) in contracted_parent.into_iter().enumerate() {
        if node == contracted_root {
            continue;
        }
        if let Some(edge) = contracted
            .iter()
            .filter(|edge| edge.from == component_parent && edge.to == node)
            .max_by(|left, right| left.weight.total_cmp(&right.weight))
        {
            result[edge.to_original] = edge.from_original;
        }
    }
    Some(result)
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

type EncodedWords = (Vec<i64>, Vec<usize>, Vec<String>);

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
        let runtime_name = if cfg!(feature = "cuda") {
            "cuda"
        } else if cfg!(feature = "metal") {
            "metal"
        } else {
            "default"
        };
        let runtime = tract::runtime_for_name(runtime_name)
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

    fn encode_words(&self, words: &[&str]) -> Result<Option<EncodedWords>, String> {
        if words.is_empty() {
            return Ok(None);
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
            return Ok(None);
        }
        ids.push(sep);
        Ok(Some((ids, positions, kept)))
    }

    /// Word-level single call.
    pub fn tag_words(&self, words: &[&str]) -> Result<Vec<(String, String)>, String> {
        let Some((ids, positions, kept)) = self.encode_words(words)? else {
            return Ok(Vec::new());
        };

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

    pub fn depparse(&self, text: &str) -> Result<Vec<Dependency>, String> {
        let tokens = WordTokenizer::tokenize(text);
        let words: Vec<&str> = tokens.iter().map(|token| token.text).collect();
        let Some((ids, positions, kept)) = self.encode_words(&words)? else {
            return Ok(Vec::new());
        };
        let word_count = kept.len();
        let token_count = ids.len();
        let pad_id = self.tokenizer.token_to_id("[PAD]").unwrap_or(0) as i64;
        let mut padded_ids = vec![pad_id; self.max_len];
        padded_ids[..token_count].copy_from_slice(&ids);
        let mut attention = vec![0_i64; self.max_len];
        attention[..token_count].fill(1);
        let sequence_shape = (1, self.max_len);
        let word_shape = (1, word_count);
        let positions: Vec<i64> = positions.into_iter().map(|value| value as i64).collect();
        let mask = vec![true; word_count];

        let scores = {
            let input_ids = Array2::from_shape_vec(sequence_shape, padded_ids.clone())
                .map_err(|error| format!("invalid input shape: {error}"))?
                .tract()
                .map_err(|error| format!("cannot build input tensor: {error}"))?;
            let attention_mask = Array2::from_shape_vec(sequence_shape, attention.clone())
                .map_err(|error| format!("invalid attention mask: {error}"))?
                .tract()
                .map_err(|error| format!("cannot build attention mask: {error}"))?;
            let word_positions = Array2::from_shape_vec(word_shape, positions.clone())
                .map_err(|error| format!("invalid word positions: {error}"))?
                .tract()
                .map_err(|error| format!("cannot build word positions: {error}"))?;
            let word_mask = Array2::from_shape_vec(word_shape, mask.clone())
                .map_err(|error| format!("invalid word mask: {error}"))?
                .tract()
                .map_err(|error| format!("cannot build word mask: {error}"))?;
            let head_indices = Array2::<i64>::zeros(word_shape)
                .tract()
                .map_err(|error| format!("cannot build head indices: {error}"))?;
            let outputs = self
                .runnable
                .run([
                    input_ids,
                    attention_mask,
                    word_positions,
                    word_mask,
                    head_indices,
                ])
                .map_err(|error| format!("inference failed: {error}"))?;
            let arcs = outputs
                .first()
                .ok_or_else(|| "model returned no arc scores".to_string())?
                .ndarray::<f32>()
                .map_err(|error| format!("cannot read arc scores: {error}"))?;
            if arcs.shape().get(2).copied() != Some(word_count + 1) {
                return Err("arc scores have an unexpected shape".to_string());
            }
            (0..word_count)
                .map(|dependent| {
                    (0..=word_count)
                        .map(|head| arcs[[0, dependent, head]] as f64)
                        .collect()
                })
                .collect::<Vec<Vec<f64>>>()
        };

        let parents = maximum_spanning_tree(&scores)
            .ok_or_else(|| "cannot decode a dependency tree".to_string())?;
        let head_indices: Vec<i64> = parents
            .iter()
            .map(|&parent| {
                if parent == word_count {
                    0
                } else {
                    parent as i64 + 1
                }
            })
            .collect();

        let input_ids = Array2::from_shape_vec(sequence_shape, padded_ids)
            .map_err(|error| format!("invalid input shape: {error}"))?
            .tract()
            .map_err(|error| format!("cannot build input tensor: {error}"))?;
        let attention_mask = Array2::from_shape_vec(sequence_shape, attention)
            .map_err(|error| format!("invalid attention mask: {error}"))?
            .tract()
            .map_err(|error| format!("cannot build attention mask: {error}"))?;
        let word_positions = Array2::from_shape_vec(word_shape, positions)
            .map_err(|error| format!("invalid word positions: {error}"))?
            .tract()
            .map_err(|error| format!("cannot build word positions: {error}"))?;
        let word_mask = Array2::from_shape_vec(word_shape, mask)
            .map_err(|error| format!("invalid word mask: {error}"))?
            .tract()
            .map_err(|error| format!("cannot build word mask: {error}"))?;
        let head_indices = Array2::from_shape_vec(word_shape, head_indices)
            .map_err(|error| format!("invalid head indices: {error}"))?
            .tract()
            .map_err(|error| format!("cannot build head indices: {error}"))?;
        let outputs = self
            .runnable
            .run([
                input_ids,
                attention_mask,
                word_positions,
                word_mask,
                head_indices,
            ])
            .map_err(|error| format!("inference failed: {error}"))?;
        let relations = outputs
            .get(1)
            .ok_or_else(|| "model returned no relation logits".to_string())?
            .ndarray::<f32>()
            .map_err(|error| format!("cannot read relation logits: {error}"))?;
        let label_count = relations.shape().get(2).copied().unwrap_or(0);
        if label_count != self.labels.len() {
            return Err(format!(
                "model has {label_count} labels, labels.json has {}",
                self.labels.len()
            ));
        }

        let mut cursor = 0;
        let mut result = Vec::with_capacity(word_count);
        for (index, word) in kept.iter().enumerate() {
            let Some(offset) = tokens[cursor..]
                .iter()
                .position(|token| token.text == word.as_str())
            else {
                continue;
            };
            let token = &tokens[cursor + offset];
            let mut best = 0;
            for label in 1..label_count {
                if relations[[0, index, label]] > relations[[0, index, best]] {
                    best = label;
                }
            }
            let parent = parents[index];
            result.push(Dependency {
                text: word.clone(),
                head: if parent == word_count {
                    None
                } else {
                    Some(parent)
                },
                relation: self.labels[best].clone(),
                start: text[..token.start].chars().count(),
                end: text[..token.end].chars().count(),
            });
            cursor += offset + 1;
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
    fn mst_breaks_cycle_with_single_root() {
        let scores = vec![vec![8.0, 0.0, 10.0], vec![9.0, 10.0, 0.0]];
        assert_eq!(maximum_spanning_tree(&scores), Some(vec![1, 2]));
    }

    #[test]
    fn mst_roots_a_single_word() {
        assert_eq!(maximum_spanning_tree(&[vec![5.0]]), Some(vec![1]));
    }

    #[test]
    fn depparse_loaded_pack() {
        let Ok(path) = std::env::var("RISHE_DEPPARSE_PACK") else {
            return;
        };
        let pack = ModelPack::load(path).expect("load depparse pack");
        let dependencies = pack
            .depparse("کتاب‌های خوب را به دوستانم می‌دهم.")
            .expect("depparse");
        assert!(!dependencies.is_empty());
        assert_eq!(
            dependencies
                .iter()
                .filter(|item| item.head.is_none())
                .count(),
            1
        );
        for dependency in &dependencies {
            if let Some(head) = dependency.head {
                assert!(head < dependencies.len());
            }
        }
        for start in 0..dependencies.len() {
            let mut seen = vec![false; dependencies.len()];
            let mut node = start;
            while let Some(head) = dependencies[node].head {
                assert!(!seen[node]);
                seen[node] = true;
                node = head;
            }
        }
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
