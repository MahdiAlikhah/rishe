use std::process::Command;

fn rishe() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rishe"))
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
fn lists_hub_packs() {
    let output = stdout(rishe().args(["hub", "list"]));
    assert!(output.contains("depparse-compact\t"));
    assert!(output.contains("rishe-embed-fa-300\t"));
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
    assert_eq!(missing.status.code(), Some(2));
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
        assert_eq!(near_miss.status.code(), Some(2));
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
    assert_eq!(output.status.code(), Some(2));
}
