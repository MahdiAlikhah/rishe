#![cfg(feature = "hub")]

use std::path::Path;
use std::process::Command;

fn rishe() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rishe"))
}

/// A scratch directory under `target/`, removed on drop.
///
/// Tests that write files use one of these so a failed assertion cannot leave
/// stray files behind; everything lives under `target/`, never the repo root.
struct ScratchDir(std::path::PathBuf);

impl ScratchDir {
    fn new(name: &str) -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-scratch")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("create scratch dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn stdout(command: &mut Command) -> String {
    let output = command.output().expect("run rishe");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn normalizes_and_tokenizes() {
    assert_eq!(stdout(rishe().args(["normalize", "مي كنم"])), "می‌کنم");
    assert_eq!(
        stdout(rishe().args(["tokenize", "سلام دنیا", "--json"])),
        "[\"سلام\",\"دنیا\"]"
    );
}

#[test]
fn parses_jalali_dates() {
    assert_eq!(
        stdout(rishe().args(["date", "۱۲ مرداد ۱۴۰۲"])),
        "2023-08-03"
    );
    assert_eq!(
        stdout(rishe().args(["date", "1402/05/12", "--json"])),
        "{\"gregorian\":{\"day\":3,\"month\":8,\"year\":2023},\"iso\":\"2023-08-03\",\"jalali\":{\"day\":12,\"month\":5,\"year\":1402}}"
    );
}

#[test]
fn serve_rejects_an_oversized_line_and_keeps_serving() {
    // `read_until` grows its buffer until it finds a newline, so a client that
    // streams bytes and never sends one used to grow the server without bound.
    // The oversized line must be answered with the error envelope, its remainder
    // discarded, and the next real request still served.
    use std::io::Write;
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    let mut payload = vec![b'x'; (1 << 20) + 1];
    payload.push(b'\n');
    payload.extend_from_slice("{\"command\":\"tokenize\",\"text\":\"کتاب خوب است\"}\n".as_bytes());
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&payload)
        .expect("write stdin");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected one error and one reply: {stdout}");
    assert_eq!(
        lines[0],
        "{\"error\":\"request line is too long\",\"ok\":false}"
    );
    assert_eq!(
        lines[1],
        "{\"ok\":true,\"result\":[\"کتاب\",\"خوب\",\"است\"]}"
    );
}

#[test]
fn serves_json_lines() {
    use std::io::Write;
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all("{\"command\":\"normalize\",\"text\":\"مي كنم\"}\n".as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "{\"ok\":true,\"result\":\"می‌کنم\"}"
    );
}

#[test]
fn serve_treats_a_wrong_typed_field_as_absent() {
    // docs/serve-schema.md: a field of the wrong JSON type counts as absent, so
    // an older client cannot get a different result from the two servers.
    use std::io::Write;
    let lines = concat!(
        "{\"command\":\"mask\",\"text\":\"info@a.co\",\"maskers\":\"email\"}\n",
        "{\"command\":\"mask\",\"text\":\"info@a.co\",\"maskers\":[3,\"email\"]}\n",
        "{\"command\":\"mask\",\"text\":\"info@a.co\",\"mask_token\":7}\n",
    );
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(lines.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    let responses: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect();
    assert_eq!(responses.len(), 3);
    for response in responses {
        assert_eq!(response["ok"], true, "unexpected error: {response}");
        assert!(response["result"].is_string());
    }
}

#[test]
fn serve_accepts_the_legacy_stages_field() {
    // docs/serve-schema.md: protocol 1 still accepts "stages" for 0.x clients.
    use std::io::Write;
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(
            "{\"command\":\"mask\",\"text\":\"info@a.co\",\"stages\":[\"email\"]}\n".as_bytes(),
        )
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    let response: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
            .expect("one JSON object");
    assert_eq!(response["ok"], true, "unexpected error: {response}");
    assert_eq!(response["result"], "<EMAIL>");
}

#[test]
fn serve_answers_the_version_handshake() {
    use std::io::Write;
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all("{\"command\":\"version\"}\n".as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    // Parse the envelope: key order and whitespace are not part of protocol 1.
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");
    assert_eq!(response["ok"], true);
    assert_eq!(response["result"]["name"], "rishe");
    assert_eq!(response["result"]["protocol"], 1);
    assert_eq!(response["result"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn serve_survives_a_reader_that_closes_the_pipe() {
    // `rishe serve | head -1` is the ordinary way to read one reply. The reader
    // closes stdout after the first line, and the server must end cleanly instead
    // of panicking on the write and exiting 101.
    use std::io::{BufRead, BufReader, Write};
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    let mut stdin = child.stdin.take().expect("stdin");
    // Keep feeding so the server is still writing when the pipe goes away.
    let feeder = std::thread::spawn(move || {
        let line = "{\"command\":\"version\"}\n";
        for _ in 0..20_000 {
            if stdin.write_all(line.as_bytes()).is_err() {
                break;
            }
        }
    });

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let mut first = String::new();
    reader.read_line(&mut first).expect("first reply");
    assert!(first.contains("\"ok\":true"), "{first}");
    // Drop the read end: the next write from the server hits a closed pipe.
    drop(reader);

    let status = child.wait().expect("wait");
    feeder.join().ok();
    assert!(
        !matches!(status.code(), Some(101)),
        "a closed stdout must not panic the server (exit {:?})",
        status.code()
    );
}

#[test]
fn serve_survives_a_non_utf8_line() {
    use std::io::Write;
    let mut child = rishe()
        .arg("serve")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe serve");
    let mut stdin = child.stdin.take().expect("stdin");
    stdin
        .write_all(b"\xff\xfe\n{\"command\":\"version\"}\n")
        .expect("write stdin");
    drop(stdin);
    let output = child.wait_with_output().expect("wait");
    // A bad line is answered, not fatal: the server keeps serving and exits 0.
    assert_eq!(output.status.code(), Some(0));
    let lines: Vec<&[u8]> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), 2, "one response per request line");
    let first: serde_json::Value = serde_json::from_slice(lines[0]).expect("error envelope");
    assert_eq!(first["ok"], false);
    assert!(first["error"]
        .as_str()
        .unwrap_or_default()
        .contains("UTF-8"));
    let second: serde_json::Value = serde_json::from_slice(lines[1]).expect("handshake");
    assert_eq!(second["ok"], true);
    assert_eq!(second["result"]["protocol"], 1);
}

#[test]
fn masks_with_the_maskers_flag_and_rejects_the_removed_one() {
    let current = stdout(rishe().args(["mask", "به https://a.b برو", "--maskers", "url"]));
    assert_eq!(current, "به <URL> برو");
    let removed = rishe()
        .args(["mask", "به https://a.b برو", "--stages", "url"])
        .output()
        .expect("run rishe");
    assert_eq!(removed.status.code(), Some(2));
}

#[test]
fn maskers_flag_replaces_the_defaults_and_repeats_append() {
    // Choosing a chain has to replace the defaults: `--maskers stopwords` used to
    // leave URLs and emails in the clear, so the flag that looked like a filter
    // published the text it was asked to clean.
    let line = "به https://a.b و info@a.co برو";
    let stopwords_only = stdout(rishe().args(["mask", line, "--maskers", "stopwords"]));
    assert!(stopwords_only.contains("https://a.b"), "{stopwords_only}");
    assert!(stopwords_only.contains("info@a.co"), "{stopwords_only}");

    // One value per flag, appended: the last flag used to win outright.
    let both = stdout(rishe().args(["mask", line, "--maskers", "url", "--maskers", "email"]));
    assert_eq!(both, "به <URL> و <EMAIL> برو");
}

#[test]
fn masking_never_corrupts_text_that_shares_a_shield() {
    // The chain swapped `<URL>`/`<EMAIL>` for the fixed pairs `ژژ`/`چچ`, which
    // real words contain, so masking invented placeholders out of the text.
    for text in ["چچ", "غژژ", "فژژ", "سلام ژژ دنیا"] {
        assert_eq!(stdout(rishe().args(["mask", text])), text, "{text:?}");
    }
    // And the caller's mask token is never fed back through the chain.
    assert_eq!(
        stdout(rishe().args(["mask", "info@a.co", "--mask-token", "[MASK]"])),
        "[MASK]"
    );
}

#[test]
fn a_masker_deleting_characters_cannot_fabricate_a_placeholder() {
    // The stand-ins were runs of ZWNJ. `non_persian` keeps ZWNJ but deletes what
    // sits between two runs, so it joined them into a third run that matched a
    // stand-in, and the swap-back turned real text into a placeholder the input
    // never had. The stand-in is now a letter absent from the text, so no
    // deletion can build one.
    let zwnj = "\u{200c}";
    for (text, args) in [
        (format!("خانه{zwnj}https://a.com"), vec!["mask"]),
        (format!("می{zwnj}۱۲۳{zwnj}ساله"), vec!["mask"]),
    ] {
        let out = stdout(rishe().args(&args).arg(&text));
        assert!(!out.contains("<EMAIL>"), "{text:?} produced {out:?}");
    }

    // The narrow case: deleting the letters between ZWNJ runs.
    let text = format!("a{zwnj}xx{zwnj}b");
    let out = stdout(rishe().args(["mask", &text, "--maskers", "non_persian"]));
    assert!(
        !out.contains("<URL>") && !out.contains("<EMAIL>"),
        "{out:?}"
    );
}

#[test]
fn a_capitalised_link_is_still_a_link() {
    // The prefix table was compared case-sensitively, so `HTTPS://` and `WWW.`
    // were never recognised and the masker left them in the clear: a privacy
    // tool that silently keeps the thing it was asked to remove.
    for link in [
        "HTTPS://a.com",
        "HTTP://a.com",
        "FTP://a.com/x",
        "WWW.A.COM",
    ] {
        assert_eq!(
            stdout(rishe().args(["mask", link, "--maskers", "url"])),
            "<URL>",
            "{link}"
        );
    }
    // Inside a sentence, and without eating the words around it.
    assert_eq!(
        stdout(rishe().args(["mask", "به WWW.A.COM برو", "--maskers", "url"])),
        "به <URL> برو"
    );
    // A word that merely contains the letters is not a link.
    assert_eq!(
        stdout(rishe().args(["mask", "myhttps://a.com", "--maskers", "url"])),
        "myhttps://a.com"
    );
}

#[test]
fn a_whitespace_mask_token_does_not_eat_the_rest_of_the_line() {
    // Shielding the caller's token rewrote every space to ZWNJ, which is not
    // whitespace, so the whole-word rules ran to the end of the line.
    assert_eq!(
        stdout(rishe().args([
            "mask",
            "x https://a.com y",
            "--maskers",
            "url",
            "--mask-token",
            " "
        ])),
        "x   y"
    );
    assert_eq!(
        stdout(rishe().args([
            "mask",
            "a @user y",
            "--maskers",
            "mention",
            "--mask-token",
            " "
        ])),
        "a   y"
    );
}

#[test]
fn convert_refuses_to_overwrite_its_own_input() {
    // `save` truncates the output, so `convert a.vec a.vec` destroyed the only
    // copy of the plain-text model and still printed "wrote".
    let scratch = ScratchDir::new("cli-convert-inplace");
    let model = scratch.path().join("model.vec");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mini-embed.vec"),
        &model,
    )
    .expect("copy fixture");
    let model = model.to_str().expect("utf-8 path").to_owned();
    let before = std::fs::read(&model).expect("read model");

    let output = rishe()
        .args(["convert", &model, &model])
        .output()
        .expect("run rishe");
    assert_eq!(
        output.status.code(),
        Some(2),
        "in-place convert must be a usage error"
    );
    assert_eq!(
        std::fs::read(&model).expect("read model"),
        before,
        "the input file was modified"
    );

    // The same file spelled differently is still the same file. A `.` segment is
    // portable; a `file/../file` spelling is not, because Linux refuses to
    // canonicalise through a regular file while Windows normalises it lexically.
    let dotted = format!(
        "./{}",
        Path::new(&model)
            .file_name()
            .expect("file name")
            .to_string_lossy()
    );
    let output = rishe()
        .current_dir(Path::new(&model).parent().expect("parent"))
        .args(["convert", &model, &dotted])
        .output()
        .expect("run rishe");
    assert_eq!(
        output.status.code(),
        Some(2),
        "a relative spelling must be refused"
    );
}

#[test]
fn lists_hub_packs() {
    let output = stdout(rishe().args(["hub", "list"]));
    assert!(output.contains("depparse-compact\t"));
    assert!(output.contains("rishe-embed-fa-300-v2\t"));
    assert!(output.contains("rishe-embed-fa-300\t"));
}

#[test]
fn converts_embeddings_to_binary() {
    let model = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mini-embed.vec");
    let scratch = ScratchDir::new("cli-convert");
    let binary = scratch.path().join("cli-convert.rvec");
    let binary = binary.to_str().expect("utf-8 path").to_owned();

    let message = stdout(rishe().args(["convert", model, &binary, "--dtype", "f16"]));
    assert!(message.contains("as f16"), "unexpected output: {message}");

    let identical = stdout(rishe().args(["similarity", "کتاب", "کتاب", "--model", &binary]));
    assert_eq!(identical, "1.000000");

    let neighbours = stdout(rishe().args([
        "similar", "کتاب", "--model", &binary, "--top", "2", "--json",
    ]));
    assert_eq!(neighbours.matches("\"score\"").count(), 2);

    let bad = rishe()
        .args(["convert", model, &binary, "--dtype", "f64"])
        .output()
        .expect("run rishe");
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn embeds_similar_and_similarity() {
    let model = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mini-embed.vec");

    let vector = stdout(rishe().args(["embed", "کتاب", "--model", model, "--json"]));
    let values = vector
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .count();
    assert_eq!(values, 8);

    let identical = stdout(rishe().args(["similarity", "کتاب", "کتاب", "--model", model]));
    assert_eq!(identical, "1.000000");

    let as_json = stdout(rishe().args(["similarity", "کتاب", "کتاب", "--model", model, "--json"]));
    assert!(
        as_json.starts_with(r#"{"score":1.0"#),
        "unexpected JSON: {as_json}"
    );

    let neighbours =
        stdout(rishe().args(["similar", "کتاب", "--model", model, "--top", "2", "--json"]));
    assert!(neighbours.starts_with("[{\"score\":"));
    assert_eq!(neighbours.matches("\"score\"").count(), 2);

    let missing = rishe()
        .args(["similar", "ناموجود", "--model", model])
        .output()
        .expect("run rishe");
    assert_eq!(missing.status.code(), Some(1));
}

#[test]
fn hub_pull_refuses_a_sha256_for_a_pack() {
    // A pack is verified against the manifest's own digests, so a caller digest has
    // nothing to apply to. The parser accepts the flag, so this is a value-level
    // failure: exit 1, not 2, matching docs/errors.md and the Python CLI.
    let output = rishe()
        .args(["hub", "pull", "pos-compact", "--sha256", &"0".repeat(64)])
        .env("RISHE_MODEL_BASE", "http://127.0.0.1:9")
        .output()
        .expect("run rishe");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("task packs do not accept --sha256"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hub_pull_refuses_a_url_with_no_file_name() {
    for url in ["https://example.com/", "https://example.com/.."] {
        let output = rishe()
            .args(["hub", "pull", url, "--sha256", &"0".repeat(64)])
            .env("RISHE_MODEL_BASE", "http://127.0.0.1:9")
            .output()
            .expect("run rishe");
        assert_eq!(output.status.code(), Some(1), "{url}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("cannot infer a filename"),
            "{url}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn hub_path_reports_the_cache_path() {
    let near_miss = rishe()
        .args(["hub", "path", "rishe-embed-fa-300"])
        .output()
        .expect("run rishe");
    if near_miss.status.success() {
        let path = String::from_utf8_lossy(&near_miss.stdout);
        assert!(path.trim_end().ends_with("rishe-embed-fa-300.vec"));
    } else {
        assert_eq!(near_miss.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&near_miss.stderr).contains("not cached"));
    }
}

#[test]
fn hub_remove_and_info_see_a_partial_download() {
    let scratch = ScratchDir::new("hub-partial");
    let home = scratch.path();
    let models = home.join("models");
    std::fs::create_dir_all(&models).expect("models dir");
    let part = models.join("rishe-embed-fa-300.vec.part");
    std::fs::write(&part, b"half a download").expect("partial file");

    // info reports the bytes already on disk instead of claiming the model is absent.
    let info = stdout(
        rishe()
            .env("RISHE_HOME", home)
            .args(["hub", "info", "rishe-embed-fa-300"]),
    );
    assert!(
        info.contains("partial"),
        "info hid the partial download: {info}"
    );
    assert!(info.contains(&part.display().to_string()), "{info}");

    // list agrees with info rather than calling the same state "missing".
    let listed = stdout(rishe().env("RISHE_HOME", home).args(["hub", "list"]));
    let row = listed
        .lines()
        .find(|line| line.starts_with("rishe-embed-fa-300\t"))
        .expect("model row");
    assert_eq!(row, "rishe-embed-fa-300\tpartial\trishe-embed-fa-300.vec");

    // remove deletes the part file, so the disk space is actually freed.
    let removed =
        stdout(
            rishe()
                .env("RISHE_HOME", home)
                .args(["hub", "remove", "rishe-embed-fa-300"]),
        );
    assert_eq!(removed.trim(), "removed");
    assert!(!part.exists(), "remove left the partial download behind");

    // With nothing cached and no part, both commands still say so.
    let again =
        stdout(
            rishe()
                .env("RISHE_HOME", home)
                .args(["hub", "remove", "rishe-embed-fa-300"]),
        );
    assert_eq!(again.trim(), "not cached");
    let missing =
        stdout(
            rishe()
                .env("RISHE_HOME", home)
                .args(["hub", "info", "rishe-embed-fa-300"]),
        );
    assert!(missing.contains("missing"), "{missing}");
}

#[test]
fn hub_remove_and_info_see_a_partial_pack() {
    let scratch = ScratchDir::new("hub-pack-partial");
    let home = scratch.path();
    let pack = home.join("models").join("pos-compact");
    std::fs::create_dir_all(&pack).expect("pack dir");
    std::fs::write(pack.join("labels.json"), b"{}").expect("partial pack file");

    // A half-pulled pack directory is not "missing": it occupies disk space.
    let info = stdout(
        rishe()
            .env("RISHE_HOME", home)
            .args(["hub", "info", "pos-compact"]),
    );
    assert!(
        info.contains("partial"),
        "info hid the partial pack: {info}"
    );
    let listed = stdout(rishe().env("RISHE_HOME", home).args(["hub", "list"]));
    assert!(
        listed.contains("pos-compact\tpartial\tpack"),
        "list disagreed with info: {listed}"
    );

    let removed = stdout(
        rishe()
            .env("RISHE_HOME", home)
            .args(["hub", "remove", "pos-compact"]),
    );
    assert_eq!(removed.trim(), "removed");
    assert!(!pack.exists(), "remove left the partial pack behind");
}

#[test]
fn hub_remove_clears_a_partial_manifest_too() {
    // `load_manifest` downloads with force into the cache root, so an interrupted
    // pack pull can leave a manifest part that no model or pack owns.
    let scratch = ScratchDir::new("hub-manifest-partial");
    let home = scratch.path();
    let models = home.join("models");
    std::fs::create_dir_all(models.join("pos-compact")).expect("pack dir");
    let manifest_part = models.join("rishe-models-manifest.json.part");
    std::fs::write(&manifest_part, b"half a manifest").expect("manifest part");

    let removed = stdout(
        rishe()
            .env("RISHE_HOME", home)
            .args(["hub", "remove", "pos-compact"]),
    );
    assert_eq!(removed.trim(), "removed");
    assert!(
        !manifest_part.exists(),
        "remove left the partial manifest behind"
    );
}

#[test]
fn reads_stdin_and_reports_bad_input() {
    let mut child = rishe()
        .arg("normalize")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn rishe");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all("مي كنم".as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "می‌کنم");

    let output = rishe()
        .args(["date", "not a date"])
        .output()
        .expect("run rishe");
    assert_eq!(output.status.code(), Some(1));
    // The Python CLI and `docs/errors.md` quote this in single quotes.
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "cannot parse a Jalali date from 'not a date'"
    );
}

#[test]
fn exits_zero_on_success_help_and_version() {
    for args in [&[][..], &["--help"][..]] {
        let output = rishe().args(args).output().expect("run rishe");
        assert_eq!(
            output.status.code(),
            Some(0),
            "rishe {args:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let expected = format!("rishe {}", env!("CARGO_PKG_VERSION"));
    for flag in ["-V", "--version"] {
        let output = rishe().arg(flag).output().expect("run rishe");
        assert_eq!(output.status.code(), Some(0), "rishe {flag}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "rishe {flag}"
        );
    }
    let normalized = rishe()
        .args(["normalize", "مي كنم"])
        .output()
        .expect("run rishe");
    assert_eq!(normalized.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&normalized.stdout).trim(), "می‌کنم");
}

#[test]
fn usage_errors_exit_two() {
    for args in [
        vec!["frobnicate"],
        vec!["tokenize", "text", "--frobnicate"],
        vec!["tokenize", "text", "--model"],
        vec!["similar", "one", "two", "--model", "missing-model.vec"],
        vec!["similarity", "one", "--model", "missing-model.vec"],
        // A bad `--top` is a malformed command line, so it is a usage error even
        // though the model does not exist: the Python binary reports it the same
        // way, and checking it after the load made the exit code depend on the cache.
        vec![
            "similar",
            "کتاب",
            "--top",
            "0",
            "--model",
            "missing-model.vec",
        ],
        vec!["serve", "--frobnicate"],
        // `--maskers ""` split to an empty list, which the engine reads as "apply
        // nothing": the text came back unmasked and the command exited 0.
        vec!["mask", "https://a.b", "--maskers", ""],
        vec!["mask", "https://a.b", "--maskers", ","],
        vec!["serve", "--maskers", ""],
        vec!["hub"],
        vec!["hub", "path"],
        vec!["hub", "frobnicate"],
    ] {
        let output = rishe().args(&args).output().expect("run rishe");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(2),
            "rishe {args:?} stderr: {stderr}"
        );
        assert!(!stderr.is_empty(), "rishe {args:?} printed no error");
        assert!(
            output.stdout.is_empty(),
            "rishe {args:?} wrote an error to stdout"
        );
    }
}

#[test]
fn runtime_errors_exit_one() {
    for args in [
        vec!["similar", "کتاب", "--model", "missing-model.vec"],
        vec!["mask", "text", "--maskers", "bogus"],
        vec!["date", "not a date"],
        vec!["pos", "text"],
        vec!["serve", "--model", "missing-model.vec"],
    ] {
        let output = rishe().args(&args).output().expect("run rishe");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "rishe {args:?} stderr: {stderr}"
        );
        assert!(!stderr.is_empty(), "rishe {args:?} printed no error");
        assert!(
            output.stdout.is_empty(),
            "rishe {args:?} wrote an error to stdout"
        );
    }
}

#[test]
fn hub_info_and_verify_report_a_missing_model_differently() {
    let scratch = ScratchDir::new("hub-info-verify");
    let home = scratch.path();

    let info = rishe()
        .env("RISHE_HOME", home)
        .args(["hub", "info", "rishe-embed-fa-300"])
        .output()
        .expect("run rishe");
    assert_eq!(info.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&info.stdout).trim(),
        "rishe-embed-fa-300\tmissing"
    );

    let verify = rishe()
        .env("RISHE_HOME", home)
        .args(["hub", "verify", "rishe-embed-fa-300"])
        .output()
        .expect("run rishe");
    assert_eq!(verify.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&verify.stdout).trim(), "invalid");
    assert!(verify.stderr.is_empty(), "verify printed a second message");

    let unknown = rishe()
        .env("RISHE_HOME", home)
        .args(["hub", "verify", "nope-model"])
        .output()
        .expect("run rishe");
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown model or pack"));
}

#[test]
fn score_rejects_a_wrong_number_of_texts() {
    // A usage error must not depend on the build: without `models` this used to
    // be reachable only as a runtime error, and with `models` the packs would
    // load before the count was checked.
    for texts in [vec!["only-a-query"], vec!["a", "b", "c"]] {
        let output = rishe()
            .arg("score")
            .args(&texts)
            .output()
            .expect("run rishe");
        assert_eq!(output.status.code(), Some(2), "for {texts:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("exactly two texts"),
            "for {texts:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[cfg(feature = "models")]
fn score_reports_a_missing_pack() {
    let output = rishe()
        .args(["score", "query", "passage"])
        .output()
        .expect("run rishe");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--pack"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg(feature = "models")]
fn score_survives_a_hostile_pack_argument() {
    // `score` loads the path straight from the command line, so a missing,
    // non-directory or wrong-task path must be an error rather than a panic or
    // a hang. The empty-string pair is checked too: the pair template still
    // wraps empty text in separators, so it used to score.
    let scratch = ScratchDir::new("score-hostile");
    let empty = scratch.path().join("empty-pack");
    std::fs::create_dir_all(&empty).expect("create empty pack");
    let a_file = scratch.path().join("not-a-dir");
    std::fs::write(&a_file, b"x").expect("write file");
    let tagging = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("tiny-pack");

    for (label, pack, expected) in [
        ("empty dir", &empty, "labels.json is missing"),
        ("file, not a dir", &a_file, "not a directory"),
        ("pos pack", &tagging, "not a reranker"),
    ] {
        let output = rishe()
            .args(["score", "--pack"])
            .arg(pack)
            .args(["query", "passage"])
            .output()
            .expect("run rishe");
        assert_eq!(output.status.code(), Some(1), "{label}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "{label}: got {stderr}");
        assert!(stderr.is_ascii(), "{label} leaked a panic: {stderr}");
    }
}
