# Command line

The `rishe` command ships with the Python package (`pip install rishe`) and as a
Rust binary (`cargo install rishe`). Both accept text as an argument or read it
from stdin when the argument is omitted. The Hub and JSON-lines server commands
are available in both binaries; `rishe convert`, which rewrites embedding files,
ships with the Rust binary.

```bash
rishe normalize "مي كنم"                 # می‌کنم
rishe tokenize "سلام دنیا"               # سلام دنیا
rishe tokenize "سلام دنیا" --json        # ["سلام", "دنیا"]
rishe lemmatize "می‌روم" --json           # ["رفت"]
rishe mask --maskers url "به https://a.b برو"
# به <URL> برو
```

`mask` defaults to all maskers; pass `--maskers` to choose, and `--mask-token`
to override the per-masker default. `--stages` is accepted as a deprecated alias
for `--maskers`. The Rust binary takes comma-separated maskers
(`--maskers url,email`) and accepts `--stages` the same way.

`rishe tokenize` prints surface tokens and does not normalize; compose the
commands for the normalizing path:

```bash
rishe normalize "مي كنم" | rishe tokenize    # می‌کنم  (one token)
```

## Exit codes

Both the console script and the Rust binary return the same statuses; the full
contract, including error-message rules, lives in [errors.md](errors.md).

| status | meaning |
| --- | --- |
| `0` | success, including `--help` and `--version` |
| `2` | usage error: unknown command, unknown or missing option value, wrong argument count |
| `1` | any failure after a valid command line |

A valid command line that cannot do its work exits `1`: a missing or unreadable
file, a corrupt embedding, an unparseable date, an out-of-vocabulary word, a
pack or model that fails to load, a network or cache error, or a `serve` startup
failure. Errors go to stderr and stdout carries only results. `serve` reports a
bad request line in the response object (`{"ok": false, ...}`) and keeps
running, so it exits `0` unless it fails to start.

## Date parsing

`rishe date` accepts `۱۴۰۲/۰۵/۱۲`, `1402-5-12`, `12.5.1402` and
`۱۲ مرداد ۱۴۰۲`, and prints the Gregorian ISO date:

```bash
rishe date "۱۲ مرداد ۱۴۰۲"                # 2023-08-03
rishe date "1402/05/12" --json            # jalali + gregorian + ISO
```

```python
from rishe import parse_date

date = parse_date("۱۲ مرداد ۱۴۰۲")
date.iso                # '2023-08-03'
date.format_persian()   # '۱۲ مرداد ۱۴۰۲'
```

In Rust the same logic is available as `rishe::dates`:

```rust
let date = rishe::dates::parse("۱۲ مرداد ۱۴۰۲").expect("date");
assert_eq!(date.to_iso().as_deref(), Some("2023-08-03"));
assert_eq!(date.format_persian(), "۱۲ مرداد ۱۴۰۲");
```

## Dependency parsing

`rishe depparse` loads a depparse pack and prints one word, head index and
relation per line. `head` is zero-based; `-` marks the root.

```bash
rishe hub pull depparse-compact
rishe depparse --pack "$(rishe hub info depparse-compact | cut -f2)" \
  "کتاب‌های خوب را به دوستانم می‌دهم."
# کتاب‌های	5	obj
# خوب	0	amod
# را	0	case
# به	4	case
# دوستانم	5	obl:arg
# می‌دهم	-	root
# .	5	punct
```

The Python API returns `Dependency` objects with `text`, `head`, `relation`,
`start` and `end`:

```python
from rishe import Pipeline

pipe = Pipeline(depparse_pack="path/to/depparse")
for item in pipe.depparse("کتاب‌های خوب را به دوستانم می‌دهم."):
    print(item.text, item.head, item.relation)
```

## Embeddings

`rishe embed`, `rishe similar` and `rishe similarity` use a static word-vector
model. `--model` accepts a file path (`.vec` or `.rvec`) or a hub model name; it
defaults to `rishe-embed-fa-300-v2`, so pull that once.

```bash
rishe hub pull rishe-embed-fa-300-v2
rishe similar کتاب --top 5
rishe similarity کتاب خوب
rishe embed "این کتاب خوب است" --json
```

`similar` prints `word<TAB>score`, `similarity` prints one score, and `embed`
prints the mean sentence vector (space-separated floats, or a JSON array with
`--json`). Out-of-vocabulary words exit with status 1; see
[Exit codes](#exit-codes).

`rishe convert` rewrites a word2vec text file in Rishe's compact binary format.
The format is auto-detected on load, so both files behave identically; `f16`
halves the file size and changes a stored value by less than 0.1% relative.

```bash
rishe convert model.vec model.rvec            # f16 by default
rishe convert model.vec model.f32.rvec --dtype f32
```

The JSON-lines server accepts the same three commands when started with
`--model`: `{"command":"similar","word":"کتاب","n":5}`,
`{"command":"similarity","a":"کتاب","b":"خوب"}` and
`{"command":"embed","text":"این کتاب خوب است"}`.

## Models

Downloads are explicit; the base install never touches the network.

```bash
rishe hub list                             # registered models and cache status
rishe hub pull rishe-embed-fa-300-v2       # 42 MB, checksum-verified
rishe hub path rishe-embed-fa-300-v2       # local cache path
rishe hub pull rishe-embed-fa-300          # 135 MB, the 0.9.x pack
rishe hub pull hazm-fasttext-300           # 2.3 GB external comparison model
rishe hub pull https://example.com/fa.vec --sha256 <64-hex-digest>
```

```bash
rishe hub list
rishe hub pull depparse-compact
rishe hub info depparse-compact
rishe hub verify depparse-compact
```

Models land in `$RISHE_HOME/models` or `~/.cache/rishe/models`, and
`rishe.hub.path(name)` returns the local path for
`rishe.Embeddings(...)`.

## JSON-lines server

`rishe serve` reads one JSON object per line from stdin and writes one JSON
response per line. It is dependency-free and keeps one `Pipeline` alive across
requests.

Pass `--model` (and/or `--pos-pack`, `--depparse-pack`, ...) to load optional
models once for the whole session. The embedding commands are only available
when `--model` is set.

```bash
printf '%s\n' \
  '{"command":"normalize","text":"مي كنم"}' \
  '{"command":"tokenize","text":"سلام دنیا"}' \
  '{"command":"mask","text":"info@a.co","stages":["email"]}' \
  | rishe serve
```

```json
{"ok": true, "result": "می‌کنم"}
{"ok": true, "result": ["سلام", "دنیا"]}
{"ok": true, "result": "<EMAIL>"}
```
