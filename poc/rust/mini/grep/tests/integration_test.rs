#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::str::contains;

    #[test]
    fn grep_str_finds_lines() {
        let mut cmd = Command::cargo_bin("grep").unwrap();
        cmd.args(["-s", "banana", "-p", "tests/testdata"]);
        cmd.assert().success().stdout(contains("banana"));
    }
}
