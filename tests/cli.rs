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
        "{\"jalali\":{\"year\":1402,\"month\":5,\"day\":12},\"gregorian\":{\"year\":2023,\"month\":8,\"day\":3},\"iso\":\"2023-08-03\"}"
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
fn masks_with_the_current_and_legacy_flags() {
    let current = stdout(rishe().args(["mask", "به https://a.b برو", "--maskers", "url"]));
    let legacy = stdout(rishe().args(["mask", "به https://a.b برو", "--stages", "url"]));
    assert_eq!(current, "به <URL> برو");
    assert_eq!(legacy, current);
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

    let neighbours =
        stdout(rishe().args(["similar", "کتاب", "--model", model, "--top", "2", "--json"]));
    assert!(neighbours.starts_with("[{\"word\":"));
    assert_eq!(neighbours.matches("\"score\"").count(), 2);

    let missing = rishe()
        .args(["similar", "ناموجود", "--model", model])
        .output()
        .expect("run rishe");
    assert_eq!(missing.status.code(), Some(1));
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
        vec!["serve", "--frobnicate"],
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
    let model = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mini-embed.vec");
    for args in [
        vec!["similar", "کتاب", "--model", "missing-model.vec"],
        vec!["similar", "کتاب", "--top", "0", "--model", model],
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
