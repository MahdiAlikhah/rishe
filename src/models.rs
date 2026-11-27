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
///
/// A *merged* pack holds one shared backbone with several heads and one graph
/// that returns several outputs. Such a pack is loaded per task: the requested
/// head supplies the labels, the output index and `max_len`, so everything above
/// this struct sees an ordinary single-task pack.
#[cfg_attr(feature = "python", pyo3::pyclass(frozen))]
pub struct ModelPack {
    task: String,
    labels: Vec<String>,
    max_len: usize,
    tokenizer: ModelTokenizer,
    runnable: Runnable,
    /// Which graph output holds this task's logits.
    ///
    /// Always 0 for a single-task pack, which is every pack published before
    /// merged packs existed. A merged pack names one output per head, because
    /// `outputs.first()` would otherwise tag every task with the POS head.
    output: usize,
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
    ///
    /// This loads a single-task pack. A merged pack, which holds several heads in
    /// one graph, is loaded with [`ModelPack::load_task`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::load_task(path, None)
    }

    /// Load `task` from a pack, which may be a single-task pack or a merged one.
    ///
    /// `task` of `None` means "the pack's only task". For a merged pack it is
    /// required, because one graph holds several heads and there is no sensible
    /// default: picking one would silently tag every task with another's head.
    pub fn load_task(path: impl AsRef<Path>, task: Option<&str>) -> Result<Self, String> {
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

        // Two pack layouts share this file.
        //
        // Single-task, which every pack published before merged packs used:
        //   { "task": "pos", "labels": [...], "max_len": 192 }
        //
        // Merged, one backbone with several heads in one graph:
        //   { "pos":   { "labels": [...], "max_len": 192, "output": 0 },
        //     "ner":   { "labels": [...], "max_len": 192, "output": 1 }, ... }
        //
        // The merged form names its `output` explicitly rather than relying on key
        // order: the JSON object arrives with sorted keys, so position in the file
        // says nothing about position in the graph, and guessing would tag every
        // task with whichever head happened to sort first.
        //
        // A single-task pack always declares `task`, even the sentence pack, which
        // has no labels. Requiring both keys to be absent keeps a malformed
        // single-task pack reporting "has no labels array" instead of being
        // mistaken for a merged one and listing its own keys as heads.
        let merged = metadata.is_object()
            && metadata.get("labels").is_none()
            && metadata.get("task").is_none();
        let selected = if merged {
            let heads = metadata.as_object().expect("checked is_object above");
            let wanted = match task {
                Some(task) => task,
                None => {
                    let mut names: Vec<&str> = heads.keys().map(String::as_str).collect();
                    names.sort_unstable();
                    return Err(format!(
                        "{} is a merged pack holding several heads ({}); pass the task to load",
                        labels_path.display(),
                        names.join(", ")
                    ));
                }
            };
            heads.get(wanted).cloned().ok_or_else(|| {
                let mut names: Vec<&str> = heads.keys().map(String::as_str).collect();
                names.sort_unstable();
                format!(
                    "{} has no {wanted} head; it holds {}",
                    labels_path.display(),
                    names.join(", ")
                )
            })?
        } else {
            metadata.clone()
        };

        let mut non_string_label = None;
        let labels: Vec<String> = selected
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
            let is_sentence = selected
                .get("task")
                .and_then(|value| value.as_str())
                .is_some_and(|task| task == "sentence");
            if !is_sentence {
                return Err(format!("{} has no labels", labels_path.display()));
            }
        }
        let task = match selected.get("task").and_then(|value| value.as_str()) {
            Some(task) => task.to_owned(),
            // A merged pack's head entries need not repeat their own name; the key
            // they are stored under is the name.
            None if merged => task.unwrap_or_default().to_owned(),
            None => String::new(),
        };
        // Which graph output holds this head's logits. Absent for every
        // single-task pack, where 0 is also what `outputs.first()` returned. A
        // merged pack must state it, because its graph returns several heads and
        // there is no way to tell them apart by position alone.
        let output = match selected.get("output") {
            None if merged => {
                return Err(format!(
                    "{} has a {task} head but no output index; a merged pack must \
                     name which graph output holds each head",
                    labels_path.display()
                ))
            }
            None => 0,
            Some(value) => match value.as_u64() {
                Some(value) => value as usize,
                None => {
                    return Err(format!(
                        "invalid output {value} in {}; expected a non-negative integer",
                        labels_path.display()
                    ))
                }
            },
        };
        // `as_u64` rejects `16.0`, `"16"`, `-5` and `null` by returning `None`,
        // which then silently became the 192 default and skipped the range check.
        let max_len = match selected.get("max_len") {
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
        //
        // Only a 3-input graph gets the synthesized segment ids. The depparse
        // packs take five and pass all five themselves, so rejecting an
        // unexpected count here would refuse a pack that already works.
        let inputs = runnable
            .input_count()
            .map_err(|error| format!("cannot read input count: {error}"))?;
        // Catch a pack whose `output` index points past the end of the graph here,
        // where the pack path is still known, rather than at the first inference.
        let outputs = runnable
            .output_count()
            .map_err(|error| format!("cannot read output count: {error}"))?;
        if output >= outputs {
            return Err(format!(
                "{} asks for output {output} but {} has {outputs} output(s)",
                labels_path.display(),
                model_path.display()
            ));
        }

        Ok(Self {
            task,
            labels,
            max_len,
            tokenizer,
            runnable,
            output,
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

    /// Which output of the graph holds this pack's logits.
    ///
    /// `0` for every single-task pack. A merged pack names one output per head so
    /// each task reads its own logits out of the shared graph.
    pub fn output_index(&self) -> usize {
        self.output
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
        // A tagging pack still returns hidden states, so pooling them yields a
        // unit vector that looks like an embedding but means nothing.
        if self.task != "sentence" {
            return Err(format!(
                "pack is {}, not a sentence encoder; it cannot produce vectors",
                self.task
            ));
        }
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

    /// Sentence vectors for many texts, in input order.
    pub fn sentence_vectors(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        texts
            .par_iter()
            .map(|text| self.sentence_vector(text))
            .collect()
    }

    /// Score how well `passage` answers `query`, for a cross-encoder reranker.
    ///
    /// A cross-encoder reads both texts in one pass and returns one number, so it
    /// orders candidates but cannot produce vectors; pair it with
    /// `sentence_vector` to fetch them. Higher is more relevant. The value is a
    /// raw logit, not a probability: it is only comparable within one pack, and
    /// only the ordering is meaningful, which is what reranking needs.
    pub fn score_pairs(&self, query: &str, passage: &str) -> Result<f32, String> {
        // `encode` with `add_special_tokens: false` and explicit ids below would
        // repeat what the pair template already does; let the tokenizer apply it.
        let encoding = self
            .tokenizer
            .encode((query, passage), true)
            .map_err(|error| format!("cannot tokenize pair: {error}"))?;
        let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&id| i64::from(id)).collect();
        // Truncate the body but keep the trailing template tokens: slicing the
        // flat id list instead drops the closing separator, and the model then
        // sees an unterminated sequence. That is a real score change (~0.18 on a
        // long passage), not cosmetic.
        if ids.len() > self.max_len {
            let len = ids.len();
            let mask = encoding.get_special_tokens_mask();
            let trailing = mask
                .iter()
                .rev()
                .take_while(|&&flag| flag == 1)
                .count()
                .min(self.max_len);
            let keep = self.max_len - trailing;
            let tail = ids[len - trailing..].to_vec();
            ids.truncate(keep);
            ids.extend_from_slice(&tail);
        }
        // Both texts empty still yields the separator scaffolding, so a token
        // count is not enough: scoring it would report a real-looking number for
        // a pair with no content.
        if encoding
            .get_special_tokens_mask()
            .iter()
            .all(|&flag| flag == 1)
        {
            return Err("pair has no tokens to score".to_string());
        }

        let shape = (1, ids.len());
        let input_ids = Array2::from_shape_vec(shape, ids)
            .map_err(|error| format!("invalid input shape: {error}"))?;
        let attention_mask = Array2::<i64>::ones(shape);

        let outputs = self.run_with_segments(input_ids, attention_mask)?;
        let logits = outputs
            .first()
            .ok_or_else(|| "model returned no outputs".to_string())?
            .ndarray::<f32>()
            .map_err(|error| format!("cannot read logits: {error}"))?;

        // A reranker emits one score per pair (`[1, 1]`), or one row per class
        // (`[1, 2]`). Anything else is a tagging or encoder pack, whose output is
        // per token; returning its last value would be a plausible-looking wrong
        // number rather than an error.
        let values: Vec<f32> = logits.iter().copied().collect();
        let score = match values.as_slice() {
            [score] => *score,
            // Positive class is last in the usual Hugging Face layout.
            [_, positive] => *positive,
            _ => {
                return Err(format!(
                    "model returned {} values, not one score per pair; \
                     this pack is not a reranker",
                    values.len()
                ))
            }
        };
        if !score.is_finite() {
            return Err(format!("model returned a non-finite score: {score}"));
        }
        Ok(score)
    }

    /// Score many `(query, passage)` pairs, in input order.
    pub fn score_pairs_batch(&self, pairs: &[(String, String)]) -> Result<Vec<f32>, String> {
        pairs
            .par_iter()
            .map(|(query, passage)| self.score_pairs(query, passage))
            .collect()
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
            .map_err(|error| format!("invalid input shape: {error}"))?;
        let attention_mask = Array2::<i64>::ones(shape);
        // Through `run_with_segments`, not `runnable.run` directly: a pack exported
        // with a third input would otherwise fail here with "expected 3 got 2".
        let outputs = self.run_with_segments(input_ids, attention_mask)?;
        let logits = outputs
            .get(self.output)
            .ok_or_else(|| format!("model returned no output {}", self.output))?
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
        // Depparse graphs return arcs first and relation logits second, so this
        // reads output 1 while `self.output` (used by the taggers) stays 0. The
        // two are separate outputs of one graph, not a head selection.
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
        // A single-task pack has one output, so it reads output 0 as it always has.
        assert_eq!(pack.output_index(), 0);
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
    fn a_merged_pack_loads_one_head_at_a_time() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merged-pack");
        let pos = ModelPack::load_task(&path, Some("pos")).expect("load merged pos head");
        let ner = ModelPack::load_task(&path, Some("ner")).expect("load merged ner head");
        assert_eq!(pos.task(), "pos");
        assert_eq!(ner.task(), "ner");
        // Each head reads its own graph output.
        assert_eq!(pos.output_index(), 0);
        assert_eq!(ner.output_index(), 1);
        assert_eq!(pos.labels(), ["A", "B", "C"]);
        assert_eq!(ner.labels(), ["X", "Y"]);
        // Both tag the same words, so a head reading the wrong output shows up as
        // a label outside its own list rather than as a subtle difference.
        for token in pos.tags("good bad").expect("pos head tags") {
            assert!(pos.labels().contains(&token.tag), "{}", token.tag);
        }
        for token in ner.tags("good bad").expect("ner head tags") {
            assert!(ner.labels().contains(&token.tag), "{}", token.tag);
        }
    }

    #[test]
    fn a_single_task_pack_missing_labels_is_not_mistaken_for_a_merged_one() {
        // A merged pack has no top-level `task` or `labels`; a broken single-task
        // pack has a `task` but no `labels`. Without that distinction the second
        // was reported as "a merged pack holding several heads (task)", which
        // sends the reader looking for a heads map that was never there.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-no-labels-key-pack");
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["model.onnx", "tokenizer.json"] {
            std::fs::copy(source.join(name), dir.join(name)).unwrap();
        }
        std::fs::write(dir.join("labels.json"), r#"{"task":"pos","max_len":16}"#).unwrap();

        let error = match ModelPack::load(&dir) {
            Ok(_) => panic!("a pos pack with no labels array must be refused"),
            Err(error) => error,
        };
        assert!(error.contains("has no labels array"), "got: {error}");
        assert!(!error.contains("merged pack"), "got: {error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_merged_pack_needs_a_task() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merged-pack");
        let error = match ModelPack::load(&path) {
            Ok(pack) => panic!(
                "a merged pack must not load without a task, got {}",
                pack.task()
            ),
            Err(error) => error,
        };
        // The message names the heads so the caller knows what to ask for.
        assert!(error.contains("merged pack"), "{error}");
        assert!(error.contains("ner") && error.contains("pos"), "{error}");
    }

    #[test]
    fn a_merged_pack_rejects_an_unknown_task() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merged-pack");
        let error = match ModelPack::load_task(&path, Some("ezafe")) {
            Ok(_) => panic!("a pack with no ezafe head must not load one"),
            Err(error) => error,
        };
        assert!(error.contains("no ezafe head"), "{error}");
        assert!(error.contains("ner") && error.contains("pos"), "{error}");
    }

    #[test]
    fn a_merged_pack_requires_an_output_index() {
        // Without `output` a merged head cannot be told from any other, so the
        // pack is refused rather than silently reading the first output.
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merged-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-merged-no-output");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(source.join("model.onnx"), dir.join("model.onnx")).unwrap();
        std::fs::copy(source.join("tokenizer.json"), dir.join("tokenizer.json")).unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"pos":{"labels":["A","B","C"],"max_len":16}}"#,
        )
        .unwrap();

        let error = match ModelPack::load_task(&dir, Some("pos")) {
            Ok(_) => panic!("a merged head with no output index must not load"),
            Err(error) => error,
        };
        assert!(error.contains("no output index"), "{error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_merged_pack_rejects_an_output_past_the_graph() {
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merged-pack");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("rishe-merged-bad-output");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(source.join("model.onnx"), dir.join("model.onnx")).unwrap();
        std::fs::copy(source.join("tokenizer.json"), dir.join("tokenizer.json")).unwrap();
        std::fs::write(
            dir.join("labels.json"),
            r#"{"pos":{"labels":["A","B","C"],"max_len":16,"output":7}}"#,
        )
        .unwrap();

        let error = match ModelPack::load_task(&dir, Some("pos")) {
            Ok(_) => panic!("an output index past the graph must not load"),
            Err(error) => error,
        };
        assert!(error.contains("asks for output 7"), "{error}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_single_task_pack_ignores_the_task_argument() {
        // `Pipeline::load_pack` passes the task on every load. The published
        // single-task packs have no heads map, so the argument must not make them
        // fail: the pack's own `task` field is the authority.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let pack = ModelPack::load_task(&path, Some("pos")).expect("load tiny pack by task");
        assert_eq!(pack.task(), "pos");
        assert_eq!(pack.output_index(), 0);
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
    fn sentence_vectors_keeps_one_vector_per_text_in_order() {
        let (pack, dir) = tiny_vector_pack("batchorder");
        let texts = vec!["a b".to_string(), "a".to_string()];
        let vectors = pack.sentence_vectors(&texts).expect("sentence vectors");
        assert_eq!(vectors.len(), texts.len());
        assert_eq!(vectors[0], pack.sentence_vector("a b").unwrap());
        assert_eq!(vectors[1], pack.sentence_vector("a").unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sentence_vector_refuses_a_pack_that_is_not_an_encoder() {
        // A POS pack returns hidden states too, so pooling them produced a
        // unit-length vector that looked like an embedding and meant nothing.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let pack = ModelPack::load(&path).expect("load tiny pack");
        let error = match pack.sentence_vector("good bad") {
            Ok(vector) => panic!("a pos pack should not embed, got {vector:?}"),
            Err(error) => error,
        };
        assert!(error.contains("not a sentence encoder"), "got: {error}");
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

    #[test]
    fn score_pairs_rejects_a_pack_that_does_not_score() {
        // A tagger emits per-token logits, so its output is not one score. Taking
        // the last value would return a plausible number for a pack that cannot
        // rerank, which is worse than an error.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let pack = ModelPack::load(&path).expect("load tiny pack");
        let error = match pack.score_pairs("good", "bad") {
            Ok(score) => panic!("a per-token pack must not score, got {score}"),
            Err(error) => error,
        };
        assert!(error.contains("not a reranker"), "got: {error}");
    }

    #[test]
    fn score_pairs_batch_keeps_one_score_per_pair_in_order() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let pack = ModelPack::load(&path).expect("load tiny pack");
        // A tagger cannot score, so the batch form must fail rather than skip the
        // bad pair and return a shorter list.
        let pairs = vec![
            ("good".to_string(), "bad".to_string()),
            ("bad".to_string(), "good".to_string()),
        ];
        assert!(pack.score_pairs_batch(&pairs).is_err());
        // An empty batch is empty, not an error.
        assert_eq!(pack.score_pairs_batch(&[]).unwrap(), Vec::<f32>::new());
    }

    #[test]
    fn score_pairs_refuses_a_pair_with_no_content() {
        // The pair template wraps empty text in separators, so the encoder still
        // sees tokens and the model would return a real-looking score.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-pack");
        let pack = ModelPack::load(&path).expect("load tiny pack");
        let error = match pack.score_pairs("", "") {
            Ok(score) => panic!("an empty pair must not score, got {score}"),
            Err(error) => error,
        };
        assert!(error.contains("no tokens"), "got: {error}");
    }

    #[test]
    fn a_pack_with_more_than_three_inputs_still_loads() {
        // The depparse packs take five inputs and feed all five themselves, so
        // rejecting an unusual count at load refused a pack that already worked.
        // This is the local guard: the depparse pack test is env-gated, so
        // without this fixture the regression only shows up in CI.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-five-pack");
        let pack = ModelPack::load(&path).expect("a five-input pack must load");
        // Five inputs are not the e5 case, so no segment ids are synthesized.
        assert!(!pack.needs_token_type_ids);
    }

    #[test]
    fn a_three_input_tagging_pack_tags_through_tag_once() {
        // `tag_once` called `runnable.run([input_ids, attention_mask])` directly
        // while `sentence_vector` already went through `run_with_segments`, so a
        // pack exported with `token_type_ids` embedded fine and failed to tag
        // with "expected 3 got 2". The only 3-input fixture was a sentence pack,
        // which never reaches `tag_once`, so nothing caught it.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/tiny-three-input-pack");
        let pack = ModelPack::load(&path).expect("load three-input pack");
        assert!(pack.needs_token_type_ids, "the fixture has three inputs");
        let tags = pack
            .tags("good bad")
            .expect("a three-input pack must tag through tag_once");
        assert_eq!(tags.len(), 2, "got {tags:?}");
    }
}
