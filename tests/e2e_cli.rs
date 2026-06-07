use assert_cmd::Command;

#[test]
fn test_help_shows_options() {
    let mut cmd = Command::cargo_bin("craftman").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("--model"))
        .stdout(predicates::str::contains("--url"));
}
