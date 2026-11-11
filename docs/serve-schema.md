# Serve protocol, version 1

`rishe serve` is a JSON-lines server: one request per line on stdin, one response
per line on stdout. This page documents protocol version 1 for both
implementations: the Rust binary (`cargo install rishe` or
`cargo run --bin rishe`) and the console script installed by the Python package
(`pip install rishe`). Both implement the same protocol, so a client can target
either one; JSON whitespace, object key order and error message wording are not
part of it (see [Envelope](#envelope)).

## Process model

- Start the process once and keep it alive: the `Pipeline`, the masker list and
  any model packs or embedding model passed on the command line are loaded once,
  before the first request, and reused for every line.
- Write one JSON object per line to stdin. A blank line (only whitespace) is
  ignored and gets no response.
- Every other line gets exactly one JSON object on stdout, in order. The server
  flushes after each response, so a client can pair lines one to one.
- A malformed line or a failed command produces the error envelope for that line
  only. The server keeps reading and answering; per-request errors never change
  the exit status. A line that is not valid UTF-8 is a malformed line: it gets
  the error envelope and the server keeps serving.
- On end of input (EOF) the server exits with status 0, including after lines
  that produced errors.

Process options are read once at startup:

| option | effect |
|---|---|
| `--maskers a,b` | default maskers for `mask` requests (`--stages` is a deprecated alias) |
| `--mask-token TEXT` | default replacement token for `mask` requests |
| `--pos-pack DIR` | POS model pack, loaded at startup |
| `--ner-pack DIR` | NER model pack, loaded at startup |
| `--ezafe-pack DIR` | ezafe model pack, loaded at startup |
| `--depparse-pack DIR` | depparse model pack, loaded at startup |
| `--model PATH\|NAME` | embedding `.vec`/`.rvec` file or hub model name |
| `-h`, `--help` | print the CLI usage and exit 0 |

Startup failures are reported on stderr and no response lines are written:
an unknown option or a missing option value exits with status 2; a pack or
embedding model that cannot be loaded exits with status 1. See
[errors.md](errors.md).

## Envelope

Success:

```json
{"ok":true,"result":...}
```

Failure:

```json
{"ok":false,"error":"<message>"}
```

Exactly one of `result` and `error` is present. The examples below show the
bytes the Rust binary writes today: compact JSON with the object keys in
alphabetical order. The console script writes the same objects without the
compact separators (`{"ok": true, "result": ...}`). Whitespace and object field
order are not part of the protocol; clients must not depend on them. Error
message wording is not part of the protocol either â€” the shape is.

## Version handshake

Request:

```json
{"command":"version"}
```

Response:

```json
{"ok":true,"result":{"name":"rishe","protocol":1,"version":"0.11.3"}}
```

| field | type | meaning |
|---|---|---|
| `name` | string | always `"rishe"` |
| `protocol` | integer | protocol version, currently `1` |
| `version` | string | version of the server answering (its Rust crate or Python package version) |

A client checks `protocol` first: a value other than `1` means the server does
not speak this protocol. `version` reports the code that is running: the Rust
binary reports its Cargo package version and the console script reports the
installed `rishe` package version, the same number for a matching release. The
handshake ignores every other request field. This is the bare version string;
`rishe --version` on either CLI prints the same version with the program name
(`rishe 0.11.3`).

## Requests

Every request is a JSON object with a `command` string. `text` is optional and
defaults to the empty string; a non-string `text` is treated as absent. Every
other optional field follows the same rule: a value of the wrong JSON type is
treated as absent, so it falls back to its default instead of failing the
request. Both implementations apply that rule identically. Commands that need a
word or a pair of words read extra fields as listed below. The examples
reproduce the response shape; values that come from a model pack
(`tag`, `label`, `relation`) depend on the pack that is loaded.

### normalize

Request:

```json
{"command":"normalize","text":"Ù…ÙŠ ÙƒÙ†Ù…"}
```

Response: the normalized text as a string.

```json
{"ok":true,"result":"Ù…ÛŒâ€ŒÚ©Ù†Ù…"}
```

### tokenize

Request:

```json
{"command":"tokenize","text":"Ø³Ù„Ø§Ù… Ø¯Ù†ÛŒØ§"}
```

Response: an array of surface-token strings. This does not normalize; it is the
same tokenization as `rishe tokenize`.

```json
{"ok":true,"result":["Ø³Ù„Ø§Ù…","Ø¯Ù†ÛŒØ§"]}
```

### lemmatize

Request:

```json
{"command":"lemmatize","text":"Ù…ÛŒâ€ŒØ±ÙˆÙ…"}
```

Response: an array of lemma strings, one per token.

```json
{"ok":true,"result":["Ø±ÙØª"]}
```

### mask

Request fields:

| field | type | meaning |
|---|---|---|
| `text` | string | text to mask |
| `maskers` | array of strings | maskers to apply, in order; `stages` is accepted as a deprecated alias |
| `mask_token` | string | replacement string for every masker |

When `maskers` is omitted the process default is used (`--maskers`, otherwise all
maskers). When `mask_token` is omitted the process default is used (`--mask-token`,
otherwise each masker's own default). A `maskers` value that is not an array is
ignored, and non-string entries inside it are dropped, in both implementations.

```json
{"command":"mask","text":"info@a.co","maskers":["email"]}
{"ok":true,"result":"<EMAIL>"}
```

An unknown masker name is an error: `unknown masker: bogus`.

### date

Request:

```json
{"command":"date","text":"1402/05/12"}
```

Response:

```json
{"ok":true,"result":{"gregorian":{"day":3,"month":8,"year":2023},"iso":"2023-08-03","jalali":{"day":12,"month":5,"year":1402}}}
```

| field | type | meaning |
|---|---|---|
| `jalali` | object | `year`, `month`, `day` integers as written |
| `gregorian` | object | `year`, `month`, `day` integers |
| `iso` | string | `YYYY-MM-DD` |

Text that is not a Jalali date is an error: `cannot parse date from "not a date"`.

### pos

Request:

```json
{"command":"pos","text":"good bad"}
```

Response: an array of tagged tokens.

```json
{"ok":true,"result":[{"end":4,"start":0,"tag":"A","text":"good"},{"end":8,"start":5,"tag":"B","text":"bad"}]}
```

| field | type | meaning |
|---|---|---|
| `text` | string | token surface form |
| `tag` | string | POS tag |
| `start`, `end` | integer | character span in the request text |

### ner

Same request as `pos`. Response: an array of entities, empty when nothing was
found.

```json
{"ok":true,"result":[{"end":4,"label":"PERSON","start":0,"text":"good"}]}
```

| field | type | meaning |
|---|---|---|
| `text` | string | entity surface form |
| `label` | string | entity label |
| `start`, `end` | integer | character span in the request text |

### ezafe

Same shape as `pos`; the `tag` field holds the ezafe label.

```json
{"ok":true,"result":[{"end":4,"start":0,"tag":"A","text":"good"},{"end":8,"start":5,"tag":"B","text":"bad"}]}
```

### depparse

Response: an array of words with their dependency head.

```json
{"ok":true,"result":[{"end":4,"head":null,"relation":"root","start":0,"text":"good"}]}
```

| field | type | meaning |
|---|---|---|
| `text` | string | word surface form |
| `head` | integer or null | zero-based head index; `null` for the root |
| `relation` | string | dependency relation |
| `start`, `end` | integer | character span in the request text |

### embed

Request:

```json
{"command":"embed","text":"Ú©ØªØ§Ø¨"}
```

Response: the mean embedding vector, an array of numbers with one entry per
dimension.

```json
{"ok":true,"result":[0.07038600742816925,0.03595375642180443]}
```

Requires `--model`; see [Failure modes](#failure-modes).

### similar

Request fields:

| field | type | meaning |
|---|---|---|
| `word` | string | query word; falls back to `text` |
| `n` | integer | neighbour count, `5` when omitted |

Response: up to `n` `{"word","score"}` objects, best score first.

```json
{"command":"similar","word":"Ú©ØªØ§Ø¨","n":2}
{"ok":true,"result":[{"score":0.22518165409564972,"word":"Ø®ÙˆØ§Ù†Ø¯Ù…"},{"score":-0.18090607225894928,"word":"Ø±Ø§"}]}
```

A word that is not in the vocabulary is not an error on the server: the response
is `{"ok":true,"result":[]}`.

### similarity

Request fields:

| field | type | meaning |
|---|---|---|
| `a` | string | first word; falls back to `text` |
| `b` | string | second word; required |

```json
{"command":"similarity","a":"Ú©ØªØ§Ø¨","b":"Ú©ØªØ§Ø¨"}
{"ok":true,"result":{"score":1.0000001192092896}}
```

If either word is out of vocabulary the response is an error:
`out of vocabulary: "Ú©ØªØ§Ø¨" or "Ù†Ø§Ù…ÙˆØ¬ÙˆØ¯"`. A missing `b` gives
`similarity needs \`a\` and \`b\``.

## Failure modes

The envelope and the process stay alive in every case below; the error is for the
one request line that caused it. The `error` column shows the Rust binary's
wording; the console script's messages differ in places, which this protocol
allows â€” match on the `ok`/`error` envelope, not on the text.

| situation | response `error` |
|---|---|
| the line is not JSON | `invalid JSON: <parser message>` |
| the line is not valid UTF-8 | `request line is not valid UTF-8` |
| the object has no `command` | `request has no command` |
| the command is not one above | `unknown command: <name>` |
| embedding command and no `--model` | `serve has no embedding model; pass --model <path-or-name>` |
| `similarity` without `b` | `similarity needs \`a\` and \`b\`` |
| a word is out of vocabulary | `out of vocabulary: "<a>" or "<b>"` |
| unknown masker | `unknown masker: <name>` |
| `date` text cannot be parsed | `cannot parse date from "<text>"` |

A binary built without the `models` feature has no pack inference at all, so the
four pack commands answer with:

```json
{"ok":false,"error":"pos requires the `models` feature; install with `cargo install rishe --features models`"}
```

(`ner`, `ezafe` and `depparse` use the same message with their own name.) The
Python package always includes `models`, so the console script never reports
this one. A binary built *with* `models` but started without the matching
`--*-pack` option answers `pos pack is not loaded` (again with the command's own
name), and the console script answers the same way. The embedding commands work
in either build as long as `--model` is given.

Model inference failures are reported the same way, for example
`inference failed: <detail>`.

## Compatibility

Protocol `1` is the contract described above:

- New commands and new optional request fields are minor additions. A client that
  ignores an unknown result field keeps working, and a server that receives an
  unknown field ignores it.
- The request fields and result shapes documented here do not change before
  `1.0.0`.
- Removing or renaming a command, changing a result shape, or changing what an
  existing request means is a major change and would be a new protocol version.
- Error message wording may change in any release, as long as the `ok`/`error`
  envelope and the per-line isolation stay as described.

See [STABILITY.md](STABILITY.md) for how protocol changes fit the version number,
and [errors.md](errors.md) for the exit codes around the server.

## Verified implementations

The Rust binary and the console script were driven with the same request lines
and return equal result objects for the handshake, `normalize`, `tokenize`,
`lemmatize`, `mask`, `date`, `pos` (with the `tests/fixtures/tiny-pack` pack),
`embed`, `similar` and `similarity`, and for every failure mode in the table
above. Only error message wording differs. The `ner`, `ezafe` and `depparse`
success shapes need a real task pack and were not part of that comparison.

The end-to-end tests for each server are `tests/cli.rs` for the Rust binary and
`python/tests/test_cli.py` (`ServeProtocolTest`) for the console script.
