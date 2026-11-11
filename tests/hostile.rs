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
