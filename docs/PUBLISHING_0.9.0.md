# Publishing Rishe 0.9.0

Manual checklist. None of these steps are automated by the repository.

> **Status (2026-11-05):** the `rishe-embed-fa-300` pack is uploaded to
> `MahdiAlikhah/rishe-models` under `embeddings/` (commit `3c1c998`), the repo
> README documents it, and `*.vec` is LFS-tracked. The remaining steps below are
> the release commit/tag, which has not been executed.

The project has a single author and contributor: **Mahdi Alikhah**. Do not add
co-author trailers or list anyone else as author/contributor in commits,
releases, model cards or package metadata.

## 1. Upload the model to Hugging Face

Target repo: `MahdiAlikhah/rishe-models` (public).

| | |
|---|---|
| local file | `training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/rishe-embed-fa-300.vec` |
| repo path | `embeddings/rishe-embed-fa-300.vec` |
| size | 135,072,816 bytes |
| SHA-256 | `afed6aa266e07745ce7b6b1e2f6600f6f0373dc91e197902d7305e19d2d35bf5` |

```bash
hf auth whoami          # must print MahdiAlikhah
hf upload MahdiAlikhah/rishe-models \
  "training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/rishe-embed-fa-300.vec" \
  embeddings/rishe-embed-fa-300.vec

# Pack metadata, for discoverability on the model page.
hf upload MahdiAlikhah/rishe-models \
  "training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/MODEL_CARD.md" \
  embeddings/MODEL_CARD.md
hf upload MahdiAlikhah/rishe-models \
  "training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/NOTICE.md" \
  embeddings/NOTICE.md
hf upload MahdiAlikhah/rishe-models \
  "training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/manifest.json" \
  embeddings/manifest.json
hf upload MahdiAlikhah/rishe-models \
  "training/result/rishe-0.9.0/artifacts/rishe-embed-fa-300/SHA256SUMS" \
  embeddings/SHA256SUMS
```

`src/hub.rs` and `python/rishe/hub.py` already point at
`embeddings/rishe-embed-fa-300.vec` and pin the SHA-256 above. Do not change the
file after uploading, or the hub entry will fail verification.

Verify the download works before tagging:

```bash
curl -sIL "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/embeddings/rishe-embed-fa-300.vec" | head
export RISHE_HOME="$(mktemp -d)"
rishe hub pull rishe-embed-fa-300
rishe hub verify rishe-embed-fa-300
```

## 2. Check the working tree

```bash
cargo test --features models
cargo test                       # default features
pip install -e .                 # editable install of the Python package
python -m unittest discover -s python/tests -p "test_*.py" -v
git status --short
```

Only tracked, 0.9.0-relevant files should be staged. `training/` is currently
excluded via `.git/info/exclude` and stays local; the large model artifacts must
never be committed.

## 3. Commit and tag

```bash
git add Cargo.toml Cargo.lock src python tests docs examples scripts README*.md NOTICE.md .gitignore
git commit -m "Release 0.9.0 with Persian embeddings"
git tag -a v0.9.0 -m "Release 0.9.0"
git push origin main
git push origin v0.9.0
```

The `v0.9.0` tag runs `.github/workflows/release.yml`, which publishes the
Python wheels to PyPI and the crate to crates.io. A tag push is the publishing
step: do it only after the model upload and the test runs above.

## 4. Optional: 135k coverage variant

`rishe-embed-fa-300-135k` is exploratory and deliberately not in `hub`. If you
want it available, upload the zip as a release asset rather than registering it
as the default model:

```bash
gh release upload v0.9.0 \
  "training/result/rishe-0.9.0/artifacts-135k/rishe-embed-fa-300.zip"
```

## 5. What must stay out of the repository

- `training/result/**/artifacts*/` (the `.vec`, zips, generated cards)
- `training/**/*.vec`, `*.model`, `sentences.txt`
- `mini-embed.vec` and `data/*.pdf`
