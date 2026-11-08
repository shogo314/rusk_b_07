mod support;
use support::Case;

fn diagnostic(source: &str) -> String {
    let case = Case::new(source);
    let mut message = None;
    for stage in ["hir", "lir", "cpp"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(!output.status.success(), "{source}");
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(
            text.contains("noescape modifier is here") && text.contains("parameter declared here"),
            "{text}"
        );
        assert!(!case.0.join("main.cpp").exists());
        if let Some(previous) = &message {
            assert_eq!(previous, &text);
        }
        message = Some(text);
    }
    message.unwrap()
}
fn ordered(message: &str, parts: &[&str]) {
    let mut remaining = message;
    for part in parts {
        let index = remaining
            .find(part)
            .unwrap_or_else(|| panic!("missing {part:?} in {message}"));
        remaining = &remaining[index + part.len()..];
    }
}

#[test]
fn aliases_fields_and_call_returns_show_source_names_and_locations() {
    let message = diagnostic(
        "struct Box{value:Int;}\nfn getter(b:Box)->Int{return b.value;}\nfn bad(noescape x:Int)->Int{\n let alias:=x;\n let box:=Box{value:alias};\n let picked:=getter(box);\n return picked;\n}\nwt bad(1);",
    );
    assert!(
        message.contains("main.rusk:7:2: noescape parameter 'x' may escape"),
        "{message}"
    );
    ordered(
        &message,
        &[
            "sharing path:",
            "main.rusk:4:6: binding 'alias' declared here",
            "shared value stored in field 'value' of 'Box'",
            "main.rusk:5:6: binding 'box' declared here",
            "main.rusk:6:14: result of 'getter' may share argument 1 through field 'value' of 'Box'",
            "main.rusk:6:6: binding 'picked' declared here",
        ],
    );
    let message = diagnostic(
        "fn bad(noescape x:Int,items:Vec<Int>){\n let alias:=x;\n items.push_back(alias);\n}let items:Vec<Int>:=[];bad(1,items);",
    );
    assert!(
        message.contains("main.rusk:3:") && message.contains("argument 2 of 'push_back'"),
        "{message}"
    );
    ordered(
        &message,
        &[
            "sharing path:",
            "main.rusk:2:6: binding 'alias' declared here",
        ],
    );
    let message = diagnostic(
        "struct Box{value:Int;}fn bad(noescape x:Int,b:Box){let alias:=x;b.value:=alias;}bad(1,Box{value:0});",
    );
    assert!(
        message.contains("cannot prove noescape")
            && message.contains("field store to field 'value' of 'Box'")
            && message.contains("destination is not proven private"),
        "{message}"
    );
    ordered(
        &message,
        &["sharing path:", "binding 'alias' declared here"],
    );
}

#[test]
fn rebinding_joins_and_shadowing_do_not_name_dead_aliases() {
    let message = diagnostic(
        "fn bad(noescape x:Int,flag:Bool)->Int{\n let discarded:=x;\n let alias:=0;\n if flag{alias:=x;}\n discarded:=0;\n {let alias:=123;wt alias;}\n return alias;\n}\nwt bad(1,true);",
    );
    let path = message.split("\nsharing path:\n").nth(1).unwrap();
    assert!(
        path.contains("main.rusk:4:") && path.contains("binding 'alias' assigned here"),
        "{message}"
    );
    assert!(
        !path.contains("discarded")
            && !path.contains("main.rusk:6:")
            && !path.contains("main.rusk:3:"),
        "{message}"
    );
    let message =
        diagnostic("fn bad(noescape x:Int)->Int{let alias:Int;alias:=x;return alias;}wt bad(1);");
    assert!(
        message.contains("binding 'alias' assigned here"),
        "{message}"
    );
}

#[test]
fn payloads_contained_results_and_captures_keep_their_routes() {
    for (source, parts) in [
        (
            "enum Wrapped{Full(Int),Empty}fn bad(noescape x:Int)->Int{let wrapped:=Wrapped::Full(x);let alias:=match wrapped{Full(c)=>c,Empty=>0,};return alias;}wt bad(1);",
            vec![
                "payload 1 of variant 'Full'",
                "binding 'wrapped' declared here",
                "shared payload 1 of variant 'Full' retrieved",
                "binding 'alias' declared here",
            ],
        ),
        (
            "fn pack(x:Int)->(Int,Int){return (x,0);}fn bad(noescape x:Int)->Int{let packed:=pack(x);let alias:=packed[0];return alias;}wt bad(1);",
            vec![
                "result of 'pack' may share argument 1; contained at field",
                "binding 'packed' declared here",
                "shared field",
                "binding 'alias' declared here",
            ],
        ),
        (
            "fn invoke<F>(callback:F)->Int{return callback();}fn bad(noescape x:Int)->Int{let alias:=x;let read:=()=>alias;return invoke(read);}wt bad(1);",
            vec![
                "binding 'alias' declared here",
                "shared value stored in field",
                "binding 'read' declared here",
                "result of 'invoke' may share argument 1 through field",
            ],
        ),
        (
            "fn bad(noescape values:Vec<Int>)->Int{let alias:=values[0];return alias;}wt bad([1]);",
            vec![
                "binding 'values' declared here",
                "through element of 'Vec<Int>'",
                "binding 'alias' declared here",
            ],
        ),
    ] {
        let message = diagnostic(source);
        ordered(&message, &parts);
    }
}

#[test]
fn unknown_origins_explain_heap_invalidation_or_opaque_return_without_inventing_aliases() {
    let message = diagnostic(
        "struct Box{value:Int;}fn bad(noescape x:Int)->Int{\n let box:=Box{value:0};\n box.value:=x;\n let alias:=box.value;\n return alias;\n}\nwt bad(1);",
    );
    assert!(
        message.contains("cannot prove noescape for parameter 'x'"),
        "{message}"
    );
    ordered(
        &message,
        &[
            "uncertain sharing path:",
            "main.rusk:3:",
            "heap sharing may be invalidated by this field store",
            "shared field 'value' of 'Box' retrieved",
            "main.rusk:4:6: binding 'alias' declared here",
        ],
    );
    assert!(!message.contains("\nsharing path:\n"));
    let message = diagnostic(
        r#"extern fn foreign(x:Int)->Int{cpp_name:"foreign",code:"auto foreign(auto x,Loc){return x;}",}fn bad(noescape x:Int)->Int{let alias:=foreign(0);return alias;}wt bad(1);"#,
    );
    let path = message.split("uncertain sharing path:").nth(1).unwrap();
    assert!(
        path.contains("return sharing of 'foreign' is unknown") && !path.contains("binding 'x'"),
        "{message}"
    );
}

#[test]
fn long_diagnostic_paths_are_bounded_without_changing_escape_certainty() {
    let mut source = "fn bad(noescape x:Int)->Int{let a0:=x;".to_string();
    for i in 1..40 {
        source += &format!("let a{i}:=a{};", i - 1);
    }
    source += "return a39;}wt bad(1);";
    let message = diagnostic(&source);
    assert!(
        message.contains("noescape parameter 'x' may escape")
            && message.contains("additional sharing steps omitted"),
        "{message}"
    );
    assert!(
        message.contains("binding 'a0' declared here")
            && !message.contains("binding 'a39' declared here"),
        "{message}"
    );
}

#[test]
fn source_binding_metadata_is_preserved_for_unannotated_bindings_and_captures() {
    let case = Case::new("let seed:=1;let f:=()=>seed+1;wt f();");
    for stage in ["hir", "lir"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let ir = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(
            ir.contains("DescribeBinding")
                && ir.contains("captured: true")
                && ir.contains("captured: false")
                && ir.contains("text: \"seed\""),
            "{ir}"
        );
    }
    let output = case.run("");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"2\n");
}

#[test]
fn analysis_limits_are_reported_as_uncertain_instead_of_a_proven_alias_chain() {
    let types = vec!["Int"; 65].join(",");
    let fields = vec!["x"; 65].join(",");
    let message = diagnostic(&format!(
        "fn bad(noescape x:Int)->({types}){{let packed:=({fields});return packed;}}bad(1);"
    ));
    assert!(
        message.contains("cannot prove noescape for parameter 'x'")
            && message.contains("analysis limits")
            && message.contains("uncertain sharing path:"),
        "{message}"
    );
    assert!(!message.contains("\nsharing path:\n"), "{message}");
}
