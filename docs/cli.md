# Command line

The `rishe` command ships with the Python package (`pip install rishe`) and as a
Rust binary (`cargo install rishe`). Both accept text as an argument or read it
from stdin when the argument is omitted. The Hub and JSON-lines server commands
are available in both binaries.

```bash
rishe normalize "مي كنم"                 # می‌کنم
rishe tokenize "سلام دنیا"               # سلام دنیا
rishe tokenize "سلام دنیا" --json        # ["سلام", "دنیا"]
rishe lemmatize "می‌روم" --json           # ["رفت"]
rishe mask --stages url "به https://a.b برو"
# به <URL> برو
```

`mask` defaults to all maskers; pass `--stages` to choose, and `--mask-token` to
override the per-stage default. The Rust binary takes comma-separated stages
(`--stages url,email`).

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

## Models

Downloads are explicit; the base install never touches the network.

```bash
rishe hub list                             # registered models and cache status
rishe hub pull hazm-fasttext-300           # 2.3 GB, checksum-verified
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
