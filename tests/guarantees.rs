//! The concurrency and determinism promises in `docs/STABILITY.md`, as tests.
//!
//! These only assert; they exist so the paragraph cannot quietly become false.

use std::sync::Arc;

use rishe::embeddings::Embeddings;
use rishe::Pipeline;

const TEXT: &str = "من به کتابخانه می‌روم و کتاب می‌خوانم.";

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<Pipeline>();
    assert_send_sync::<Embeddings>();
    #[cfg(feature = "models")]
    assert_send_sync::<rishe::models::ModelPack>();
}

#[test]
fn concurrent_processing_matches_one_thread() {
    let pipeline = Arc::new(Pipeline::create(true, true, true));
    let expected = pipeline.process(TEXT).tokens;

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let pipeline = Arc::clone(&pipeline);
            std::thread::spawn(move || pipeline.process(TEXT).tokens)
        })
        .collect();

    for handle in handles {
        assert_eq!(handle.join().expect("worker panicked"), expected);
    }
}

#[test]
fn processing_is_deterministic_across_instances() {
    let first = Pipeline::create(true, true, true).process(TEXT);
    let second = Pipeline::create(true, true, true).process(TEXT);

    assert_eq!(first.tokens, second.tokens);
    assert_eq!(first.lemmas, second.lemmas);
    assert_eq!(first.sentences, second.sentences);
    assert_eq!(first.ezafe_markers, second.ezafe_markers);
}
