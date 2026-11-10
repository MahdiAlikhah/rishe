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

# MSRV: the default feature set must build on the highest `rust-version`
# declared anywhere in that feature graph (currently 1.88).
rustup toolchain install 1.88.0 --profile minimal
cargo +1.88.0 check --locked
cargo +1.88.0 check --locked --features python

# `models` (and with it the Python wheel) needs 1.91: `tract` declares it.
rustup toolchain install 1.91.0 --profile minimal
cargo +1.91.0 check --locked --features python,models

# Every declared feature combination must compile on its own.
cargo check --locked --no-default-features
cargo check --locked --features python
cargo check --locked --features models
cargo check --locked --features python,models

# Public docs must be warning-free.
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --features python,models

# The public surface is a committed snapshot; regenerate it when a public item
# is added, renamed or removed, and never edit the file by hand.
python scripts/api_surface.py
python scripts/api_surface.py --check

python -m unittest discover -s python/tests -p "test_*.py" -v
python scripts/benchmark_golden.py

# The examples CI runs are the ones that need no model pack and no network.
PYTHONIOENCODING=utf-8 python examples/basic.py
PYTHONIOENCODING=utf-8 python examples/masking_keywords.py
PYTHONIOENCODING=utf-8 python scripts/check_docs_examples.py
```

CI runs the Python suite on CPython 3.9 through 3.14, so check the oldest
interpreter you have (`py -3.9` on Windows, `python3.9` elsewhere) before
tagging:

```bash
py -3.9 -m unittest discover -s python/tests -p "test_*.py" -v
```

The MSRV gate is a real `cargo check --locked` on Rust 1.88: that is the highest
`rust-version` declared by anything in the default-feature dependency graph
today (`ureq` → `url` → `idna` → `icu_normalizer`). The `models` feature is a
separate floor: `tract` declares 1.91, and the Python wheel enables it, so a
source install of the Python package needs 1.91 even though the default crate
does not. `Cargo.lock` is in lockfile format 4, which no cargo older than 1.78
can even read, so a lower `--locked` gate cannot work without rewriting the
lock. Raising a gate means bumping `Cargo.toml` `rust-version` together with
`README.md`, `README.crates.md`, `README.pypi.md` and `docs/STABILITY.md` in the
same commit. `examples/embeddings.py` loads the
`rishe-embed-fa-300-v2` pack from the hub cache, so it is deliberately not part
of the offline gate — run it by hand once the cache is populated.

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

6. **Watch CI** until every job is green: `rust` (three OSes), `rust models`,
   `msrv`, `features`, `docs`, `examples`, `python tests` (3.9 – 3.14), and
   the wheel and sdist artifacts. The two publish jobs are tag-gated and run
   after all of the gates above:

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

### Hub manifest schema

The registry manifest (`manifest.json`, pinned by `PACK_MANIFEST_SHA256` in
`src/hub.rs` and `python/rishe/hub.py`) is versioned independently of the pack
files. Both readers treat a missing `"schema"` field as schema 1 (the only
published version so far) and refuse another major version, or a non-integer,
with an error that names the found and the supported version. Write
`"schema": 1` when publishing a pack. The field is optional, so clients older
than the reader that introduced it keep working.

Bump the schema only for a breaking layout change, and in the same release update
both readers, their schema tests and the pinned manifest hash. Adding optional
fields inside schema 1 is backward compatible and needs no version bump.

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
- [ ] MSRV gate green (`cargo +1.88.0 check --locked`, `--features python`)
- [ ] `models` gate green (`cargo +1.91.0 check --locked --features python,models`)
- [ ] feature matrix green (`--no-default-features`, `python`, `models`, `python,models`)
- [ ] `cargo doc` warning-free under `RUSTDOCFLAGS="-D warnings"`
- [ ] API surface unchanged, or `docs/api-surface.txt` regenerated with
      `python scripts/api_surface.py`
- [ ] offline examples (`basic.py`, `masking_keywords.py`) run
- [ ] README quick-start blocks run (`python scripts/check_docs_examples.py`)
- [ ] Python suite green on 3.9 – 3.14
- [ ] working tree contains only intended files
- [ ] commit and tag authored solely by Mahdi Alikhah
- [ ] CI green on the tag
- [ ] PyPI, crates.io and GitHub release verified
- [ ] model artifacts uploaded and checksums verified (if changed)
