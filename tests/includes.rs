mod support;
use std::{fs, process::Command};
use support::Case;

fn file(case: &Case, path: &str, source: &str) {
    let path = case.0.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

#[test]
fn includes_expand_in_order_relative_to_the_including_file_once() {
    let case = Case::new(
        r#"
wt "before";
include "lib/left.rusk";
include "lib/right.rusk";
include "lib/../lib/common.rusk";
wt twice(3),7;
"#,
    );
    file(
        &case,
        "lib/common.rusk",
        "wt \"common\"; fn twice(n:Int)->Int { return n*2; }",
    );
    file(
        &case,
        "lib/left.rusk",
        "include \"common.rusk\"; wt \"left\";",
    );
    file(
        &case,
        "lib/right.rusk",
        "include \"common.rusk\"; wt \"right\";",
    );
    let out = case.run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"before\ncommon\nleft\nright\n6 7\n");
    let dependencies = Command::new(env!("CARGO_BIN_EXE_rusk"))
        .arg("dependencies")
        .arg(case.0.join("main.rusk"))
        .output()
        .unwrap();
    assert!(dependencies.status.success());
    let paths = String::from_utf8(dependencies.stdout).unwrap();
    assert_eq!(paths.lines().count(), 4);
    for name in ["main.rusk", "common.rusk", "left.rusk", "right.rusk"] {
        assert!(paths.contains(name), "{paths}");
    }
}

#[test]
fn include_failures_keep_original_source_locations() {
    let case = Case::new("include \"lib/a.rusk\";");
    file(&case, "lib/a.rusk", "include \"b.rusk\";");
    file(&case, "lib/b.rusk", "include \"a.rusk\";");
    let out = case.compile();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        error.contains("include cycle") && error.contains("a.rusk") && error.contains("b.rusk:1:"),
        "{error}"
    );
    assert!(!case.0.join("main.cpp").exists());
    file(&case, "lib/b.rusk", "wt missing;");
    let out = case.compile();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        error.contains("b.rusk:1:") && error.contains("unknown variable"),
        "{error}"
    );
    file(&case, "lib/b.rusk", "wt 1/0;");
    let out = case.run("");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("b.rusk:1:"));
}

#[test]
fn missing_and_nested_includes_fail_before_writing_output() {
    for source in [
        "include \"missing.rusk\";",
        "fn f() { include \"missing.rusk\"; }",
        "{ include \"missing.rusk\"; }",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("main.rusk:1:"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!case.0.join("main.cpp").exists());
    }
}

#[test]
fn union_find_library_uses_structs_and_deep_copy() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/beta02/library/union_find.rusk"
    );
    let source = format!(
        r#"
include {root:?};
let uf:=UnionFind(4); uf.unite(0,1);
let detached = uf; detached.unite(1,2);
wt uf.same(0,1),uf.same(0,2),detached.same(0,2);
"#
    );
    let out = Case::new(&source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"Yes No Yes\n");
}

#[cfg(unix)]
#[test]
fn include_aliases_through_symlinks_load_once() {
    let case = Case::new("include \"original.rusk\"; include \"alias.rusk\";");
    file(&case, "original.rusk", "wt 1;");
    std::os::unix::fs::symlink(case.0.join("original.rusk"), case.0.join("alias.rusk")).unwrap();
    let out = case.run("");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"1\n");
}
