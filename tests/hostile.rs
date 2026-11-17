//! Malformed input must return an error or finish, never panic or hang.
//!
//! `src/embeddings.rs` already covers corrupt `.vec` and `.rvec` files; this
//! file covers the model pack and the oversized single token.

#[cfg(feature = "models")]
use std::path::{Path, PathBuf};

#[cfg(feature = "models")]
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-scratch")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&path).ok();
    std::fs::create_dir_all(&path).expect("create scratch dir");
    path
}

#[test]
fn a_ten_megabyte_token_finishes() {
    let text = "ا".repeat(5 * 1024 * 1024); // 10 MB as UTF-8
    let doc = rishe::Pipeline::create(true, true, true).process(&text);
    assert!(!doc.tokens.is_empty());
}

#[test]
fn a_long_suffixed_token_finishes() {
    // `to_formal` recurses once per stripped noun suffix, so this used to
    // overflow the stack and abort the process at about 5000 repetitions.
    let text = "کتاب".to_string() + &"مون".repeat(20_000);
    let doc = rishe::Pipeline::create(true, true, true).process(&text);
    assert!(!doc.tokens.is_empty());
}

#[test]
fn unpacking_a_wild_lemma_returns_the_key() {
    // `unpack_lemma` is a safe `pub fn` taking a caller-supplied u64. A wild
    // offset used to panic on the slice; an offset splitting a character was
    // UB through `from_utf8_unchecked`. Both must fall back to the key.
    use rishe::morphology::MorphologyEngine;

    for packed in [
        0x007F_FFFF_FF00_0001, // offset far past the blob
        0x0000_0000_00FF_FFFF, // length far past the blob
        0x0000_0000_0000_0001, // offset 0, one byte: splits a character
        0x0000_0000_0000_0003, // offset 0, three bytes: splits a character
        u64::MAX,              // both fields saturated
    ] {
        let lemma = MorphologyEngine::unpack_lemma(packed, "کلید");
        assert_eq!(lemma, "کلید", "packed {packed:#x} did not fall back");
    }
}

#[cfg(feature = "models")]
#[test]
fn a_corrupt_model_pack_returns_an_error() {
    use rishe::models::ModelPack;

    let dir = scratch("hostile-pack");
    assert!(ModelPack::load(&dir).is_err(), "empty pack dir must fail");

    std::fs::write(dir.join("labels.json"), b"{").unwrap();
    std::fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
    std::fs::write(dir.join("model.onnx"), b"\x00\x01").unwrap();
    assert!(
        ModelPack::load(&dir).is_err(),
        "a pack with a malformed labels.json must fail"
    );

    std::fs::remove_dir_all(&dir).ok();
}
