use fst::MapBuilder;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;

const FLAG_IDENTITY: u64 = 1 << 55;
const DEFAULT_VERB_FLAG: u8 = 1 << 7; // 0x80: Verb stem

struct StringTableBuilder {
    blob: Vec<u8>,
    cache: HashMap<String, (u64, u64)>,
}

impl StringTableBuilder {
    fn new() -> Self {
        Self {
            blob: Vec::with_capacity(256 * 1024),
            cache: HashMap::new(),
        }
    }

    fn intern(&mut self, s: &str) -> (u64, u64) {
        if let Some(&(offset, len)) = self.cache.get(s) {
            return (offset, len);
        }
        let offset = self.blob.len() as u64;
        let len = s.len() as u64;
        self.blob.extend_from_slice(s.as_bytes());
        self.cache.insert(s.to_owned(), (offset, len));
        (offset, len)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=data/nouns.tsv");
    println!("cargo:rerun-if-changed=data/verbs.tsv");
    println!("cargo:rerun-if-changed=data/informal_words.tsv");
    println!("cargo:rerun-if-changed=data/informal_verbs.tsv");

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir);

    let mut string_table = StringTableBuilder::new();

    compile_tsv_to_fst(
        Path::new("data/nouns.tsv"),
        &out_path.join("nouns.fst"),
        &mut string_table,
        true,
    );

    compile_tsv_to_fst(
        Path::new("data/verbs.tsv"),
        &out_path.join("verbs.fst"),
        &mut string_table,
        false,
    );

    compile_tsv_to_fst(
        Path::new("data/informal_words.tsv"),
        &out_path.join("informal_words.fst"),
        &mut string_table,
        false,
    );

    compile_tsv_to_fst(
        Path::new("data/informal_verbs.tsv"),
        &out_path.join("informal_verbs.fst"),
        &mut string_table,
        false,
    );

    let blob_path = out_path.join("lemmas_blob.bin");
    fs::write(&blob_path, &string_table.blob).expect("Failed to write lemmas_blob.bin");
}

fn compile_tsv_to_fst(
    tsv_path: &Path,
    fst_out_path: &Path,
    string_table: &mut StringTableBuilder,
    use_identity_sentinel: bool,
) {
    let mut entries: BTreeMap<String, u64> = BTreeMap::new();

    let file = File::open(tsv_path).unwrap_or_else(|e| {
        panic!(
            "cannot open {}: {e} (data/ must ship with the crate)",
            tsv_path.display()
        )
    });
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line.expect("Invalid UTF-8 in TSV");
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').map(|s| s.trim()).collect();
        let key = parts[0];
        let lemma = parts.get(1).copied().unwrap_or(key);
        let pos_flags: u8 =
            parts
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(if use_identity_sentinel {
                    1
                } else {
                    DEFAULT_VERB_FLAG
                });

        if key.is_empty() {
            continue;
        }

        let packed_val: u64 = if use_identity_sentinel && key == lemma {
            ((pos_flags as u64) << 56) | FLAG_IDENTITY
        } else {
            let (offset, len) = string_table.intern(lemma);
            ((pos_flags as u64) << 56) | ((offset & 0x7FFF_FFFF) << 24) | (len & 0x00FF_FFFF)
        };

        entries.insert(key.to_owned(), packed_val);
    }

    let wtr = BufWriter::new(File::create(fst_out_path).expect("Failed to create FST file"));
    let mut builder = MapBuilder::new(wtr).expect("Failed to init MapBuilder");
    for (key, val) in entries {
        builder
            .insert(key.as_bytes(), val)
            .expect("FST insert failed");
    }
    builder.finish().expect("Failed to finish FST");
}
