# Command line

The `rishe` command is installed with the Python package. Every command accepts
text as an argument or reads it from stdin when the argument is omitted.

```bash
rishe normalize "مي كنم"                 # می‌کنم
rishe tokenize "سلام دنیا"               # سلام دنیا
rishe tokenize "سلام دنیا" --json        # ["سلام", "دنیا"]
rishe lemmatize "می‌روم" --json           # ["رفت"]
echo "به https://a.b برو" | rishe mask --stages url
# به <URL> برو
```

`mask` defaults to all maskers; pass `--stages` to choose, and `--mask-token` to
override the per-stage default.

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
