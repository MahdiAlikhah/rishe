# Errors and exit codes

The `rishe` command ships as the Rust binary (`cargo install rishe`) and as the
`rishe` console script from the Python package. This page is the error contract
both follow: three exit statuses, a fixed split between stdout and stderr, the
JSON-lines error envelope for `serve`, and the exception types the Python API
raises. Error message wording may change; the statuses, the stream split and the
shapes are the contract.

## Exit codes

| code | meaning | when |
|---|---|---|
| `0` | success | the command finished; also `-h`/`--help`, `-V`/`--version`, the usage print when no command is given, and `serve` reaching end of input |
| `2` | usage error | the argument parser rejects the command line: unknown command, unknown option or flag, missing option value, wrong number of positional arguments. Nothing that has a valid command line returns 2. |
| `1` | runtime failure | the command line parsed, but the operation failed: everything from a missing file to a failed download |

`2` covers only parser-level rejections. A value the parser accepts stays on the
command line and so belongs to `1` when it turns out to be unusable â€” an
out-of-vocabulary word, an unknown masker name, a `--top` below 1, a pack that is
not there, an unknown Hub name.

### Status 0

```bash
rishe normalize "Ù…ÙŠ ÙƒÙ†Ù…"          # stdout: Ù…ÛŒâ€ŒÚ©Ù†Ù…
rishe --help                      # stdout: usage
rishe                             # stdout: usage
rishe -V                          # stdout: rishe 0.11.2
rishe hub info rishe-embed-fa-300 # stdout: rishe-embed-fa-300<TAB>missing
```

`hub info` is a report, not a check: a known name that is not cached prints
`missing` and exits 0. `hub verify` is the check, and exits 1 when the files are
missing or corrupt (see below).

`serve` exits 0 when stdin reaches EOF, even if requests on the way failed. The
per-line envelopes are described in [serve-schema.md](serve-schema.md).

### Status 2

```bash
rishe frobnicate
# stderr: unknown command: frobnicate   (followed by the usage text)

rishe tokenize text --frobnicate
# stderr: unknown option: --frobnicate

rishe tokenize text --model
# stderr: --model needs a value

rishe convert
# stderr: convert needs an input and an output path

rishe hub
# stderr: usage: rishe hub <pull|list|info|path|remove|verify> ...

rishe hub path
# stderr: hub path needs a name
```

### Status 1

```bash
rishe date "not a date"
# stderr: cannot parse a Jalali date from "not a date"

rishe similar Ú©ØªØ§Ø¨ --model missing.vec
# stderr: embedding file not found: missing.vec

rishe similar Ù†Ø§Ù…ÙˆØ¬ÙˆØ¯ --model model.vec
# stderr: "Ù†Ø§Ù…ÙˆØ¬ÙˆØ¯" is not in the vocabulary

rishe similar Ú©ØªØ§Ø¨ --top 0 --model model.vec
# stderr: --top must be at least 1

rishe mask text --maskers bogus
# stderr: unknown masker: bogus

rishe pos text
# stderr: this rishe binary was built without the `models` feature; ...
#         (with the `models` feature and no pack: pos needs --pack <dir>)

rishe hub path rishe-embed-fa-300    # when it has not been pulled
# stderr: "rishe-embed-fa-300" is not cached; run `rishe hub pull ...`

rishe hub path nope-model
# stderr: unknown model "nope-model"; known models: ...

rishe hub verify rishe-embed-fa-300  # when the files are missing or corrupt
# stdout: invalid
# stderr: (empty)

rishe serve --model missing.vec
# stderr: embedding file not found: missing.vec
#         (startup failure: no response lines are written)

rishe hub pull rishe-embed-fa-300-v2 # when the download or checksum fails
# stderr: download failed for <url>: ...  /  checksum mismatch for <url>: ...
```

`hub verify` is the one command that reports a failure on stdout (`invalid`)
instead of stderr: the answer is the verification result, and the status 1 marks
it as a failed check. Missing files, an unreadable cache and an unknown name
still report on stderr.

## stdout and stderr

- Result data goes to stdout. Diagnostics go to stderr.
- A usage error writes its message (and for an unknown command, the usage text)
  to stderr and exits 2; it writes nothing to stdout.
- A runtime failure writes one line to stderr and exits 1; it writes nothing to
  stdout before failing. The only exception is `hub verify` printing `invalid`,
  described above.
- `-h`/`--help`, `-V`/`--version` and the no-command usage print write to stdout
  and exit 0. `--version` prints `rishe` and the version, for example
  `rishe 0.11.2`.
- `serve` writes response envelopes to stdout and startup diagnostics to stderr.
  Requests that fail do not change the process exit status.

## The serve error envelope

A request line that cannot be served gets one envelope and the server keeps
running:

```json
{"ok":false,"error":"unknown command: nope"}
```

Exactly one response line is written per non-blank request line, in order. The
process exits 0 at EOF. Startup failures are different: the server writes the
message to stderr, writes no response line, and exits 2 for a bad option or 1 for
a pack or embedding model that cannot be loaded. The full protocol, including
every per-command failure message, is in [serve-schema.md](serve-schema.md).

## Python API exceptions

The Python API reports problems as exceptions, not exit codes. Measured with the
local build (CPython 3.13.15, Windows x86_64, `.venv\Scripts\python.exe`):

| operation | exception | example |
|---|---|---|
| wrong argument type | `TypeError` | `rishe.normalize(123)` â†’ `argument 'text': 'int' object cannot be converted to 'PyString'` |
| invalid value | `ValueError` | `rishe.mask("x", ["bogus"])` â†’ `unknown masker: bogus` |
| missing embedding file | `OSError` | `rishe.Embeddings("missing.vec")` â†’ `cannot open ...: The system cannot find the file specified. (os error 2)` |
| corrupt embedding file | `OSError` | `rishe.Embeddings(bad)` â†’ `expected a '<vocab> <dim>' header on the first line of the .vec file` |
| empty or malformed embedding | `OSError` | `empty embedding file`, `embedding dimension must be positive` |
| Hub name not registered | `ValueError` | `hub.path("unknown-model")` â†’ `unknown model or pack 'unknown-model'; models: ...; packs: ...` |
| Hub name registered but not cached | `FileNotFoundError` | `hub.path("rishe-embed-fa-300")` â†’ `'rishe-embed-fa-300' is not cached; run: rishe hub pull ...` |
| pack load failure | `ValueError` | `Pipeline(pos_pack="missing-dir")` â†’ `model pack is not a directory: ...`; a missing component â†’ `labels.json is missing from ...` |
| word out of vocabulary | no exception | `Embeddings.similarity(a, b)` returns `None`; `most_similar` returns `[]`; `vector` returns `None` |
| unparseable date | no exception | `rishe.parse_date("not a date")` returns `None` |

`FileNotFoundError` is a subclass of `OSError`, so a caller that catches
`OSError` covers both cache-related failures. The exact message text is not part
of the contract; the exception type is.
