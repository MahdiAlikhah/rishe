#![allow(clippy::incompatible_msrv)] // the optional `models` feature requires Rust 1.91 via tract

use std::fs;
use std::path::Path;

use ndarray::Array2;
use rayon::prelude::*;
use tokenizers::Tokenizer as ModelTokenizer;
use tract::prelude::*;

use crate::tokenizer::Tokenizer as WordTokenizer;

tract::impl_ndarray_interop!();

/// Most words one dependency parse accepts.
///
/// The decoder scores every (word, head) pair, so its memory is quadratic in the
/// word count: 4094 words — the most a 4096-piece pack can encode in one pass —
/// needs roughly 700 MB, which turns a long paragraph into an out-of-memory kill
/// rather than an error. A real sentence is far below this. The limit is applied
/// after encoding, so it counts encoded words, not characters.
pub(crate) const MAX_PARSE_WORDS: usize = 512;

/// Counts characters up to a byte offset, continuing from the previous call.
/// Tokens arrive in text order, so this turns a per-token `text[..at].chars()`
/// scan into one pass over the text.
#[derive(Default)]
struct CharOffset {
    byte: usize,
    chars: usize,
}

impl CharOffset {
    fn upto(&mut self, text: &str, byte: usize) -> usize {
        if let Some(slice) = text.get(self.byte..byte) {
            self.chars += slice.chars().count();
            self.byte = byte;
        }
        self.chars
    }
}

/// A word with a POS tag and character offsets.
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, module = "rishe._core", get_all)
)]
#[derive(Debug, Clone)]
pub struct TaggedToken {
    /// Token text.
    pub text: String,
    /// Tag label.
    pub tag: String,
    /// Character offset of the first character.
    pub start: usize,
    /// Character offset one past the last character.
    pub end: usize,
}

/// Reject a model whose output has a different label count than `labels.json`.
///
/// Split out from the inference paths so the check can be tested without an ONNX
/// model on disk.
fn check_labels(what: &str, found: usize, expected: usize) -> Result<(), String> {
    if found == expected {
        return Ok(());
    }
    Err(format!(
        "{what} has {found} labels, labels.json has {expected}"
    ))
}

/// A named entity merged from BIO-tagged tokens.
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, module = "rishe._core", get_all)
)]
#[derive(Debug, Clone)]
pub struct Entity {
    /// Entity text, words joined by single spaces when the entity spans tokens.
    pub text: String,
    /// Entity label without the BIO prefix.
    pub label: String,
    /// Character offset of the first character.
    pub start: usize,
    /// Character offset one past the last character.
    pub end: usize,
}

#[cfg_attr(
    feature = "python",
    pyo3::pyclass(frozen, module = "rishe._core", get_all)
)]
/// One word and its dependency head; `head` is zero-based or `None` for the root.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Token text.
    pub text: String,
    /// Zero-based head index, or `None` for the root.
    pub head: Option<usize>,
    /// Dependency relation label.
    pub relation: String,
    /// Character offset of the first character.
    pub start: usize,
    /// Character offset one past the last character.
    pub end: usize,
}

impl TaggedToken {
    /// The label of this span. `tag` keeps the historical field name.
    pub fn label(&self) -> &str {
        &self.tag
    }
}

impl Dependency {
    /// The label of this dependency relation. `relation` keeps the historical
    /// field name.
    pub fn label(&self) -> &str {
        &self.relation
    }
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
    // Bucket the contracted edges by head, so putting the original edges back only
    // looks at one node's incoming edges instead of rescanning every edge per node.
    let mut incoming_edges: Vec<Vec<ArcEdge>> = vec![Vec::new(); components];
    for edge in edges {
        let from = component[edge.from];
        let to = component[edge.to];
        if from == to {
            continue;
        }
        let contracted_edge = ArcEdge {
            from,
            to,
            weight: edge.weight - incoming[edge.to],
            from_original: edge.from,
            to_original: edge.to,
        };
        contracted.push(contracted_edge);
        incoming_edges[to].push(contracted_edge);
    }
    let contracted_parent = maximum_arborescence(components, contracted_root, &contracted)?;
    let mut result = parent;
    for (node, component_parent) in contracted_parent.into_iter().enumerate() {
        if node == contracted_root {
            continue;
        }
        if let Some(edge) = incoming_edges[node]
            .iter()
            .filter(|edge| edge.from == component_parent)
            .max_by(|left, right| left.weight.total_cmp(&right.weight))
        {
            result[edge.to_original] = edge.from_original;
        }
    }
    Some(result)
}

/// Merge BIO-tagged tokens into entities, tolerating `_`/`-` prefixes and broken `I` tags.
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
    /// Whether the model takes `token_type_ids` as a third input.
    ///
    /// Most packs here take `(input_ids, attention_mask)`. The Hugging Face
    /// XLM-R exports of e5 take a third all-zero segment id, and feeding two
    /// inputs fails with "Wrong number of inputs for model. Expected 3 got 2".
    needs_token_type_ids: bool,
}

/// Encoded ids, per-word positions, the words kept, and how many input words
/// were consumed (which can exceed `kept.len()` when a word has no pieces).
type EncodedWords = (Vec<i64>, Vec<usize>, Vec<String>, usize);

impl ModelPack {
    /// Load a pack directory holding `model.onnx`, `tokenizer.json` and
    /// `labels.json`; errors on a missing or malformed file.
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
        let mut non_string_label = None;
        let labels: Vec<String> = metadata
            .get("labels")
            .and_then(|value| value.as_array())
            .ok_or_else(|| format!("{} has no labels array", labels_path.display()))?
            .iter()
            .enumerate()
            .filter_map(|(index, value)| match value.as_str() {
                Some(label) => Some(label.to_owned()),
                // A non-string label used to be dropped, so the model's class
                // index no longer matched the label list. Refuse it instead.
                None => {
                    non_string_label = Some(index);
                    None
                }
            })
            .collect();
        if let Some(index) = non_string_label {
            return Err(format!(
                "labels[{index}] in {} is not a string",
                labels_path.display()
            ));
        }
        if labels.is_empty() {
            // A sentence encoder produces a vector, not a class, so it has no
            // labels to name. Every tagging task still needs them.
            let is_sentence = metadata
                .get("task")
                .and_then(|value| value.as_str())
                .is_some_and(|task| task == "sentence");
            if !is_sentence {
                return Err(format!("{} has no labels", labels_path.display()));
            }
        }
        let task = metadata
            .get("task")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_owned();
        // `as_u64` rejects `16.0`, `"16"`, `-5` and `null` by returning `None`,
        // which then silently became the 192 default and skipped the range check.
        let max_len = match metadata.get("max_len") {
            None => 192,
            Some(value) => match value.as_u64() {
                Some(value) => value as usize,
                None => {
                    return Err(format!(
                        "invalid max_len {value} in {}; expected an integer 3..=4096",
                        labels_path.display()
                    ))
                }
            },
        };
        if !(3..=4096).contains(&max_len) {
            return Err(format!(
                "invalid max_len {max_len} in {}; expected 3..=4096",
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
        // Most packs here take `(input_ids, attention_mask)`. The Hugging Face
        // XLM-R exports of e5 take a third segment-id input; feeding two fails
        // with "Wrong number of inputs for model. Expected 3 got 2".
        let inputs = runnable
            .input_count()
            .map_err(|error| format!("cannot read input count: {error}"))?;
        if !(2..=3).contains(&inputs) {
            return Err(format!(
                "{} takes {inputs} inputs; expected 2 or 3",
                model_path.display()
            ));
        }

        Ok(Self {
            task,
            labels,
            max_len,
            tokenizer,
            runnable,
            needs_token_type_ids: inputs == 3,
        })
    }

    /// Task the pack was built for: `pos`, `ner`, `ezafe` or `depparse`.
    pub fn task(&self) -> &str {
        &self.task
    }

    /// Label names in model output order.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Maximum token count the pack accepts.
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    /// Tag text into spans. Each span carries `text`, `tag`, `start` and `end`;
    /// `label()` returns the tag under the name every result type shares.
    ///
    /// A token the tokenizer cannot represent at all — one whose own subword
    /// pieces exceed `max_len`, and which has no `[UNK]` to stand in for it —
    /// keeps no tag and is left out of the result, so the output can be shorter
    /// than the token stream. That is deliberate: the alternative was dropping
    /// every word after it. A text made only of such a token is an error instead,
    /// because an empty list would read as "nothing to tag".
    pub fn tags(&self, text: &str) -> Result<Vec<TaggedToken>, String> {
        let tokens = WordTokenizer::tokenize(text);
        let words: Vec<&str> = tokens.iter().map(|token| token.text).collect();
        let tagged = self.tags_from_words(&words)?;
        let mut cursor = 0;
        let mut chars = CharOffset::default();
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
                start: chars.upto(text, token.start),
                end: chars.upto(text, token.end),
            });
            cursor += offset + 1;
        }
        Ok(result)
    }

    /// Tag many texts, one result per input.
    pub fn tags_batch(&self, texts: &[String]) -> Result<Vec<Vec<TaggedToken>>, String> {
        texts.par_iter().map(|text| self.tags(text)).collect()
    }

    /// Entities extracted from `text` via the pack's tags.
    pub fn entities(&self, text: &str) -> Result<Vec<Entity>, String> {
        Ok(merge_entities(&self.tags(text)?))
    }

    /// Entities per text, in input order.
    pub fn entities_batch(&self, texts: &[String]) -> Result<Vec<Vec<Entity>>, String> {
        Ok(self
            .tags_batch(texts)?
            .iter()
            .map(|tokens| merge_entities(tokens))
            .collect())
    }

    /// Resolve a special token by either spelling this crate's packs use.
    ///
    /// The ALBERT packs are BERT-style (`[CLS]`/`[SEP]`); the e5 and mMARCO packs
    /// are XLM-R-style (`<s>`/`</s>`). Looking up only the BERT names made every
    /// XLM-R pack fail to load with "tokenizer has no [CLS] token".
    fn special_id(&self, bert: &str, xlm_roberta: &str) -> Option<i64> {
        self.tokenizer
            .token_to_id(bert)
            .or_else(|| self.tokenizer.token_to_id(xlm_roberta))
            .map(|id| id as i64)
    }

    /// Run the graph, supplying the all-zero segment ids a 3-input export needs.
    ///
    /// Returns the outputs the same shape `runnable.run` does. `inputs` is the
    /// usual two-tensor list; the third is zero-filled here so `sentence_vector`
    /// and the taggers do not each have to know which export they are holding.
    fn run_with_segments(
        &self,
        input_ids: ndarray::Array2<i64>,
        attention_mask: ndarray::Array2<i64>,
    ) -> Result<Vec<tract::Tensor>, String> {
        let dimensions = attention_mask.dim();
        let input_ids = input_ids
            .tract()
            .map_err(|error| format!("cannot build input tensor: {error}"))?;
        let attention_mask_typed = attention_mask
            .tract()
            .map_err(|error| format!("cannot build attention mask: {error}"))?;
        let outputs = if self.needs_token_type_ids {
            let segments = Array2::<i64>::zeros(dimensions);
            let segments = segments
                .tract()
                .map_err(|error| format!("cannot build segment ids: {error}"))?;
            self.runnable
                .run([input_ids, attention_mask_typed, segments])
                .map_err(|error| format!("inference failed: {error}"))?
        } else {
            self.runnable
                .run([input_ids, attention_mask_typed])
                .map_err(|error| format!("inference failed: {error}"))?
        };
        Ok(outputs)
    }

    fn encode_words(&self, words: &[&str]) -> Result<Option<EncodedWords>, String> {
        if words.is_empty() {
            return Ok(None);
        }

        let cls = self
            .special_id("[CLS]", "<s>")
            .ok_or_else(|| "tokenizer has no [CLS] or <s> token".to_string())?;
        let sep = self
            .special_id("[SEP]", "</s>")
            .ok_or_else(|| "tokenizer has no [SEP] or </s> token".to_string())?;
        let unk = self
            .tokenizer
            .token_to_id("<unk>")
            .or_else(|| self.tokenizer.token_to_id("[UNK]"));

        let mut ids = Vec::with_capacity(self.max_len);
        let mut positions = Vec::new();
        let mut kept = Vec::new();
        let mut visited = 0usize;
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
                    None => {
                        // Consumed but unrepresentable, so it keeps no tag.
                        visited += 1;
                        continue;
                    }
                }
            }
            if ids.len() + pieces.len() >= self.max_len {
                break;
            }
            visited += 1;
            positions.push(ids.len());
            ids.extend(pieces.into_iter().map(i64::from));
            kept.push((*word).to_owned());
        }
        if kept.is_empty() {
            return Ok(None);
        }
        ids.push(sep);
        Ok(Some((ids, positions, kept, visited)))
    }

    /// Mean-pool one text into a sentence vector, the way sentence-transformers
    /// does for an e5 pack.
    ///
    /// Pooling averages over the real tokens only; averaging over the padded
    /// sequence would change the vector with the batch it happened to land in.
    /// The result is L2-normalised so cosine similarity is a plain dot product.
    pub fn sentence_vector(&self, text: &str) -> Result<Vec<f32>, String> {
        let cls = self
            .special_id("[CLS]", "<s>")
            .ok_or_else(|| "tokenizer has no [CLS] or <s> token".to_string())?;
        let sep = self
            .special_id("[SEP]", "</s>")
            .ok_or_else(|| "tokenizer has no [SEP] or </s> token".to_string())?;

        // Encode the whole text as pieces, not per word: a sentence vector has no
        // word offsets to preserve, and the model's own template wants one
        // sequence.
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|error| format!("cannot tokenize {text:?}: {error}"))?;
        let mut ids = vec![cls];
        ids.extend(encoding.get_ids().iter().map(|&id| i64::from(id)));
        // Leave room for the separator so the sequence never exceeds `max_len`.
        if ids.len() >= self.max_len {
            ids.truncate(self.max_len - 1);
        }
        ids.push(sep);
        if ids.len() <= 2 {
            return Err(format!("{text:?} has no tokens to embed"));
        }

        let shape = (1, ids.len());
        let input_ids = Array2::from_shape_vec(shape, ids)
            .map_err(|error| format!("invalid input shape: {error}"))?;
        let attention_mask = Array2::<i64>::ones(shape);

        let outputs = self.run_with_segments(input_ids, attention_mask)?;
        let hidden = outputs
            .first()
            .ok_or_else(|| "model returned no outputs".to_string())?
            .ndarray::<f32>()
            .map_err(|error| format!("cannot read hidden states: {error}"))?;

        let sequence = hidden.shape().get(1).copied().unwrap_or(0);
        let dim = hidden.shape().get(2).copied().unwrap_or(0);
        if sequence == 0 || dim == 0 {
            return Err(format!(
                "hidden states have an unexpected shape {:?}",
                hidden.shape()
            ));
        }

        let mut pooled = vec![0f32; dim];
        for token in 0..sequence {
            for (index, value) in pooled.iter_mut().enumerate() {
                *value += hidden[[0, token, index]];
            }
        }
        let norm = pooled.iter().map(|value| value * value).sum::<f32>().sqrt();
        if !norm.is_finite() || norm <= 1e-12 {
            return Err(format!("{text:?} produced a degenerate embedding"));
        }
        for value in &mut pooled {
            *value /= norm;
        }
        Ok(pooled)
    }

    /// Word-level single call: tag pre-tokenized words, returning `(word, tag)`.
    ///
    /// One model pass holds only the words that fit in `max_len` subword pieces,
    /// so a longer input is tagged in several passes. It used to come back with
    /// tags for the first ~190 words and nothing after them, with no error.
    pub fn tags_from_words(&self, words: &[&str]) -> Result<Vec<(String, String)>, String> {
        let mut result = Vec::with_capacity(words.len());
        let mut rest = words;
        while !rest.is_empty() {
            let Some((ids, positions, kept, visited)) = self.encode_words(rest)? else {
                // A word whose own pieces do not fit cannot be tagged; skipping it
                // keeps the words after it. A single such word leaves nothing to
                // report, so it is an error rather than an empty list.
                if result.is_empty() && rest.len() == 1 {
                    return Err(format!(
                        "one word is longer than the model's max_len ({}): {:?}",
                        self.max_len, rest[0]
                    ));
                }
                rest = &rest[1..];
                continue;
            };
            result.extend(self.tag_once(ids, positions, kept)?);
            rest = &rest[visited..];
        }
        Ok(result)
    }

    fn tag_once(
        &self,
        ids: Vec<i64>,
        positions: Vec<usize>,
        kept: Vec<String>,
    ) -> Result<Vec<(String, String)>, String> {
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

        let sequence = logits.shape().get(1).copied().unwrap_or(0);
        if let Some(&position) = positions.iter().find(|&&position| position >= sequence) {
            return Err(format!(
                "logits have {} rows but need position {position}",
                logits.shape().get(1).copied().unwrap_or(0)
            ));
        }
        let label_count = logits.shape().get(2).copied().unwrap_or(0);
        check_labels("model", label_count, self.labels.len())?;

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

    /// Dependency parse of `text`; `head` is zero-based or `None` for the root.
    ///
    /// The arc decoder scores a whole sentence at once, so a text past the
    /// model's `max_len` is refused rather than parsed only up to the limit.
    pub fn depparse(&self, text: &str) -> Result<Vec<Dependency>, String> {
        let tokens = WordTokenizer::tokenize(text);
        let words: Vec<&str> = tokens.iter().map(|token| token.text).collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let Some((ids, positions, kept, visited)) = self.encode_words(&words)? else {
            // `encode_words` returns `None` when it encoded none of the words,
            // which for a non-empty input means the first word alone is longer
            // than `max_len`. That used to come back as an empty parse and exit
            // 0, while the same word in second position was a hard error.
            return Err(format!(
                "no word in the input fits the model's {}-piece limit; split the text into shorter sentences",
                self.max_len
            ));
        };
        if visited < words.len() {
            return Err(format!(
                "text is longer than the model's {}-piece limit; split it into shorter sentences",
                self.max_len
            ));
        }
        // The arc decoder holds a `words × (words + 1)` score matrix plus one edge
        // per pair, so a whole-sentence parse is quadratic: 4094 words (the most a
        // 4096-token pack allows) needs roughly 700 MB and the process dies instead
        // of answering. A real sentence is far under this.
        if kept.len() > MAX_PARSE_WORDS {
            return Err(format!(
                "text has {} words, more than the {} a dependency parse accepts; split it into sentences",
                kept.len(),
                MAX_PARSE_WORDS
            ));
        }
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
            if arcs.shape().get(2).copied() != Some(word_count + 1)
                || arcs.shape().get(1).copied() != Some(word_count)
            {
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
        let rows = relations.shape().get(1).copied().unwrap_or(0);
        if rows < kept.len() {
            return Err(format!(
                "relation logits have {rows} rows but need {}",
                kept.len()
            ));
        }
        let label_count = relations.shape().get(2).copied().unwrap_or(0);
        check_labels("model", label_count, self.labels.len())?;

        let mut cursor = 0;
        let mut chars = CharOffset::default();
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
                start: chars.upto(text, token.start),
                end: chars.upto(text, token.end),
            });
            cursor += offset + 1;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Path of a downloaded test pack, or `None` when the variable is unset.
    ///
    /// `pull_test_packs.py` exports these, and `pack-tests` in CI asserts they
    /// are set before running. A plain `return` when missing turned a mistyped
    /// variable name into a passing test, so a set-but-missing path is an error.
    fn pack_path(name: &str) -> Option<std::path::PathBuf> {
        let value = std::env::var(name).ok()?;
        let path = std::path::PathBuf::from(&value);
        assert!(
            path.exists(),
            "{name} is set to {value:?}, which does not exist"
        );
        Some(path)
    }

    /// The slow, obviously correct answer: try every subset of edges that is a
    /// legal arborescence and keep the heaviest.
    fn brute_force_optimum(node_count: usize, root: usize, edges: &[ArcEdge]) -> f64 {
        let mut best = f64::NEG_INFINITY;
        for mask in 0u32..1 << edges.len() {
            let incoming = |node: usize| {
                (0..edges.len())
                    .filter(|&index| mask >> index & 1 == 1 && edges[index].to == node)
                    .count()
            };
            if (0..node_count).any(|node| node != root && incoming(node) != 1)
                || incoming(root) != 0
            {
                continue;
            }
            let mut acyclic = true;
            for start in 0..node_count {
                if start == root {
                    continue;
                }
                let mut node = start;
                for _ in 0..node_count {
                    if node == root {
                        break;
                    }
                    let Some(index) = (0..edges.len())
                        .find(|&index| mask >> index & 1 == 1 && edges[index].to == node)
                    else {
                        acyclic = false;
                        break;
                    };
                    node = edges[index].from;
                }
                if node != root {
                    acyclic = false;
                }
                if !acyclic {
                    break;
                }
            }
            if !acyclic {
                continue;
            }
            let total: f64 = (0..edges.len())
                .filter(|&index| mask >> index & 1 == 1)
                .map(|index| edges[index].weight)
                .sum();
            if total > best {
                best = total;
            }
        }
        best
    }

    #[test]
    fn the_arborescence_matches_a_brute_force_search() {
        // The heaviest incoming edge of 1, 2 and 3 is the cycle 1 -> 2 -> 3 -> 1, so
        // the contraction and the reconstruction of the original edges both run.
        let graph = [
            (0usize, 1usize, 4.0),
            (0, 2, 4.5),
            (0, 3, 3.0),
            (1, 2, 5.0),
            (2, 3, 5.0),
            (3, 1, 5.0),
            (1, 3, 0.5),
            (3, 2, 0.25),
        ];
        let edges: Vec<ArcEdge> = graph
            .iter()
            .map(|&(from, to, weight)| ArcEdge {
                from,
                to,
                weight,
                from_original: from,
                to_original: to,
            })
            .collect();
        let parents = maximum_arborescence(4, 0, &edges).expect("every node is reachable");
        let total: f64 = (1..4)
            .map(|node| {
                edges
                    .iter()
                    .find(|edge| edge.to == node && edge.from == parents[node])
                    .expect("the parent edge exists")
                    .weight
            })
            .sum();
        assert_eq!(total, brute_force_optimum(4, 0, &edges));
    }

    fn token(text: &str, tag: &str, start: usize, end: usize) -> TaggedToken {
        TaggedToken {
            text: text.to_owned(),
            tag: tag.to_owned(),
            start,
            end,
        }
    }

    #[test]
    fn char_offsets_match_counting_from_the_start() {
        // The incremental counter replaced `text[..at].chars().count()`; every
        // boundary must still give the same answer, multi-byte text included.
        let text = "کتاب\u{200C}ها 😀 خوب، دنیا";
        let mut counter = CharOffset::default();
        for (byte, _) in text.char_indices() {
            assert_eq!(counter.upto(text, byte), text[..byte].chars().count());
        }
        assert_eq!(counter.upto(text, text.len()), text.chars().count());
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
    fn a_wrong_label_count_is_an_error_not_a_panic() {
        assert!(check_labels("model", 3, 3).is_ok());
        let error = check_labels("model", 3, 20).expect_err("a mismatch must be an error");
        assert!(error.contains("3 labels"), "{error}");
        assert!(error.contains("labels.json has 20"), "{error}");
    }

    #[test]
    fn a_max_len_that_is_not_an_integer_is_refused() {
        // `as_u64` returns `None` for `16.0`, `"16"`, `-5` and `null`, so each
        // silently became the 192 default and skipped the 3..=4096 check.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-bad-max-len");
        for value in ["16.0", "\"16\"", "-5", "null"] {
            std::fs::create_dir_all(&dir).unwrap();
            for entry in ["model.onnx", "tokenizer.json"] {
                std::fs::copy(source.join(entry), dir.join(entry)).unwrap();
            }
            std::fs::write(
                dir.join("labels.json"),
                format!(r#"{{"task":"pos","labels":["A","B","C"],"max_len":{value}}}"#),
            )
            .unwrap();
            let error = match ModelPack::load(&dir) {
                Ok(_) => panic!("max_len {value} must be refused"),
                Err(error) => error,
            };
            assert!(error.contains("max_len"), "{value}: {error}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_label_that_is_not_a_string_is_refused() {
        // A non-string label was dropped, which shifted every later class index.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-bad-label-type");
        std::fs::create_dir_all(&dir).unwrap();
        for entry in ["model.onnx", "tokenizer.json"] {
            std::fs::copy(source.join(entry), dir.join(entry)).unwrap();
        }
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"pos","labels":["A",7,"C"],"max_len":16}"#,
        )
        .unwrap();
        let error = match ModelPack::load(&dir) {
            Ok(_) => panic!("a non-string label must be refused"),
            Err(error) => error,
        };
        assert!(error.contains("labels[1]"), "{error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_pack_whose_labels_disagree_with_the_model_is_an_error() {
        // The model in the fixture emits 3 labels; declaring 20 must fail the call
        // instead of indexing a label the output does not have.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-bad-labels");
        std::fs::create_dir_all(&dir).unwrap();
        for entry in ["model.onnx", "tokenizer.json"] {
            std::fs::copy(source.join(entry), dir.join(entry)).unwrap();
        }
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"pos","labels":["A","B","C","D","E","F","G","H","I","J","K","L","M","N","O","P","Q","R","S","T"],"max_len":16}"#,
        )
        .unwrap();

        let pack = ModelPack::load(&dir).expect("the pack itself loads");
        match pack.tags("کتاب") {
            Ok(tags) => panic!("20 declared labels for a 3-label model must fail, got {tags:?}"),
            Err(error) => assert!(error.contains("labels"), "{error}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_long_input_is_tagged_in_chunks_instead_of_cut() {
        // One pass held only the words that fit in `max_len` pieces, so every
        // word past the limit came back without a tag and without an error.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-chunk-pack");
        std::fs::create_dir_all(&dir).unwrap();
        for entry in ["model.onnx", "tokenizer.json"] {
            std::fs::copy(source.join(entry), dir.join(entry)).unwrap();
        }
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"pos","labels":["A","B","C"],"max_len":8}"#,
        )
        .unwrap();

        let pack = ModelPack::load(&dir).expect("the pack itself loads");
        let words = vec!["کتاب"; 40];
        let tagged = pack.tags_from_words(&words).expect("tag the long input");
        assert_eq!(tagged.len(), words.len());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn one_over_long_word_does_not_drop_the_words_after_it() {
        // `encode_words` refuses a word whose own pieces exceed `max_len`; the
        // loop used to stop there and return tags only for the words before it.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-long-word-pack");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(source.join("model.onnx"), dir.join("model.onnx")).unwrap();
        std::fs::write(
            dir.join("tokenizer.json"),
            r###"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,"model":{"type":"WordPiece","vocab":{"[PAD]":0,"[UNK]":1,"[CLS]":2,"[SEP]":3,"a":4,"##a":5},"unk_token":"[UNK]","continuing_subword_prefix":"##","max_input_chars_per_word":100}}"###,
        )
        .unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"pos","labels":["A","B","C"],"max_len":8}"#,
        )
        .unwrap();

        let pack = ModelPack::load(&dir).expect("the pack itself loads");
        let long = "a".repeat(7);
        // The premise: this word alone needs more pieces than `max_len`.
        assert!(pack
            .encode_words(&[long.as_str()])
            .expect("encode")
            .is_none());
        let words = [long.as_str(), "a", "a", "a", "a"];
        let tagged = pack
            .tags_from_words(&words)
            .expect("the words after the long one are still tagged");
        assert_eq!(tagged.len(), 4);
        assert!(tagged.iter().all(|(word, _)| word == "a"));
        // Alone it has nothing to report, so it is an error, not an empty list.
        assert!(pack.tags_from_words(&[long.as_str()]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_first_word_that_cannot_be_encoded_is_an_error_not_an_empty_parse() {
        // `depparse` turned the `encode_words` `None` into an empty parse and exit
        // 0, while the same word in second position was a hard error.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-depparse-long-word");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(source.join("model.onnx"), dir.join("model.onnx")).unwrap();
        std::fs::write(
            dir.join("tokenizer.json"),
            r###"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,"model":{"type":"WordPiece","vocab":{"[PAD]":0,"[UNK]":1,"[CLS]":2,"[SEP]":3,"a":4,"##a":5},"unk_token":"[UNK]","continuing_subword_prefix":"##","max_input_chars_per_word":100}}"###,
        )
        .unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"depparse","labels":["root"],"max_len":8}"#,
        )
        .unwrap();

        let pack = ModelPack::load(&dir).expect("the pack itself loads");
        let long = "a".repeat(7);
        let text = format!("{long} a");
        let error = match pack.depparse(&text) {
            Ok(parse) => panic!("an unencodable first word must be an error, got {parse:?}"),
            Err(error) => error,
        };
        assert!(error.contains("no word in the input fits"), "{error}");
        // An empty input is still an empty parse, not an error.
        assert!(pack.depparse("").expect("empty input").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_parse_with_more_words_than_the_decoder_accepts_is_an_error() {
        // One model pass can encode up to `max_len` pieces, and a whole-sentence
        // parse needs a `words × (words + 1)` score matrix, so a long paragraph
        // was an out-of-memory kill instead of an error.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-depparse-word-cap");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tokenizer.json"),
            r###"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,"model":{"type":"WordPiece","vocab":{"[PAD]":0,"[UNK]":1,"[CLS]":2,"[SEP]":3,"a":4},"unk_token":"[UNK]","continuing_subword_prefix":"##","max_input_chars_per_word":100}}"###,
        )
        .unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"depparse","labels":["root"],"max_len":4096}"#,
        )
        .unwrap();
        std::fs::copy(source.join("model.onnx"), dir.join("model.onnx")).unwrap();

        let pack = ModelPack::load(&dir).expect("the pack itself loads");
        let text = "a ".repeat(MAX_PARSE_WORDS + 2);
        let error = match pack.depparse(&text) {
            Ok(parse) => panic!("{} words must be refused", parse.len()),
            Err(error) => error,
        };
        assert!(error.contains("more than the"), "{error}");
        std::fs::remove_dir_all(&dir).ok();
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
    fn a_pack_with_an_absurd_max_len_is_an_error() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-bad-maxlen");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"labels":["X"],"max_len":10000000000}"#,
        )
        .unwrap();
        assert!(ModelPack::load(&dir).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn depparse_loaded_pack() {
        let Some(path) = pack_path("RISHE_DEPPARSE_PACK") else {
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
            pack.tags("good bad")
                .unwrap()
                .into_iter()
                .map(|token| (token.text, token.tag))
                .collect::<Vec<_>>(),
            vec![
                ("good".to_string(), "A".to_string()),
                ("bad".to_string(), "B".to_string()),
            ]
        );
    }

    #[test]
    fn an_xlm_roberta_pack_resolves_its_own_special_tokens() {
        // The e5 and mMARCO packs are XLM-R-style: their specials are `<s>` and
        // `</s>`, with no `[CLS]`/`[SEP]` in the vocabulary at all. `encode_words`
        // looked up only the BERT names, so every XLM-R pack failed to load with
        // "tokenizer has no [CLS] token". Same tiny ONNX, XLM-R vocabulary.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-xlm-roberta-pack");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(source.join("model.onnx"), dir.join("model.onnx")).unwrap();
        std::fs::write(
            dir.join("tokenizer.json"),
            r###"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"Whitespace"},"post_processor":null,"decoder":null,"model":{"type":"WordPiece","vocab":{"<pad>":0,"<unk>":1,"<s>":2,"</s>":3,"a":4},"unk_token":"<unk>","continuing_subword_prefix":"##","max_input_chars_per_word":100}}"###,
        )
        .unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"pos","labels":["A","B","C"],"max_len":8}"#,
        )
        .unwrap();

        let pack = ModelPack::load(&dir).expect("an XLM-R pack loads");
        assert_eq!(pack.special_id("[CLS]", "<s>"), Some(2));
        assert_eq!(pack.special_id("[SEP]", "</s>"), Some(3));
        // The ids it builds must be the XLM-R ones, not a hardcoded BERT id.
        let (ids, _, kept, _) = pack.encode_words(&["a"]).expect("encode").unwrap();
        assert_eq!(kept, vec!["a".to_string()]);
        assert_eq!(ids.first(), Some(&2));
        assert_eq!(ids.last(), Some(&3));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Copy the tiny 3-input pack into a scratch directory named for `label`.
    ///
    /// Tests run in parallel, so the directory must be unique per test: a shared
    /// one gets deleted by whichever test finishes first and the rest panic
    /// loading a directory that no longer exists.
    fn tiny_vector_pack(label: &str) -> (ModelPack, std::path::PathBuf) {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/tiny-vector-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("rishe-tiny-vector-pack-{label}"));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["model.onnx", "tokenizer.json", "labels.json"] {
            std::fs::copy(source.join(name), dir.join(name)).unwrap();
        }
        (ModelPack::load(&dir).expect("load tiny vector pack"), dir)
    }

    #[test]
    fn a_sentence_pack_needs_no_labels() {
        // A sentence encoder returns a vector, so it has nothing to label. The
        // loader rejected an empty `labels` array for every pack, which would
        // have blocked the vector pack outright.
        let (pack, dir) = tiny_vector_pack("nolabels");
        assert_eq!(pack.task(), "sentence");
        assert!(pack.labels().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_tagging_pack_still_needs_labels() {
        // The exemption above must not leak: a pos pack with no labels would
        // report class indices that map to nothing.
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/tiny-vector-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-tiny-nolabels-pack");
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["model.onnx", "tokenizer.json"] {
            std::fs::copy(source.join(name), dir.join(name)).unwrap();
        }
        std::fs::write(
            dir.join("labels.json"),
            r#"{"task":"pos","labels":[],"max_len":16}"#,
        )
        .unwrap();
        let error = match ModelPack::load(&dir) {
            Ok(_) => panic!("a pos pack with no labels must be refused"),
            Err(error) => error,
        };
        assert!(error.contains("has no labels"), "got: {error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sentence_vector_supplies_zeroed_segment_ids() {
        // The fixture concatenates the token ids with the segment ids, so the
        // pooled vector encodes both. "a b" is ids [<s>, a, b, </s>] = [2,4,5,3]
        // and zero segments, so the mean is [3.5, 0] and normalising gives
        // [1.0, 0.0]. Segments of ones would yank the second component to ~0.27.
        let (pack, dir) = tiny_vector_pack("segments");
        let vector = pack.sentence_vector("a b").expect("sentence vector");
        assert_eq!(vector.len(), 2);
        assert!((vector[0] - 1.0).abs() < 1e-6, "got {vector:?}");
        assert!(vector[1].abs() < 1e-6, "got {vector:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sentence_vector_is_normalised() {
        let (pack, dir) = tiny_vector_pack("normalised");
        for text in ["a b", "b", "a"] {
            let vector = pack.sentence_vector(text).expect("sentence vector");
            let norm: f32 = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-6, "{text:?} has norm {norm}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sentence_vector_refuses_text_with_no_tokens() {
        let (pack, dir) = tiny_vector_pack("notokens");
        let error = match pack.sentence_vector("   ") {
            Ok(vector) => panic!("whitespace should not embed, got {vector:?}"),
            Err(error) => error,
        };
        assert!(error.contains("no tokens"), "got: {error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_two_input_pack_never_gets_a_third_tensor() {
        // The taggers feed two tensors. Adding a segment input to a 2-input
        // model would break every existing pack, so the flag is what keeps them
        // working; this pins the flag to the model's real input count.
        let (pack, dir) = tiny_vector_pack("twoinput");
        assert!(pack.needs_token_type_ids);
        std::fs::remove_dir_all(&dir).ok();

        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let tagging = ModelPack::load(&path).expect("load tiny pack");
        assert!(!tagging.needs_token_type_ids);
        // And it still tags, which is the behaviour a stray third input breaks.
        assert_eq!(tagging.tags("good bad").unwrap().len(), 2);
    }
}
