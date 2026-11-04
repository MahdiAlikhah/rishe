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
