# Releasing Rishe

A checklist for cutting a release. It deliberately contains no version-specific
hashes or paths, so it stays valid for every future version.

The project has a single author and contributor: **Mahdi Alikhah**. Every
commit, tag and release must be authored by him alone. Do not add co-author
trailers or list anyone else as author/contributor.

## Versioning

- `0.x` follows SemVer in spirit: **patch** for fixes, docs and metadata,
  **minor** for features or any public API change, `1.0.0` when the API is
  frozen.
- Model packs are versioned separately from the crate. A new model does not
  require a crate release unless the loader or CLI changes.
- Two kinds of model artifact exist:
  - **single-file models** (`Embeddings`, e.g. `rishe-embed-fa-300-v2`) are
    registered in `MODELS` in `src/hub.rs` and `python/rishe/hub.py` with a
    pinned SHA-256. The file may be word2vec text (`.vec`) or Rishe's compact
    binary (`.rvec`); the hub entry points at whichever file the default
    download should be;
  - **task packs** (POS/NER/ezafe/depparse) are described by the HF
    `manifest.json`, whose hash is pinned as `PACK_MANIFEST_SHA256` in the same
    two files.
  Changing a pinned hash requires a crate release, so batch model changes with
  a code release when possible.

## Pre-flight

Run the same gates CI runs, locally:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --features models
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --features python,models
python -m unittest discover -s python/tests -p "test_*.py" -v
python scripts/benchmark_golden.py
```

If the optional model packs are available locally, run the gated tests too:

```bash
RISHE_DEPPARSE_PACK=/path/to/depparse \
RISHE_EMBEDDINGS_PACK=/path/to/rishe-embed-fa-300-v2.rvec \
cargo test --features models

RISHE_TEST_DEPPARSE_PACK=/path/to/depparse \
RISHE_TEST_EMBEDDINGS_PACK=/path/to/rishe-embed-fa-300-v2.rvec \
python -m unittest discover -s python/tests -p "test_*.py" -v
```

## Cut the release

1. **Bump the version** in `Cargo.toml`, then sync the lockfile:

   ```bash
   cargo update -p rishe --offline
   ```

2. **Add a `CHANGELOG.md` entry** under a new `## [x.y.z] - YYYY-MM-DD`
   heading. Group changes under Added / Changed / Fixed / Removed.

3. **Stage explicitly.** Do not use `git add -A`:

   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md README*.md NOTICE.md .gitignore \
     src python tests docs examples scripts
   git status --short
   ```

   `training/` is excluded locally (`.git/info/exclude`) and must stay out.
   `target/`, `dist/` and generated `.vec`/`.zip` files are ignored.

4. **Commit** with a release message, for example:

   ```bash
   git commit -m "Release x.y.z" -m "One-paragraph summary of the release."
   ```

5. **Tag and push.** The tag triggers the release workflow, which publishes to
   PyPI and crates.io:

   ```bash
   git tag -a vx.y.z -m "Release x.y.z"
   git push origin main
   git push origin vx.y.z
   ```

6. **Watch CI** until every job is green:

   ```bash
   gh run list --limit 2
   gh run view <run-id>
   ```

7. **Verify the published artifacts:**

   ```bash
   curl -s https://pypi.org/pypi/rishe/x.y.z/json | python -m json.tool | head
   curl -s -A "rishe-release-check" https://crates.io/api/v1/crates/rishe/x.y.z
   ```

   Then create the GitHub release with notes from the changelog:

   ```bash
   gh release create vx.y.z --title "Rishe x.y.z" --notes-file notes.md
   ```

## Model artifacts

The model repository is `MahdiAlikhah/rishe-models`. Large files are stored with
Git LFS; `.vec` is already tracked in `.gitattributes`, and a `.rvec` payload is
added to it by the Hub client on the first upload (check the commit diff and add
the pattern by hand if it is missing).

When publishing or refreshing a pack:

1. Build the pack directory: payload (`.vec` or `.onnx`), `MODEL_CARD.md`,
   `NOTICE.md`, `LICENSE`, `metrics.json`, `manifest.json`, `SHA256SUMS`, and
   the zip. For a full-precision pack that also ships a compact binary, generate
   the second payload from the first, so both hold the same matrix:

   ```bash
   rishe convert pack.vec pack.rvec --dtype f16
   ```

2. The manifest lists every file except itself and `SHA256SUMS`, and records the
   default file plus the format of each payload.
3. Upload the directory (full upload on first publish; when only metadata
   changed, exclude the payload so the pinned file is untouched), then confirm
   the LFS OID equals the payload's SHA-256:

   ```bash
   # first publish
   hf upload MahdiAlikhah/rishe-models path/to/pack embeddings
   # metadata-only refresh (payload unchanged)
   hf upload MahdiAlikhah/rishe-models path/to/pack embeddings \
     --exclude "*.vec" --exclude "*.rvec"

   curl -sI "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/embeddings/<file>.rvec"
   ```

4. Never modify a published payload file: the hub pins its SHA-256. Metadata
   (card, license, manifest, checksums) can be revised without a crate release;
   payload changes require a version bump plus a code release for the pin.

Network tip: the system proxy may throttle uploads. Bypassing it for HF
(`NO_PROXY=huggingface.co,.huggingface.co,hf.co,.hf.co`) and disabling Xet
(`HF_HUB_DISABLE_XET=1`) is faster and more reliable on this machine.

## If a release fails

- Windows, `LNK1114: cannot overwrite the original file …rishe.dll.lib` (error
  code 5) when building the extension: a stale or partially linked artifact is
  locked in `target\debug\deps\`. Delete `rishe.dll`, `rishe.dll.exp` and
  `rishe.dll.lib` there and rebuild. It is a file-lock, not a code problem.
- Before the registry publish succeeds: fix the problem, amend or add a commit,
  then move the tag (`git tag -d`, delete the remote tag, re-tag) and push
  again.
- After PyPI or crates.io has the version: never move the tag. Cut a new patch
  release instead.

## Checklist

- [ ] version bumped in `Cargo.toml` and `Cargo.lock`
- [ ] `CHANGELOG.md` entry added
- [ ] fmt, clippy, tests (default + models), Python suite, golden benchmark green
- [ ] working tree contains only intended files
- [ ] commit and tag authored solely by Mahdi Alikhah
- [ ] CI green on the tag
- [ ] PyPI, crates.io and GitHub release verified
- [ ] model artifacts uploaded and checksums verified (if changed)
