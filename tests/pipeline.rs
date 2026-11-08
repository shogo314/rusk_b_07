mod support;
use support::Case;

#[test]
fn every_stage_is_inspectable_and_ir_is_target_independent() {
    let case = Case::new(
        r#"
fn twice(x:Int)->Int{return x*2;}
fn twice(x:Str)->Str{return x+x;}
let x:=3;{let x:="a";wt twice(x);}
let chosen:=if x<5{twice(x)}else{0};
for i in 0..3{if i==1{continue;}wt chosen+i;}
"#,
    );
    for stage in ["cst", "ast", "hir", "lir", "cpp"] {
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(
            out.status.success(),
            "{stage}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(!dump.is_empty());
        if matches!(stage, "hir" | "lir") {
            assert!(dump.contains("function:") && dump.contains("Local("));
            assert!(
                !dump.contains("make_obj<") && !dump.contains("std::") && !dump.contains("rusk_fn")
            );
            let plain = case.compile_with_options(&["--emit", stage, "--no-opt"]);
            assert!(plain.status.success());
            assert_eq!(
                dump,
                std::fs::read_to_string(case.0.join("main.cpp")).unwrap()
            );
        }
        if stage == "hir" {
            assert!(dump.contains("If {") && dump.contains("Loop {") && dump.contains("Continue("));
        }
        if stage == "lir" {
            assert!(
                dump.contains("Branch {") && dump.contains("Jump(") && dump.contains("cleanup:")
            );
            assert!(!dump.contains("If {") && !dump.contains("Loop {") && !dump.contains("Scope("));
        }
        if stage == "cpp" {
            assert!(dump.contains("goto rusk_block_") && !dump.contains("while (true)"));
        }
    }
    let out = case.run("");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"aa\n6\n8\n");
}

#[test]
fn jump_edges_release_borrows_and_return_values_keep_their_owners() {
    let source = include_str!("../examples/beta05/control_flow.rusk");
    let case = Case::new(source);
    let out = case.run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"b c\nab!\nab!? 2\n13\n");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert_eq!(out.stdout, plain.stdout);
    assert_eq!(out.stderr, plain.stderr);
    assert_eq!(out.status.code(), plain.status.code());
}

#[test]
fn semantic_failures_are_rejected_before_hir_and_lir_output() {
    for stage in ["hir", "lir", "cpp"] {
        let case = Case::new("fn f(x:Int)->Int{return x;}wt f(true);");
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(!out.status.success());
        assert!(!case.0.join("main.cpp").exists());
    }
    let opaque = Case::new(r#"extern type Iter<Int>{cpp_name:"std::int64_t",} wt 7;"#);
    let out = opaque.run("");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"7\n");
    let case = Case::new("let x:Int;wt x;");
    assert!(
        case.compile_with_options(&["--emit", "ast"])
            .status
            .success()
    );
    assert!(
        !case
            .compile_with_options(&["--emit", "hir"])
            .status
            .success()
    );
}
