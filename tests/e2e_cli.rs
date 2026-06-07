use assert_cmd::Command;

#[test]
fn test_hello_world() {
    let mut cmd = Command::cargo_bin("craftman").unwrap();
    cmd.assert().success().stdout("Hello, world!\n");
}
