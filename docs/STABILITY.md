# Stability and versioning

What the Rishe version number means, which parts of the project it covers, and
what can change without one. This is the contract a release is tested against,
not a list of things that are coming.

## What the version number covers

Rishe is a Rust crate and a Python package released together, from the same
source and the same tag. A version number covers all of these surfaces:

| surface | where it lives |
|---|---|
| Python API | `rishe`, `rishe.api`, `rishe.compat`, `rishe.hub`, `rishe.corpora`, `rishe.dates` |
| Rust API | the `rishe` crate's public items, including `Pipeline`, `Embeddings`, `ModelPack`, `hub` |
| Command line | the `rishe` binary (`cargo install`) and the `rishe` console script (pip) |
| Serve protocol | the JSON-lines protocol in [`serve-schema.md`](serve-schema.md) |
| Model and pack formats | word2vec `.vec` text, `.rvec` version 1, hub `manifest.json` schema 1 |
| Published pack names | every name the Hub resolves, under its own version |

The code license does not cover model weights. Weights are versioned separately:
a pack is identified by its name and its checksum, and a new pack is a new name.

## What a version number allows

| change | version |
|---|---|
| fix a bug without changing documented behaviour | patch (`x.y.Z`) |
| add a method, keyword argument, command, response field, Cargo feature, pack or doc | minor (`x.Y.0`) |
| add a deprecated alias (it keeps working) | minor |
| change a default (for example the default Hub pack), with a CHANGELOG note; every published name keeps resolving | minor |
| change performance, error message wording, logging or internal structure | any release |
| remove or rename a public name, change a return type, change documented behaviour, remove a CLI flag, remove a Python or Rust version, change `.rvec` or manifest semantics | major (`X.0.0`) only |

Before `1.0.0` the minor numbers are still allowed to break: `0.11.0` was the last
release that renamed public names. From `1.0.0` the table above applies as
written.

## Deprecated names

- A deprecated name keeps working and warns (`DeprecationWarning` in Python,
  `#[deprecated]` in Rust, a note in the CLI help).
- The aliases introduced before `1.0.0` are removed in `1.0.0`; they are listed
  in the CHANGELOG and in [`API_CONVENTIONS.md`](API_CONVENTIONS.md).
- After `1.0.0`, a name deprecated in a minor is not removed before the next
  major version.

Calling a deprecated name is never an error, and it never changes the result it
returned before.

## What is not covered

- **Model quality and benchmark ranking.** A version number says which code and
  which formats you get, not how the weights score. Evaluation numbers are
  reported per pack, with the method, in [`MODEL_CARDS.md`](MODEL_CARDS.md) and
  [`MODEL_BENCHMARKS.md`](MODEL_BENCHMARKS.md).
- **The default pack.** It may change in a minor with a CHANGELOG note. Pinning a
  pack name, or a pack checksum, is the way to keep a result fixed.
- **Absolute performance.** Latency and memory numbers are measurements on a
  documented host, not a guarantee.
- **Future releases.** The repository describes what shipped. It does not
  publish a schedule, a feature roadmap or dates.

## Support matrix

| surface | supported |
|---|---|
| Python | CPython 3.9 – 3.13, abi3 wheel, no runtime dependencies |
| Rust | MSRV 1.88 for the default feature set (driven by the locked dependency graph); the `models` feature needs Rust 1.91 |
| Platforms | Linux x86_64/aarch64 (glibc and musl), macOS x86_64/arm64, Windows x86_64 |
| Command line | the commands documented in [`cli.md`](cli.md) |

Dropping a platform, a Python version or the MSRV is a major-version change.

## Failure behaviour

Exit codes, exception types and the serve error shape are part of the surface:
see [`errors.md`](errors.md). A malformed input produces an error, never a panic,
and the CLI never writes an error to stdout.

## Formats

`.rvec` version 1 and hub manifest schema 1 are read forever once published: a
future reader accepts them even after the writer moves on. The version fields are
checked, and an unknown major version is refused with an error that names it,
rather than being guessed at.
