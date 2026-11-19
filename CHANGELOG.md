# Changelog

All notable changes to Rishe are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Model packs are versioned separately from the crate; their versions are recorded
in the pack manifest and on the model repository.

## [1.0.5] - 2026-11-19

### Fixed

The 1.0.4 audit's remaining findings, every one shipped with the regression test
that would have caught it. No public item was removed or renamed, so this is a
patch like 1.0.4.

- **Morphology.** `lemmatize` read `ترساند` as a form of `ترسید`: the whole token
  was never looked up before the person endings were stripped, so its final `د`
  left `ترسان`. The participle heuristic accepted any verb stem instead of a *past*
  stem, so `شماره` (`شمار`) and `کافه` (`کاف`) became `شمرد`/`کافت` — 321 nouns
  stopped over-stemming. `verbs.tsv` keyed the real present stems `گذر`/`شنو`/`یاب`
  to the non-words `گدشت`/`شنفت`/`یابید` (absent from all 187,582 nouns and 27,132
  frequency entries), so `conjugate` invented a paradigm for a spelling nobody
  writes; the present-stem rows now point at `گذشت`/`شنید`/`یافت` and the typo
  identity rows are gone. A preverb verb now conjugates preverb-first
  (`برمی‌گردم`, `برنمی‌گردم`, `برمی‌گشتم`, `برنگشتم`, `برگردم`) and reads its own
  output back. The whole negative past and subjunctive of a vowel-initial verb
  (`نیامد`, `بیایم`) failed to parse; the `ی` glide is resolved only when the token
  really is in the candidate verb's paradigm, so `نیاز` and `بیار` stay nouns.
- **Normalization.** All three ezafe spellings (`خانۀ`, `خانه‌ی`, `خانه ی`) flatten
  to plain `خانه` — uniform text, and search works. `خانه‌یی` and `خانه یا` are
  left alone. `punctuation_spacing` split `3.14`, `12:30`, `v1.0.4` and every URL;
  `orthographic_spacing` deleted a bare `\r`. `spacing` was not idempotent
  (`خوب؟‌ها`), split a bracket pair (`((ب))` → `( (ب))`), and `repeated_letters`
  trimmed leading/trailing space it was never asked to touch.
- **Maskers.** The shield mechanism rewrote real `ژژ`/`چچ` (14 lexicon words);
  shields are now bound once per chain and no longer collide with the text.
  `mask_diacritics` listed eight harakat while `Normalizer` strips U+064B..U+0656
  plus U+0670, so `آب` lost its madda instead of composing to `آ` and
  `compat(remove_diacritics=True)` disagreed with `normalize`. The emoji ranges
  missed the Misc Symbols and Arrows block, so `⬛`/`⭕`/`⬅` stuck to the word.
- **Command line.** `--maskers` now replaces the defaults on its first use and
  appends on repeats, in both binaries. `spell_correct` corrected per line instead
  of collapsing every line break into one space.
- **Hub.** A pinned model could be overwritten through a case variant or a
  trailing dot/space in the file name; `same_cache_file` compares case-insensitively
  and `_safe_filename` rejects the trailing dot. Python `hub.pull(registered,
  sha256=…)` now raises instead of silently ignoring the pin.
- **Docs and data.** `docs/errors.md` promised `ValueError` for a missing pack
  (it is `FileNotFoundError`), claimed `--top 0` exits 1 (both binaries exit 2),
  and `docs/STABILITY.md` listed a phantom `rishe.dates`. `CHANGELOG.md` said
  187,591 nouns; the file has 187,582. `docs/api.md` described `join_verb_parts`
  backwards. `docs/BENCHMARKS.md` labelled one run as both 0.11.4 and the 1.0.0
  reference. `scripts/gen_presentation_forms.py` hard-coded one machine's absolute
  path, so it could not be re-run anywhere else.
- **Augmentation.** `Rng::new` XORed the seed with the golden-ratio constant, so
  the seed that *equals* that constant produced state 0 — the one state xorshift
  cannot leave — and `keyboard_noise` emitted a single space. The state is now
  SplitMix64-mixed.

## [1.0.4] - 2026-11-18

### Fixed

Everything below is one of the ~45 findings the 1.0.3 audit ranked "should fix";
each ships with the regression test that would have caught it. No public item was
removed or renamed, so this is a patch, not a minor.

- **Morphology and the tokenizer round-trip.** `tokenize` dropped a trailing ZWNJ,
  so `detokenize` could not put it back. `detokenize` put a space before ASCII
  `,`/`;` and around both ASCII quotes; the quote toggles now. The identity rows
  for `است`/`نیست`/`هست` shadowed the verb table, so the copula read as a noun.
  Perfect 1sg/2sg (`رفته‌ام`) collapsed to the participle while 3pl reduced to the
  verb. `stem`/`split_token` stripped suffixes off verb forms (`رفت` → `رف`+`ت`,
  `کردیم` → `کردی`+`م`); `stem` now restores `گان` → `ه` like `lemmatize` does.
  `join_verb_parts` lacked the non-participle noun guard (`خنده است`), and the
  copula split ran on `است`/`نیست`/`هست`/`چیست`.
- **Ezafe.** A marker was inserted before every bare past verb
  (`او کتاب خواند`); `ezafe.rs` had no tests at all before this.
- **Maskers.** Maskers and the utilities used different word-boundary predicates,
  so `استx` counted as the stopword `است`. `mask_non_persian` took two flags no
  surface could pass; removed, it now drops English the way its name says.
- **`data/offensive_words.txt` and `data/informal_words.tsv`.** Five duplicate
  rows, a vocalised entry `normalize` can never match, and the space/ZWNJ/closed
  variants were missing, so `بی پدر`, `بی‌ناموس` and `کصلیس` all passed the filter.
  Ten rows had a stray edge ZWNJ, six keys were unreachable (including the common
  colloquial `رو` → `را`) and four lemmas emitted a leading U+200C.
- **Conjugation.** The negation or particle went before a separable preverb
  (`نبرگشتم`, `ببرگردم`); preverbs come from the stem table and keep their place
  (`برنگشتم`, `برگردم`). 83 lemmas without a present stem got one fabricated from
  the past stem, and the present paradigm is now empty rather than invented. The
  infinitive key (`بیختن` → `بیخت`) was picked as the present stem. `verbs.tsv`
  lost the unresolvable `آهنج یا آهاز` key.
- **`to_tajik`.** It counted the ZWNJ as a word letter (`خونҳҳо`, `خانۀ` → `ҳ`) and
  softened every final `ه` (`ماه` → `моа`).
- **`augment`.** The keyboard-shift row for `9`/`0` was swapped (`90` → `)(`), a
  seed of 0 was clamped to 1, Persian digits and `ژ` had no neighbours, and
  `ocr_noise` ate Persian digits.
- **`inflector`.** `possessive("ایرانی")` inserted no ZWNJ; the module had no
  tests. `possessive("ایرانی", 0)` is `ایرانی‌ام` and `possessive("دنیا", 0)` is
  `دنیایم` now.
- **Lexicon data.** `nouns.tsv` had 16k keys the tokenizer can never produce
  (Arabic-coded letters, 127 punctuation keys): canonicalized and filtered,
  190,450 → 187,582 rows, every `key == lemma`, byte-sorted. 163k of the 190k
  `word_freq.tsv` rows had frequency 0, which is what made `spell_correct` prefer
  junk; only real counts are kept (27,132 rows). `extract_lexicons.py` applies the
  same canonicalization and filtering, so a regeneration keeps both files clean,
  and `build.rs` asserts a key has no whitespace so a hand edit fails loudly.
- **Corpus readers.** `ConlluReader` reported nothing when a row had the wrong
  column count; it warns on stderr with `file:line` now. `PlainTextReader` and
  `PeymaReader` open with `utf-8-sig`, so a BOM no longer becomes part of the
  first sentence.
- **CLI exit codes.** Every command exited 101 on a closed stdout
  (`rishe hub list | head -1`); a print hook makes it a clean 0. The Python binary
  wrote CRLF on Windows, rejected `--top < 1` with exit 1 where Rust uses 2, and
  accepted `hub list extra` where Rust rejects it. Rust read a stdin BOM as a
  token and Python did not; both drop it now. A `--top` past `usize` was
  `OverflowError` with a traceback in Python and exit 2 in Rust; both exit 2.
- **Hub locks and cache safety.** A killed pull left a `.lock` that refused
  retries for an hour with no way out; `hub unlock <name>` (Rust and Python)
  clears it without deleting the payload, and for a pack the leaked lock blocked
  every pack. A custom URL could overwrite a registered model's cache file with
  bytes only the caller's digest vouched for; both implementations refuse that
  name, and `--sha256` on a registered model is refused instead of ignored.
  Windows device names (`NUL`, `COM1`, …) are refused: they download, verify and
  then can never be renamed into place, leaving an unremovable `.part`. A body
  with no `Content-Length` was unbounded; the cap is 8 GiB. `hub list` said
  `cached` for a corrupt file; it checks the digest. Python: a complete `.part`
  answered with 416 was deleted and re-fetched, a short body was reported as a
  checksum mismatch and its resumable prefix deleted, `http.client` errors escaped
  as tracebacks, `file:`/`ftp:`/`data:` URLs copied local bytes into the model
  cache, a foreign lock was deleted in `finally`, and an empty `HOME` picked a
  different cache root than Rust.
- **Conjugation and the Python binding.** `conjugate()` returned a `HashMap`, so
  the Python dict order changed per call and per process; the binding builds it
  in a fixed field order.
- **`compat.Normalizer`.** The alphabet table covered 150 Arabic presentation
  forms and missed 49, so `ﻙﺕﺍﺏ` came back as `کتﺍب`. Removing a symbol left a
  double space (`کتاب © ۲۰۰۰`). The special-character filter asked
  `unicodedata.category` once per character instead of once per distinct
  character and the verbal-spacing pass ran twice; a 1 MB document went from
  3.0 s to 2.6 s. `Lemmatizer` accepted `pos` and `joined_verb_parts` and ignored
  both.
- **`compat` constructor flags.** `unicodes_replacement` and the
  `seperate_mi`/`correct_spacing` pair were stored on the instance and then never
  consulted, because `__init__` raises for both cases. The dead attributes are
  gone.
- **`process_batch("…")`.** A bare string was iterated as characters and returned
  one document per character.
- **Embeddings.** A duplicate vocabulary row resolved to whichever row the
  unstable sort put first, so `vector()` could return the first or the last row;
  the order is `(word, row)` and the first row always wins. `similarity()` used an
  f64 loop while `most_similar()` used the vectorized f32 dot product, so one pair
  had two answers; both go through one scorer. Exact score ties are alphabetical
  instead of heap order. The `.rvec` reader trusted the header count and ignored
  trailing bytes, so a shrunken count decoded a smaller model; the file length
  must match the header now.
- **Model packs.** In `labels.json`, a `max_len` of `16.0`, `"16"`, `-5` or
  `null` silently became the 192 default and skipped the `3..=4096` check, and a
  non-string label was dropped, which shifted every later class index; both are
  errors. `depparse` returned an empty parse and exit 0 when the first word could
  not be encoded, while the same word in second position was a hard error.
- **Errors follow `API_CONVENTIONS.md` §11.** A missing `.vec` or pack directory
  raises `FileNotFoundError` from `Embeddings`, `ModelPack` and `Pipeline`, not a
  generic `OSError`/`ValueError`.
- **A bad count is a `ValueError`.** `rake`, `textrank`, `ngrams`, `spell_suggest`
  and `most_similar` took `usize`, so Python's own conversion raised
  `OverflowError` — a type `errors.md` never mentions, and not a `ValueError`, so
  `except ValueError` around a bad argument did not fire.
- **`hub list` calls a corrupt cached file `corrupt`.** It printed `cached` for any
  file that existed; `hub verify` and the Rust binary hash it against the pinned
  digest.
- **A whole-sentence dependency parse is capped at 512 words.** The arc decoder
  holds a `words × (words + 1)` score matrix, so a 4094-word paragraph needed
  roughly 700 MB and the process was killed instead of reporting anything.
- **`rishe normalize ""` means an empty document, not stdin.** Only no text (or a
  single `-`) reads stdin, which is what the Rust binary already did.
- **A bad `--top` is a usage error in both binaries.** `similar … --top 0` exited
  1 in the Rust binary when the model was missing and 2 in Python, because both
  checked it after loading the model.
- **`hub pull` refuses a non-http scheme by rule.** `file:`, `ftp:` and `data:`
  were rejected only because the HTTP client could not parse them, so the rule was
  never stated or tested.
- **The shared pack manifest is waited for, not failed on.** Two `hub pull`s for
  different packs collided on it, so the second failed while the first downloaded.
- **`hub verify` no longer blesses an empty `files` object.** The loop ran zero
  times and returned true, while the Python side refused it.

### Changed

- `scripts/api_surface.py` reads `pub use` re-exports, so `JalaliDate` is in
  `docs/api-surface.txt` (217 → 218 Rust items). `python/tests/test_surface.py`
  now enforces the naming rules on the Rust snapshot too, with the four names
  frozen at 1.0.0 as its shrinking allowlist.
- `scripts/pull_test_packs.py` also exports `RISHE_DEPPARSE_PACK` and
  `RISHE_EMBEDDINGS_PACK`, which is what the Rust pack tests read; a set-but-
  missing path is an error instead of a silent skip.
- One `word_boundary` predicate for the crate: `maskers` kept a second, identical
  copy whose comment described the rule differently, so the two agreed by
  accident and any edit to one would have made `mask` and `rake` disagree about
  where a word ends.
- `ya` converts the word-final `ه‌ی` before punctuation as well as before a space:
  `خانه‌ی.` used to stay unconverted while `خانه‌ی ما` became `خانۀ ما`, so one
  word had two spellings.
- `punctuation_spacing` separates a leading `،`/`…`/`؟` from the word after it;
  `، دنیا` came back as `،دنیا`.
- `rake` and `textrank` return nothing for `max_length=0`, which asks for phrases
  of at most zero words. It was clamped to 1 and answered with one-word phrases.
- `normalize` strips an isolated-form haraka (`ﹰ`, `ﹶ`, …): the generated table
  filtered them out because their `NFKC` form is a space plus a combining mark, so
  `strip_diacritics` never saw them. Four ligature forms were missing for the same
  reason; the table grows from 698 to 721 entries.
- Seven presentation-form ligatures that decompose to a letter plus a mark (`ﱝ`
  is `ذ` with a superscript alef, `ﴼ` is `ا` with a fathatan) were missing from
  the same table, because the generator accepted an expansion that was all
  letters or all marks but never both. `ﱜ`, `ﲐ` and `ﳙ` came back from `normalize`
  verbatim. The table grows again, to 728 entries.
- Seven unreachable rows left `data/nouns.tsv`: eight keys carried a ZWNJ at the
  edge, which normalization strips, and seven of them duplicated a row already in
  the file. `build.rs` refuses such a key now, as it already refused whitespace.
- Two more `data/nouns.tsv` keys held characters no Persian word can: `مشبهñبه` (a
  Latin `ñ`) and `گله‌دار8602` (digits glued on by a bad merge). Both tokenized only
  to their own junk spelling, and `مشبه` and `گله‌دار` were already in the file.
  `build.rs` refuses a key outside Persian letters and ZWNJ, which covers the
  class.
- Two rows in `data/verbs.tsv` used the infinitive as the past stem (`تب` →
  `تبیدن`, `نیاز` → `نیازیدن`). Because the past stem is what the past tense is
  built from, `conjugate("تب")` produced `تبیدنم` instead of `تبیدم`, and the
  lemma was the infinitive too. `build.rs` refuses that shape now.

### Doc corrections

- `docs/api.md` said `tokenize` returns tokens "exactly as they appear" and then
  listed the options that change them. The options are real but off by default, so
  the page now says that and names what each one does.
- `docs/serve-schema.md` said a `mask` request applies its `maskers` "in order"
  without saying that order is load-bearing. A masker only sees a string, so
  `["non_persian","url"]` deletes the URL's letters before `url` can match it and
  returns `به ://. برو`, while `["url","non_persian"]` keeps `<URL>` because the
  chain shields a placeholder already written. Only the default order is safe, and
  the page now says so.

### Tests

- Every per-stage `Normalizer` method is called by a test: `correct_spacing`,
  `decrease_repeated_chars`, `persian_style`, `persian_number`,
  `unicodes_replacement`, `seperate_mi` and `token_spacing` each wrap one named
  `normalize` stage and none was covered. `seperate_mi` leaves `ميروم` alone
  because the Arabic `ي` is not the Persian `ی`; hazm does the same, so that is
  compat and is now asserted rather than left as a surprise.
- The generated presentation table has a test that it is sorted and unique, since
  `expand` binary-searches it: an unsorted regeneration would silently stop
  resolving forms instead of failing.

### CI

- `pack tests` runs the two Rust pack tests as well as the twelve Python ones,
  and asserts the five variables they read are set.
- New `msrv models` job: the `models` feature needs 1.91 (`tract`), which
  `docs/STABILITY.md` documented and nothing checked.
- `features` runs `cargo test --no-default-features --lib`, not only `check`.
- `sdist` installs its own archive, so a file missing from it fails the release
  rather than the users who fall back to it.
- New `wheel-smoke` job installs each built wheel in a clean environment and
  imports it, so a wheel that builds but cannot load fails the release instead of
  the user's first `import rishe`. The aarch64 macOS wheel is cross-compiled and
  is not natively import-tested.
- `docs/RELEASING.md` matches the workflow: `cargo build` for the GPU features
  (they were `check`ed, which never links), the new jobs, the two Rust pack tests,
  and `audit` as the second non-blocking publish gate. `docs/cli.md` no longer
  claims `convert` "halves the file size" when the published pack went from 236 MB
  to 42 MB.

## [1.0.3] - 2026-11-17

### Changed

- **The `aarch64-unknown-linux-gnu` wheel is not part of 1.0.3.** `ring` 0.17.14
  cannot assemble its ARM asm in the current manylinux cross image, so that
  build is allowed to fail without blocking the release: ARM64 Linux installs
  from the sdist, which builds normally with Rust 1.91. Every other wheel is
  unaffected.

### Fixed

- **`spell_correct` no longer rewrites forms the morphology knows.** `زدم` (from
  `زدن`) came back as `دم`; every word the tables can analyse is left alone now,
  and a suggestion must carry a non-zero frequency to be offered at all.
- **The `email` masker handles Unicode local parts and IDN domains**:
  `کاربر@example.com` and `user@مثال.ایران` are masked instead of passing
  through and being deleted by the default chain.
- **`non_persian` keeps `ى` and `ة`**: `مصطفى كبيرة` is no longer shortened.
- **Arabic number separators make one number**: `tokenize("۱۲٬۰۰۰")` is a single
  token and `number_to_words("۱٫۵")` reads as «یک ممیز پنج» instead of raising.
- **`textrank` phrases come from adjacent words**: the fabricated
  `["کتاب خانه"]` is gone and real phrases like `هوش مصنوعی` are found.
- **CI refuses a truncated license file** (10,000-byte floor for
  `LICENSE-APACHE`), which is what shipped a 689-byte appendix notice in 1.0.2.

- **`mask()` no longer panics on multi-byte text next to `&` or `<`.** The
  bounded look-ahead that fixed the quadratic HTML scan used a byte count as a
  string index, so a window ending inside a Persian letter, `…` or an emoji
  panicked the slice. The window is now snapped back to a character boundary.
- **The Python CLI rejects a non-UTF-8 stdin exactly like the Rust binary:**
  one error line, exit 1. It used to decode with replacement characters and
  exit 0, so a corrupted file was silently processed as garbled text.
- **`LICENSE-APACHE` now carries the full Apache-2.0 text.** The file held only
  the "Licensed under…" appendix, while `Cargo.toml` and `pyproject.toml`
  declare `MIT OR Apache-2.0` and the sdist ships that file as the license.
- **A mirror that sends more bytes than it declared is cut off.** The declared
  length was used only to print a progress percentage, so an endless body filled
  the cache disk before the checksum ran.
- **A taken-over download lock is no longer deleted by its previous owner.** The
  lock carries a token now and is removed only while it is still ours, so a
  download that runs past the one-hour stale window cannot let a third process
  into the same `.part` file.
- **Two saves into one embedding path cannot corrupt it.** `Embeddings::save`
  used one fixed `.part` name, so concurrent saves truncated and interleaved each
  other's bytes; the temp name is unique per call now.
- **One over-long token no longer blanks POS/NER for the rest of the document.**
  A word whose pieces do not fit the model's `max_len` is skipped and the words
  after it are still tagged; a call with only such a word raises instead of
  returning an empty list.
- **`rishe hub pull <pack> --sha256` is refused instead of silently ignoring the
  pin** in the Python CLI, matching the Rust binary and `docs/errors.md`.
- **`--sha256` must be hex, not just 64 characters**, in the Rust binary: a typo
  is caught before the transfer starts instead of after it.
- **A lone-surrogate request line no longer kills `serve`**: the reply escapes it,
  so the line is answered and the loop keeps going.
- **`mask()` with the default maskers keeps the URLs and emails it just masked.**
  Each masker ran on the previous one's output, so `html` read `<URL>` as a tag
  and `non_persian`/`punctuation` ate its letters: text with a link came back
  with the link deleted. The placeholders are shielded between stages now.
- **`non_persian` keeps Arabic-coded Persian letters**: `كتاب` no longer becomes
  `تاب`, since `normalize` already maps those two code points.
- **The `html` masker no longer deletes Persian prose inside `<!…>` or `<?…>`**
  (or after an unmatched quote); only real comments, declarations and ASCII
  processing instructions count as markup.
- **Presentation forms normalise idempotently and never to a non-Persian
  letter.** Table targets were pushed verbatim, so a medallion yeh came out as
  U+0649 and a second `normalize()` changed the string again.
- **Decomposed hamza/maddah compose:** `ا` + U+0653/0654/0655 becomes
  `آ`/`أ`/`إ`, and the remaining marks are still stripped.
- **`spacing` keeps a ZWNJ that is the only separator in a compound**
  (`آذر‌ماه`, `نماز‌جمعه`, `آن‌ها‌است`); deleting it destroyed shipped words.
- **`verbal_spacing`/`word_spacing` no longer respell known nouns or split
  unknown words** (`میانجی`, `میگساری`, `معانات`).
- **Character tables leave other scripts alone:** Hebrew `פ`, Canadian `ᒿ`,
  Hangul `ㅡ` and CJK `一`/`ー` are no longer rewritten to Persian letters,
  digits or punctuation.
- **`repeated_letters` truncates letters only**, so `...` (ellipsis) and `!!!`
  keep their characters.
- **Bidi controls are removed** by `spacing` and the default normalizer
  (U+202A–202E and U+2066–2069, not only the four previously listed).
- **The default normalizer's `ها`/`تر` rule respects non-joining letters:**
  `مرد ها` becomes `مردها`, the spelling the lexicon has, not `مرد‌ها`.
- **A URL ending in `,`, `;`, `!` or `?` no longer swallows the next word.**
- **`tokenize` and the maskers share one linear sigil scan**: the boundary test
  runs before the body scan, removing the quadratic path.
- **`possessive()` inserts a ZWNJ for `ی`-final nouns**: `ایرانی` →
  `ایرانی‌ام`, not `ایرانیم`.
- **f16 conversion accepts every value IEEE rounds to a finite half**, and a
  failed rename no longer leaves a `.part` file behind.
- **`hub remove` clears a stale download lock and the shared manifest's
  `.part`/`.lock`** in both binaries: a killed pack pull no longer blocks every
  pack for an hour, and a running download refreshes its lock.
- **A 416 response promotes a complete `.part` instead of re-downloading it**,
  and a local rename failure is fatal rather than retried three times.
- **`hub path` resolves task packs** (Rust), matching the Python API and docs.
- **Python `hub.verify` refuses an empty pack directory**, `hub.remove` clears
  the manifest part, and the cache root resolves `HOME`/`RISHE_HOME` like Rust.
- **Python `process_batch("…")` treats a string as one document**, not one per
  character, and `keywords(n=0)` raises `ValueError`.
- **The Python `mask --maskers` flag takes one value**, so the documented
  `--maskers url "متن"` order works.
- **`docs/errors.md` and `docs/serve-schema.md` are readable UTF-8 again**; their
  examples had been double-encoded through cp1252.

### Changed

- **`lemmatize` no longer invents a lemma from a clitic form.** `بهمین` came back
  as `بهم` because `بهم` is a fused preposition+pronoun form, not a noun stem; the
  suffix is no longer stripped there, so `stem`, `lemmatize` and `split_token` all
  leave the word alone.
- **`rishe` reads piped text as the document it is.** Both CLIs used to `trim()`
  stdin, which deleted the blank lines and indentation a file starts with; they
  now drop only the newline the shell adds. Single-line input is unchanged.
- **`ureq` and `sha2` are now behind a `hub` feature (on by default).** They are
  only used by `src/hub.rs`, which is the pack downloader, so a library build
  with `default-features = false` no longer compiles a TLS stack it never calls.
  The `rishe` binary requires the feature; `cargo install rishe` is unchanged.
- **The CLI and `serve` now emit JSON through one encoder, so object keys come
  out in one order (alphabetical).** `date --json`, `similar --json` and the
  `--pack` commands used a hand-written escaper and their own key order, which
  differed from every other command. The payload is unchanged; only the order of
  the keys inside an object moved.

### Removed

- **`rishe.api.TOKENIZE_OPTIONS` and `rishe.cli.DEFAULT_MASK_STAGES`.** Neither
  was used: pyo3's `signature=` already rejects an unknown keyword argument to
  `tokenize()`, and `DEFAULT_MASKERS` is the one place the masker list lives.

## [1.0.2] - 2026-11-16

No API or format change. This release closes the largest remaining gap in the
test suite (model inference was never tested in CI) and fixes three latent
`0.3.0`-era bugs in the HTML masker and the date parser.

### Fixed

- **Nothing in CI ran ONNX inference.** Twelve tests in `python/tests/` are
  gated on `RISHE_TEST_PACK`, `RISHE_TEST_DEPPARSE_PACK` and
  `RISHE_TEST_EMBEDDINGS_PACK`, and no workflow set them, so every `pos`, `ner`,
  `ezafe` and `depparse` path shipped on manual testing alone — a `tract` or
  pyo3 change could break all four and CI would stay green. A `pack-tests` job
  now pulls the real packs with `scripts/pull_test_packs.py` and runs them, and
  both publishers need it. It is tag-gated and cached, because it downloads
  ~225 MB that a pull request should not pay for.
- **`test_typed_tokens_and_entities` called a method that does not exist.** It
  asked for `ModelPack.tag_tokens`, an alias `1.0.0` removed; the surface has
  `tags`. The test was env-gated, so it had never run since the alias was
  deleted and the break was invisible. It now calls `tags` and passes.
- **The `html` masker decoded numeric references into control characters.**
  `&#0;` became a NUL byte and `&#1;`-`&#x1f;`, `&#127;`, `&#xFFFE;` and
  `&#xFFFF;` came through raw — 30 code points HTML5 treats as parse errors.
  `mask("&#0;", maskers=["html"])` returned `"\x00"`, and the same text went
  through `rishe mask --maskers html` to any file or pipe. Control characters
  and noncharacters are now left encoded. Tab, newline and carriage return
  still decode.
- **The `html` masker deleted text that was escaped.** It unescaped first and
  stripped tags second, so `&lt;b&gt;سلام&lt;/b&gt;` became `سلام` instead of
  `<b>سلام</b>`: a page discussing markup lost the words inside the escaped
  brackets. Tags are now stripped before the text is unescaped, so `&lt;b&gt;`
  stays visible text.
- **A Gregorian ISO date was silently parsed as a Jalali year.**
  `parse_date("2023-08-03")` returned `JalaliDate(2023, 8, 3)` — Gregorian
  2644, 621 years off — because a leading four-digit year is read as Jalali.
  Since `JalaliDate.iso` emits Gregorian, round-tripping a date through its own
  ISO string quietly changed it. A numeric date with a year of 1700 or more now
  returns `None` instead; Jalali text years are 13xx-15xx. A date with a month
  name (`۱۲ مرداد ۲۰۰۰`) still parses, so a far year can be written unambiguously.

## [1.0.1] - 2026-11-15

No API, output or format change. This release closes the gaps in the release
process itself, all of which `1.0.0` exposed.

### Fixed

- **The GitHub Release was never created.** `1.0.0` published to both registries
  and pushed its tag, but the Releases page still showed `0.11.4` as latest
  because no job created the release. A `publish-release` job now does, using
  the CHANGELOG section for the tag's version as its notes, so the release page
  and the file cannot disagree. The section is read by
  `scripts/release_notes.py`, which refuses a tag with no section, so a missing
  entry can never publish an empty page.
- **A missing CHANGELOG entry failed after publishing, not before.** The check
  lives in the `docs` job, which both publishers already need, and it also
  rejects a tag whose version disagrees with `Cargo.toml`. A published version
  can never be re-uploaded, so this has to fail before either registry is
  touched rather than in `publish-release` afterwards.
- **The `cuda` and `metal` features were never compiled in CI.** They are opt-in
  and nobody builds them by default, so a break would reach a GPU user first.
  A `gpu-features` job now checks both. `metal` runs on macOS: `tract` declares
  `tract-metal` under `cfg(target_vendor = "apple")`, so checking it on Linux
  resolves to nothing. Neither publisher blocks on it — cuda and metal are not
  in the published wheel, so a GPU break fails the run without stranding one
  registry on a version the other refused.
- **Free-threaded CPython is now a stated decision, not a silence.** Tested on
  `3.13t` and `3.14t`: `import rishe` fails with `DLL load failed`, and
  `pip install rishe` falls back to the sdist and fails at link time on a
  missing `python3.lib`/`python3.dll`. An `abi3` wheel cannot load on a
  free-threaded build, and pyo3 0.22 has no free-threaded support at all
  (that arrived in 0.23), so supporting them is a dependency upgrade and a
  separate release, not a flag. `docs/STABILITY.md` now says this with the
  reason and the measured failure.

## [1.0.0] - 2026-11-14

The API freeze. The surface listed in [`docs/STABILITY.md`](docs/STABILITY.md)
is the promise: from here, removing or renaming a public name, changing a
return type, a default or a documented behaviour, or dropping a platform, a
Python version or the MSRV needs `2.0.0`. A new name arrives as a minor.

### Removed

Every alias that `0.11.0` deprecated and promised to drop here:

- Rust: `Pipeline::normalize_stages`, `mask_stages`, `lemma_full`,
  `word_tokenize`, `word_tokenize_with`; `ModelPack::tag_text`,
  `tag_text_items`, `tag_text_batch`, `tag_words`.
- Python: `Pipeline.pipe`, `process_stages`, `normalize_stages`, `mask_stages`,
  `word_tokenize`, `word_tokenize_with`, `lemma_full`; `ModelPack.tag`,
  `tag_tokens`, `tag_batch`, `tag_words`; `rishe.word_tokenize`;
  `hub.pull_pack`, `hub.cached_pack`, `hub.path_pack`.
- CLI: `--stages` is gone, `--maskers` is the only spelling. The `serve`
  request field `stages` is still accepted for 0.x clients, because the
  protocol is versioned separately.

The replacements: `tokenize` / `process` for tokens, `normalize(text, stages=...)`,
`mask(text, maskers=...)`, `lemmatize_verb`, `process_batch`,
`tags` / `tags_batch` / `tags_from_words`, and `hub.pull` / `cached` / `path`.

`TaggedToken.tag` and `Dependency.relation` stay. They are the historical field
names, equal to `label`; breaking the most-used attribute of a result class for
cosmetics was not worth it at the freeze.

### Changed

- The `DeprecationWarning` / `#[deprecated]` machinery is gone with the aliases
  it served.
- `docs/api-surface.txt` is regenerated and diffed in CI: the frozen Rust and
  Python surface is checked rather than reviewed.
- `docs/STABILITY.md` states the freeze and is linked from the README, the
  documentation index and both CLI helps.
- Building the Python extension from source needs the `extension-module`
  Cargo feature; maturin enables it through `pyproject.toml`, and the CI
  feature job runs the PyO3 tests.
- The `ProcessedDoc` stub says `lemmas` equals the tokens and
  `ezafe_markers` is empty when that stage is off, instead of promising
  aligned lemma and ezafe data for every stage list.

### Fixed

- `most_similar` caps the requested count at the vocabulary size, so an
  oversized `n` returns the whole vocabulary instead of panicking; this also
  keeps `rishe serve` and the Python server alive after such a request.
- A `.vec` file whose header declares an absurd vocabulary size now fails with
  an error instead of aborting the process.
- `JalaliDate.from_gregorian` returns `None` for years outside the supported
  calendar range instead of overflowing.
- A model pack whose `labels.json` has a `max_len` outside `3..=4096` is
  rejected when the pack is loaded.
- `Embeddings.save` writes a temporary file and renames it, so an interrupted
  `rishe convert` no longer leaves a truncated `.rvec`.
- `hub pull` in Rust resumes a partial download and reports progress on a
  terminal; the Python `pull` checks the checksum of an already cached file
  instead of trusting it.
- `hub remove` also deletes the `.part` file of an interrupted pull, and
  `hub list`/`hub info` report a partial model or half-filled pack directory
  as `partial` with its size instead of claiming it is absent.
- A checksum mismatch fails on the first attempt instead of downloading the
  whole file three times.
- `cargo clippy --features python,models` now runs in CI, as the release
  checklist says.
- `spell_suggest("")` returns an empty list, and `number_to_words`
  reports an int too large for `i64` as `OverflowError` instead of a
  `TypeError`.
- A `.vec` file whose first line declares a different number of vectors
  than the file contains is an error instead of a silently partial model.
- `tokenize(..., join_verb_parts=True)` no longer panics when the text ends
  with a verb prefix such as `خواهد`.
- `spell_suggest` and `spell_correct` skip words longer than 64 characters
  instead of building millions of one-edit candidates.
- The `html` masker no longer removes text between `<` and `>` that is not
  a tag.
- An embedding file with a non-finite value is rejected at load instead of
  returning NaN similarities.
- `possessive(noun, -1)` returns `None` instead of raising `OverflowError`.
- `possessive` inserts the linking `ی` after nouns ending in `ا`/`آ`/`و`, so
  `دنیا` gives `دنیایم` instead of `دنیام`.
- `JalaliDate.from_gregorian` returns `None` for days in 560 before
  560-03-20, matching the documented window instead of returning a date that
  cannot be converted back.
- The compat `Normalizer` honors `persian_numbers=False` and rejects
  `unicodes_replacement=False` and `seperate_mi=False` with
  `NotImplementedError` instead of silently ignoring them.
- `tokenize_spans` counts characters once per gap instead of once per token,
  so a long text costs linear time instead of quadratic.
- `spell_correct` remembers each word's correction within a call, so a text
  that repeats a misspelling is searched once per distinct word.
- `textrank` sums each word's link weights once instead of once per pass per
  link, so a wide `window` costs linear work per pass: 800 words went from
  46 s to 2.6 s at window 120.
- The sentence splitter looks back at most six characters to decide whether a
  dot ends an abbreviation, so a long run with dots and no spaces costs linear
  time: 84,000 characters of space-free Persian went from 1.35 s to under
  1 ms, and a one-line 354 KB list of URLs from 2.7 s to under 1 ms.
- `spell_correct` skips a word that contains no Persian letter instead of
  building one-edit candidates for it: 259 KB of JSON keys went from 3.3 s to
  12 ms.
- `persianize_suggest` counts character offsets incrementally instead of from
  the start of the text for every match: 780 KB with 16,000 loanwords went
  from 225 ms to 10 ms.
- `most_similar` skips the query word by text, so a `.vec` that lists a word
  twice no longer returns the query itself among its neighbours.
- The maskers no longer trim a text they did not mask, so masking leaves the
  surrounding whitespace alone when there is nothing to replace.
- The `mention` and `hashtag` maskers require a word boundary before the
  sigil, so an `@` or `#` inside a word (`info@example.com`) is no longer
  masked as a social token.
- `textrank`'s graph builder copies each word once per edge instead of twice.
- `plural`, `comparative`, `superlative`, `possessive` and `split_token`
  return an empty result for empty input instead of a bare suffix or `[""]`.
- The sentence splitter has unit tests for its punctuation, abbreviation,
  ellipsis and no-whitespace branches, which is where the quadratic scan above
  had been hiding.
- `hub pull <url>` no longer writes outside the cache directory. The incoming
  file name is validated as one plain path component, so a URL whose last
  segment carries a separator, a drive prefix or `..` is refused instead of
  joining to a path outside `$RISHE_HOME/models`. A query string or fragment is
  dropped from the inferred name instead of becoming part of it, which also
  fixes URLs like `model.vec?download=true` that previously failed with a raw
  filesystem error on Windows.
- `serve` ends cleanly when its reader closes the pipe. `rishe serve | head -1`
  used to panic on the write and exit 101; both the Rust and Python servers now
  treat a closed stdout as end of input and exit 0.
- A resumed download whose local `.part` prefix does not match the remote object
  restarts from zero instead of failing permanently. Only a mismatch on a fresh
  download, where the bytes came from the remote as-is, still fails on the first
  attempt.
- `rishe hub pull <pack> --sha256 <digest>` is refused instead of silently
  ignoring the flag. Task packs are verified against the registry manifest's own
  digests, and the Python CLI already rejected this.
- `hub.info()` always includes the `partial` key, `False` for a cached entry, so
  a caller can read it without `.get` or a `KeyError`.
- `hub remove <pack>` also clears a `rishe-models-manifest.json.part` left by an
  interrupted manifest download.
- `Normalizer(correct_spacing=False)` is honoured. The compat wrapper used to run
  the full default normalizer first, so the spacing step had already been applied
  before the flag was consulted and the switch did nothing.
- The copula split no longer mangles ordinary words that end in `ست`. `است`,
  `دست`, `هست`, `نیست`, `چیست`, `درست` and `مست` were being split into a stem plus
  `است` (`دست` became `ده است`), because the lexicon holds one-off entries such as
  `ده` and `مه` that made almost any stem look like a word. A token the lexicon
  already knows is now left alone, and a genuine contraction splits to its stem as
  written, so `کتابست` gives `کتاب است` and `عالیست` gives `عالی است` rather than
  the invented `کتابه است` / `عالیه است`.
- `split_token` no longer splits an ordinary word on a prefix. `برنامه`, `درخت`,
  `بازار`, `واژه`, `دریا` and `فروشگاه` came back as `بر`+`نامه`, `در`+`خت` and so
  on, because the prefix loop ran before the whole-word lexicon check.
- Lexicalized nouns ending in `ه` no longer lemmatize through a lookalike verb stem.
  `دانه`, `هسته`, `آیه`, `گوشه`, `سایه`, `پوشه`, `گیره`, `کنده` and `مانده` were
  reduced to `دانست`, `بود`, `آمد`, `گوشید`, `سایید`, `پوشید` and `گرفت`.
- `توت` is no longer read as `در` + `تو`; it is mulberry.
- `split_token` no longer strips a noun suffix off a fused clitic, so `بهمین`
  ("to this") stays one word instead of becoming `بهم` + `ین`. The same guard
  keeps `بهتان`, `تومان`, `بهشت` and `توتم` whole; a clitic form is not a noun
  stem.
- `split_token` keeps a ZWNJ morpheme boundary. `می‌فروش` and `پیش‌بینی` are
  lexicon entries, but the whole-word check no longer swallows them, so they
  still split to `می`+`فروش` and `پیش`+`بینی`.
- `convert` refuses to write f16 values outside ±65504 instead of writing a file
  that `load` then rejects as non-finite, and it leaves no partial file behind.
- An embedding row with a finite but very large magnitude no longer makes
  `similarity` return `NaN`. Norms and dot products accumulate in f64.
- A model whose output has the wrong number of sequence rows is an error instead
  of an index panic.
- A model pack whose `labels.json` disagrees with the model's own output width is
  an error naming both counts, instead of a panic or a mislabelled result.
- A downloaded model is flushed to disk before being moved into place, in the Rust
  and Python downloaders, so a crash during the move cannot leave a short file
  where a good one used to be.

## [0.11.4] - 2026-11-12

No functional changes: the public API, the output and the file formats are the
same as 0.11.3. This is the freeze candidate — it records the numbers `1.0.0`
will be compared against and re-verifies the published artifacts, leaving the
removal of the deprecated aliases as the only step to `1.0.0`.

Every published hub name was pulled from scratch and checksum-verified against
its manifest: `rishe-embed-fa-300-v2`, `rishe-embed-fa-300`, `pos-compact`,
`ner-compact`, `ezafe-compact`, `depparse-compact` and `hazm-fasttext-300`.

### Added

- `docs/BENCHMARKS.md` records the Rust engine baseline: median time per
  `Pipeline::process` call on this host, with the exact command and the
  run-to-run spread, so a later change can be compared with a number instead of
  a feeling.

## [0.11.3] - 2026-11-11

### Added

- `tests/guarantees.rs`: `Pipeline`, `Embeddings` and `ModelPack` are asserted to
  be `Send + Sync`, eight threads give the same result as one, and the output is
  identical across runs and across instances.
- `tests/hostile.rs`: a corrupt model pack returns an error, and a 10 MB single
  token finishes instead of hanging.
- `docs/STABILITY.md` states the concurrency and determinism guarantees, and says
  which tests hold the "error, never a panic" claim for each input format.
- `README.md` shows one badge row: the supported Python versions, total PyPI
  downloads and the license.

### Changed

- `README.md` no longer repeats the measured version table: the badge carries the
  Python versions, the install bullets keep the Rust floors, and the full matrix
  stays in `README.pypi.md`, `README.crates.md`, `README.fa.md` and
  [`docs/STABILITY.md`](docs/STABILITY.md).
- `docs/API_CONVENTIONS.md` no longer lists `ProcessedDoc.token_spans` as a field:
  it was never implemented, and character offsets are available from
  `tokenize_spans`. Adding it is additive for a later minor.

## [0.11.2] - 2026-11-10

### Added

- Doc comments on every public Rust item, enforced by `#![deny(missing_docs)]`,
  with doctests on `Pipeline`, `ProcessOptions`, `dates` and `hub`.
- Docstrings for every class, method and property in `python/rishe/_core.pyi`, so
  an editor shows the contract instead of a bare signature.
- `ProcessOptions::from_stages_checked`: the stage list of `from_stages`, except
  an unknown name is an error (additive Rust API).
- `rishe._core.DEFAULT_MASKERS`; `python/rishe/api.py` derives its tuple from it,
  so the Rust binary, the Python CLI and the package share one masker list.
- `docs/api-surface.txt` and `scripts/api_surface.py`, checked in CI, so a public
  item cannot disappear or be renamed unnoticed.
- `scripts/check_docs_examples.py`, which executes the Quick start blocks of all
  three READMEs in CI.
- `README.md` states the supported versions with measured numbers instead of a
  guess: CPython 3.9 – 3.14 for the wheel, Rust 1.88 for the default and
  `python` features, Rust 1.91 for `models` and for a source install of the
  Python package, each with the exact message an older tool refuses with. The
  PyPI, crates.io and Persian READMEs say the same, and `docs/STABILITY.md`
  carries the matrix.
- `Programming Language :: Python :: 3.14` and `Programming Language :: Python
  :: Implementation :: CPython` classifiers.

### Changed

- `process(stages=...)` rejects an unknown stage name with `ValueError` instead of
  ignoring it, so a typo cannot silently change the output; an empty list still
  means every stage, as documented. `normalize(stages=...)` already behaved this
  way.
- `serve` treats a request field of the wrong JSON type as absent in both
  implementations (`maskers`, `mask_token`, `word`, `n`), the documented rule for
  `text`. Previously the console script raised while the Rust binary used the
  default, so the same request got different answers.
- CI builds and tests the Python package on every supported CPython version
  (3.9, 3.10, 3.11, 3.12, 3.13, 3.14), and the MSRV job also compiles the
  `python` feature on Rust 1.88.

## [0.11.1] - 2026-11-09

### Added

- `serve` protocol 1: a `{"command":"version"}` handshake returning
  `{"name","protocol","version"}`, and a `date` command in both the Rust binary
  and the Python console script. The whole protocol is documented in
  `docs/serve-schema.md`.
- `docs/STABILITY.md`: which surfaces a version number covers, what a minor and
  a major release may change, the deprecation rule, and what is explicitly not
  covered (model quality, benchmark ranking, defaults, future schedules).
- `docs/errors.md`: the error contract for both CLIs (exit codes, stdout/stderr
  split, `serve` error envelope) and the exception types the Python API raises
  per failure mode.
- `.rvec` version 1 reader-forever fixture (`tests/fixtures/rvec-v1-f16.rvec`)
  with tests that load it and reject truncated or corrupt copies, in Rust and
  Python.
- Hub manifest schema versioning: the manifest reader accepts `"schema": 1`,
  treats an absent field as version 1, and refuses an unknown major version
  (Rust and Python readers agree).
- CI gates: MSRV 1.88 job, Python 3.9/3.12/3.13 matrix, offline `examples` smoke
  job, rustdoc with `-D warnings`, and a feature-combination matrix; the publish
  jobs now depend on them.

### Changed

- CLI error contract, in both the Rust binary and the Python console script:
  `0` success, `2` command line rejected by the parser, `1` failure after the
  command line parsed (missing file, bad data, out-of-vocabulary word, network,
  pack or model error). `-V`/`--version` prints `rishe <version>`.
- The Python console script's `serve` now speaks the same protocol as the Rust
  binary (version handshake, `date`, same error envelope), and `depparse`
  without `--pack` is a runtime failure (exit 1) in both.
- MSRV is now Rust 1.88. The locked dependency graph already requires it
  (`ureq` → `url` → `idna` → `icu` declare `rust-version = 1.88`), so the
  previously documented 1.75 was not buildable; `Cargo.toml`, both READMEs and
  `STABILITY.md` now say 1.88, and CI checks it.
- Clippy suggestions that became active with the MSRV bump are fixed in
  `embeddings`, `maskers`, `normalizer`, `sbd` and `utilities` with no behaviour
  change.
- Documentation: `docs/hazm.md` is now `docs/compat.md` and presents the
  `rishe.compat` classes as a feature ("drop-in preprocessing classes") rather
  than as a migration guide. The three per-release model cards are merged into
  one `docs/MODEL_CARDS.md`, linked from the README, `docs/index.md` and
  `docs/MODEL_BENCHMARKS.md`.
- Documentation: the quick-start examples in both READMEs and `docs/index.md`
  now show direct module-level calls (`rishe.normalize`, `rishe.tokenize`,
  `rishe.lemmatize`, `rishe.process(text).formal_text`) instead of constructing
  a `Pipeline`. `Pipeline` remains for configuration and for the methods that
  have no module-level function.

### Removed

- `docs/ROADMAP.md`. The project does not publish a roadmap and makes no
  promises about future releases; what is published describes what already
  shipped.

### Fixed

- `rishe.word_tokenize` (the deprecated 0.10 alias) keeps its old behaviour:
  it normalizes and joins verb parts again (`"رفته است"` → `["رفته_است"]`)
  instead of returning surface tokens, as the 0.11.0 entry promised.
- A corrupt `.rvec` header that declares an absurd vocabulary size is refused
  with an error instead of aborting the process while reserving memory; the
  reader-forever test now covers it.
- `serve` no longer dies on a request line that is not valid UTF-8: both
  implementations answer with an error envelope and keep serving, as the
  protocol page says. The Python console script also no longer prints a
  traceback for it.
- The Python console script with no arguments prints usage and exits 0, matching
  the Rust binary and `docs/errors.md`.
- `import rishe` works on Python 3.9 again: `rishe.corpora` used
  `dataclass(slots=True)`, which needs Python 3.10, so every 0.x wheel failed to
  import on the oldest interpreter it claimed to support. The new Python 3.9 CI
  job caught it.
- `hub verify` prints the same word (`invalid`) in both CLIs, as documented.
- `rishe::hub::run` keeps returning `Ok(())` for a failed `verify`, so Rust
  callers of the compatibility wrapper see no change in behaviour.

## [0.11.0] - 2026-11-08

### Added

- Module-level front door: `rishe.normalize`, `rishe.tokenize`,
  `rishe.tokenize_spans`, `rishe.sent_tokenize`, `rishe.process`,
  `rishe.process_batch`, `rishe.lemmatize`, `rishe.stem`, `rishe.mask`,
  `rishe.keywords` and `rishe.pipeline()`, all backed by one shared default
  pipeline created on first use.
- Canonical pipeline names: `Pipeline.process(text, stages=None)`,
  `process_batch(texts)`, `normalize(text, stages=None)`,
  `mask(text, maskers=..., mask_token=...)` and
  `tokenize(text, normalize=True, **options)`, plus
  `Pipeline.lemmatize_verb(word)`.
- `ModelPack.tags(text)`, `tags_batch(texts)` and `tags_from_words(words)`.
- `label` on every result span: `TaggedToken.label` and `Dependency.label`
  join the existing `Entity.label`, so one loop reads POS, NER and dependency
  output the same way.
- `rishe.hub.pull`, `cached` and `path` now accept task packs as well as
  single-file models; `hub.is_pack` reports which one a name is.
- `docs/API_CONVENTIONS.md`, the naming rules the public surface follows up to
  `1.0.0`, and `docs/ROADMAP.md`, the release-ordered backlog to it.
- `rishe._deprecation` helper so renamed names warn once and disappear at
  `1.0.0`.

### Changed

- Rust surface follows the same conventions as Python: `Pipeline::tokenize`
  takes a `word_tokenizer::WordTokenizerOptions` and returns surface tokens,
  `Pipeline::mask` replaces `mask_stages`,
  `Pipeline::normalize_with` replaces `normalize_stages`,
  `Pipeline::lemmatize_verb` replaces `lemma_full`, and `ModelPack::tags`,
  `tags_batch`, `tags_from_words` replace the `tag_text*` / `tag_words` names.
  `TaggedToken::label()` and `Dependency::label()` join `Entity.label`.
- `rishe mask` takes `--maskers`; `--stages` still works as a deprecated alias
  (same for `serve`). The masker list is defined once, in `rishe.api`.
- `tokenize` is now pure tokenization: it returns surface tokens and does not
  normalize. `normalize` normalizes, and `process` (or `pipe(text)`) runs the
  sequence; `word_tokenize*` keep their old normalize-then-tokenize behaviour as
  deprecated aliases. This applies to `Pipeline`, the module-level `rishe.tokenize`
  and `rishe tokenize` on the command line.
- Hazm-style defaults stay available from `rishe.compat` only; the compat
  façade now calls the canonical methods.
- Python test suites gained a naming ratchet (`python/tests/test_surface.py`)
  that pins the remaining deprecated aliases and may only shrink.

### Deprecated

Removed in `1.0.0`; each emits `DeprecationWarning` today.

- `Pipeline.word_tokenize` and `Pipeline.word_tokenize_with` → `tokenize`
- `Pipeline.process_stages` and `Pipeline.pipe` → `process` / `process_batch`
- `Pipeline.normalize_stages` → `normalize(stages=...)`
- `Pipeline.mask_stages` → `mask(maskers=...)`
- `Pipeline.lemma_full` → `lemmatize_verb`
- `ModelPack.tag`, `tag_tokens`, `tag_batch`, `tag_words` → `tags`,
  `tags_batch`, `tags_from_words`
- Rust: `ModelPack::tag_text`, `tag_text_items`, `tag_text_batch`, `tag_words`
  → `tags`, `tags_batch`, `tags_from_words`
- Rust: `Pipeline::word_tokenize`, `word_tokenize_with` → `tokenize`
- Rust: `Pipeline::mask_stages` → `mask`; `normalize_stages` →
  `normalize_with`; `lemma_full` → `lemmatize_verb`
- `hub.pull_pack`, `hub.cached_pack`, `hub.path_pack` → `hub.pull`, `cached`,
  `path`
- `rishe.word_tokenize` → `rishe.tokenize`

## [0.10.0] - 2026-11-07

### Added

- `rishe-embed-fa-300-v2`, a second first-party embedding pack: Word2Vec
  skip-gram, 300 dimensions, 68,144 words, trained on 150,000 Persian Wikipedia
  articles (3,830,930 sentences, 57,661,561 tokens; minimum count 20, 15 epochs).
  On ParsiNLU entailment it lifts token coverage from 82.6% to 97.2%, macro F1
  from 0.2438 to 0.2985 and the random-baseline separation gap from 0.3250 to
  0.3347.
- Compact binary embedding format `.rvec` (version 1): an `RVEC` header records
  the version, storage width, dimension and vocabulary size, then a
  length-prefixed vocabulary and the vector payload. `Embeddings::load` detects
  the format by its magic bytes and decodes both `f32` and `f16` files into the
  same in-memory layout. The v2 pack ships an `f16` file of 41,849,368 bytes
  next to the 236,068,629-byte text file.
- `rishe convert <input> <output> [--dtype f16|f32]` to rewrite any word2vec
  text file in the compact binary format.
- `Embeddings::save` and `Embeddings::dtype` (with a `DType` enum) in Rust, and
  a read-only `dtype` property on the Python `Embeddings` class.
- Hub entry for `rishe-embed-fa-300-v2` with a pinned SHA-256.
- `docs/MODEL_CARD_0.10.0.md` for the v2 pack.

### Changed

- `rishe-embed-fa-300-v2` is now the default model for `rishe embed`,
  `rishe similar`, `rishe similarity` and `rishe serve --model`; the 0.9.x pack
  remains available as `rishe-embed-fa-300`.
- Benchmarks, READMEs, the CLI reference and the hub listing quote the v2
  numbers and both pack formats.
- `.vec` and `.rvec` paths are accepted wherever an embedding file is expected.

## [0.9.1] - 2026-11-06

### Added

- `CHANGELOG.md` covering the full project history.
- `docs/RELEASING.md`, a version-independent release checklist.
- CC BY-SA 4.0 `LICENSE` files inside the embedding packs, with the manifests
  and zips regenerated so the checksums cover them.
- The model repository README now documents the embedding pack and its license.

### Changed

- The 0.9.0 model card describes the training recipe inline instead of pointing
  at a private training notebook.
- Pack `MODEL_CARD.md` files reference both `LICENSE` and `NOTICE.md`.
- Embedding pack zips are re-issued with the license file; payload `.vec` files
  and their hashes are unchanged.

### Removed

- `docs/PUBLISHING_0.9.0.md` (superseded by `docs/RELEASING.md`).

### Fixed

- Removed a documentation reference to a notebook that is not in the public
  repository.
- Embedding pack checksums no longer omit the license file.

## [0.9.0] - 2026-11-05

### Added

- First-party Persian embeddings, `rishe-embed-fa-300`: Word2Vec skip-gram,
  300 dimensions, 38,864 words, trained on 30,000 Wikipedia articles.
- `Embeddings` class in Rust and Python with word/sentence vectors,
  cosine similarity and nearest neighbours.
- Hub registration for `rishe-embed-fa-300` with a pinned SHA-256, plus
  `rishe hub path`.
- CLI commands `rishe embed`, `rishe similar` and `rishe similarity`, and
  `serve --model` support for them.
- Embedding model card, benchmark tables, `examples/embeddings.py` and
  `scripts/compare_embeddings.py`.
- Real-pack smoke tests gated by `RISHE_EMBEDDINGS_PACK` and
  `RISHE_TEST_EMBEDDINGS_PACK`.

### Changed

- Documentation leads with the first-party model; Hazm's fastText model remains
  available as an external comparison download.
- Docs, READMEs and `NOTICE.md` state the CC BY-SA 4.0 model license separately
  from the MIT/Apache-2.0 code license.

### Fixed

- The 30k embedding pack's `NOTICE.md` named the wrong Wikipedia config.
- The model card no longer claims `sentence_vector()` returns a normalized
  vector; only `sentence_similarity()` normalizes.

## [0.8.1] - 2026-11-04

### Added

- `rishe hub` with `pull`, `list`, `info`, `remove` and `verify`, backed by
  checksum-verified Hugging Face downloads.
- `rishe serve`, a dependency-free JSON-lines server with optional model packs.
- Golden-output benchmark CI (`scripts/benchmark_golden.py`).
- Optional `cuda` and `metal` features via tract's GPU runtimes.
- `--locked` CI, package-specific wheels and a single universal root manifest.

### Changed

- Release matrix no longer builds `aarch64-unknown-linux-gnu`; the
  `aarch64-unknown-linux-musl` wheel remains.
- README and CLI docs cover hub and serve.

### Fixed

- Crate publish is idempotent and checks crates.io before uploading.

## [0.8.0] - 2026-11-03

### Added

- `depparse-compact`, a compact Persian dependency parser trained on UD Persian
  Seraji and PerDT, exported to ONNX.
- `Pipeline.depparse` / `depparse_batch`, `rishe::models::Dependency`, Python
  bindings and CLI `rishe depparse`.
- Single-root Chu-Liu/Edmonds decoding and head-conditioned relation scoring.
- Parser model card, three-seed results and UD CC BY-SA 4.0 attribution.

## [0.7.1] - 2026-11-02

### Added

- Jalali dates exposed through the Python API (`JalaliDate`, `parse_date`).

## [0.7.0] - 2026-11-01

### Added

- `rishe` command-line interface: normalize, tokenize, lemmatize, mask and date,
  with `--json` output and stdin support.
- Jalali date parsing and Gregorian conversion in the Rust core.
- macOS 15 wheels.

### Changed

- Simplified the core, Python bindings and release automation.
- Dropped the docs-site configuration; documentation stays as Markdown.

## [0.6.4] - 2026-10-31

### Added

- Persian README.
- Python CI job alongside the Rust matrix.

## [0.6.3] - 2026-10-30

### Added

- CI model-pack fixture and cache verification.

## [0.6.2] - 2026-10-29

### Changed

- Model-pack developer experience: hub cache handling and clearer CLI errors.

## [0.6.1] - 2026-10-28

### Added

- Pack task validation and batch APIs.

## [0.6.0] - 2026-10-27

### Added

- `Pipeline` model-pack integration: POS, NER and ezafe packs can be attached to
  one pipeline object.

## [0.5.1] - 2026-10-26

### Added

- Package-specific readmes for PyPI (`README.pypi.md`) and crates.io
  (`README.crates.md`).

## [0.5.0] - 2026-10-25

### Added

- ONNX model-pack runtime built on tract, with no ONNX Runtime dependency.
- Compact POS, NER and ezafe packs, `ModelPack` batch APIs and model benchmarks.

### Removed

- mkdocs site configuration.

## [0.4.0] - 2026-10-23

### Added

- Static word and sentence embeddings (`Embeddings`) with cosine similarity,
  sentence pooling and nearest neighbours.
- Explicit model downloads through `rishe.hub`.
- Embedding tests and a Rishe-vs-Hazm embedding benchmark.

## [0.3.0] - 2026-10-22

### Added

- Maskers (URL, email, mention, hashtag, emoji, HTML, digits, punctuation),
  text augmentation, keyword extraction, transliteration and token utilities.
- Python CLI, Hazm-compatible API shims and corpus readers (CoNLL-U, PEYMA,
  plain text).
- API, CLI and Hazm compatibility docs, plus examples.

## [0.2.0] - 2026-10-21

### Added

- Morphology suite: verb conjugation, inflection, spell correction, stemming,
  stopwords and token splitting.
- Stopword and word-frequency data, and feature benchmarks against Hazm.

## [0.1.1] - 2026-10-20

### Added

- Batch-throughput benchmark and a documented limitations section.

### Changed

- README rewritten around benchmark results, with a banner.

### Fixed

- crates.io metadata: five valid keywords and categories.
- Crate publish is idempotent.

## [0.1.0] - 2026-10-20

### Added

- Initial release: normalization, sentence boundary detection, tokenization,
  morphology, ezafe detection, colloquial-to-formal rewriting and
  detokenization.
- Hazm-derived noun, verb and informal-word lexicons.
- Python bindings (abi3) and a Rust core.
- Benchmark suite and dual MIT/Apache-2.0 licensing.

[Unreleased]: https://github.com/MahdiAlikhah/rishe/compare/v1.0.4...HEAD
[1.0.4]: https://github.com/MahdiAlikhah/rishe/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/MahdiAlikhah/rishe/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/MahdiAlikhah/rishe/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/MahdiAlikhah/rishe/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.4...v1.0.0
[0.11.4]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.3...v0.11.4
[0.11.3]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.2...v0.11.3
[0.11.2]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.1...v0.11.2
[0.11.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.4...v0.7.0
[0.6.4]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MahdiAlikhah/rishe/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/MahdiAlikhah/rishe/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/MahdiAlikhah/rishe/releases/tag/v0.1.0
