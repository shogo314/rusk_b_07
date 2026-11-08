mod support;
use support::Case;

#[test]
fn early_runtime_failure_preserves_diagnostics_when_stdin_is_not_consumed() {
    let case = Case::new("let n:Option<Int>:=None;wt n?!;");
    let output = case.run(&"1 ".repeat(1_000_000));
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("main.rusk:") && error.contains("absent Option value"),
        "{error}"
    );
}
