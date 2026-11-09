mod support;
use support::Case;

fn diagnostic(source: &str) -> String {
    let case = Case::new(source);
    let mut previous = None;
    for stage in ["hir", "lir", "cpp"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(!output.status.success(), "{source}");
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(
            message.contains("readonly modifier is here")
                && message.contains("readonly propagation path:"),
            "{message}"
        );
        assert!(!case.0.join("main.cpp").exists());
        if let Some(previous) = previous {
            assert_eq!(previous, message);
        }
        previous = Some(message);
    }
    let message = previous.unwrap();
    let steps = path(&message)
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("  ..."))
        .count();
    assert!(
        steps <= 16,
        "diagnostic exceeds the documented 16 steps: {message}"
    );
    message
}
fn path(message: &str) -> &str {
    message.split("readonly propagation path:").nth(1).unwrap()
}
fn ordered(message: &str, parts: &[&str]) {
    let mut rest = message;
    for part in parts {
        let index = rest
            .find(part)
            .unwrap_or_else(|| panic!("missing {part:?} in {message}"));
        rest = &rest[index + part.len()..];
    }
}

#[test]
fn assignments_joins_and_shadowing_use_the_reaching_restriction() {
    let message = diagnostic(
        "fn bad(flag:Bool){\nreadonly let source:=1;\nlet discarded:=source;\nlet alias:=0;\nif flag{alias:=source;}\ndiscarded:=0;\n{let alias:=123;wt alias;}\nalias=2;\n}bad(true);",
    );
    ordered(
        path(&message),
        &[
            "readonly binding 'source' introduced here",
            "main.rusk:5:",
            "binding 'alias' assigned here",
            "argument 1 of '__assign__'",
        ],
    );
    assert!(
        !path(&message).contains("discarded")
            && !path(&message).contains("main.rusk:4:")
            && !path(&message).contains("main.rusk:7:"),
        "{message}"
    );
    let message = diagnostic(
        "fn bad(n:Int){readonly let source:=1;let selected:=0;for i in 0..n{selected:=source;}selected=2;}bad(1);",
    );
    assert!(
        path(&message).contains("binding 'selected' assigned here"),
        "{message}"
    );
}

#[test]
fn stored_restrictions_follow_heap_aliases_and_nested_projections() {
    let message = diagnostic(
        "struct Box{value:Int;}struct Outer{child:Box;}\nlet box:=Box{value:0};let outer:=Outer{child:box};let alias:=outer.child;\nreadonly let source:=1;\nalias.value:=source;\nlet selected:=outer.child.value;\nselected=2;",
    );
    ordered(
        path(&message),
        &[
            "readonly binding 'source'",
            "stored in field 'value' of 'Box'",
            "field 'child' of 'Outer' retrieved",
            "field 'value' of 'Box' retrieved",
            "binding 'selected' declared here",
        ],
    );
    let message = diagnostic(
        "enum Cell{Full(Int),Empty}readonly let source:=1;let cell:=Cell::Full(source);let selected:=match cell{Full(value)=>value,Empty=>0,};selected=2;",
    );
    assert!(
        path(&message).contains("payload 1 of variant 'Full'")
            && path(&message).contains("binding 'selected' declared here"),
        "{message}"
    );
}

#[test]
fn returned_access_includes_callee_bindings_and_return_locations() {
    let message = diagnostic(
        "struct Box{value:Int;}\nfn get(box:Box)->Int{let picked:=box.value;return picked;}\nreadonly let original:=Box{value:1};\nlet selected:=get(original);\nselected=2;",
    );
    ordered(
        path(&message),
        &[
            "readonly binding 'original'",
            "enters parameter 1 of 'get'",
            "binding 'box' declared here",
            "binding 'picked' declared here",
            "readonly access returned here",
            "result of 'get' propagates",
            "binding 'selected' declared here",
        ],
    );
    let message = diagnostic(
        "fn make()->Int{readonly let source:=1;let alias:=source;return alias;}let selected:=make();selected=2;",
    );
    ordered(
        path(&message),
        &[
            "readonly binding 'source'",
            "binding 'alias' declared here",
            "readonly access returned here",
            "result of 'make' propagates",
        ],
    );
}

#[test]
fn mutation_paths_enter_nested_callees_and_choose_the_restricted_argument() {
    let message = diagnostic(
        "fn inner(a:Int,b:Int){\na=1;\nb=2;\n}\nfn outer(x:Int){let forwarded:=x;inner(0,forwarded);}\nreadonly let source:=1;\nouter(source);",
    );
    ordered(
        path(&message),
        &[
            "readonly binding 'source'",
            "enters parameter 1 of 'outer'",
            "binding 'forwarded' declared here",
            "enters parameter 2 of 'inner'",
            "binding 'b' declared here",
            "main.rusk:3:",
            "argument 1 of '__assign__'",
        ],
    );
    assert!(!path(&message).contains("main.rusk:2:"), "{message}");
    let message = diagnostic(
        "struct Box{value:Int;}fn set(box:Box){let alias:=box;alias.value:=2;}readonly let source:=Box{value:1};set(source);",
    );
    ordered(
        path(&message),
        &[
            "enters parameter 1 of 'set'",
            "binding 'alias' declared here",
            "field or tuple element rebinding",
        ],
    );
}

#[test]
fn opaque_effects_and_untracked_heap_routes_are_explicitly_uncertain() {
    let conservative = diagnostic(
        "struct Pair{left:Int;right:Int;}fn change(pair:Pair){pair.right=2;}readonly let source:=1;let pair:=Pair{left:source,right:0};change(pair);",
    );
    assert!(
        conservative.contains("cannot prove readonly access is preserved")
            && conservative.contains("uncertain readonly propagation path:"),
        "{conservative}"
    );
    assert!(
        path(&conservative)
            .contains("precise operation for this contained readonly access is not reconstructed"),
        "{conservative}"
    );
    assert!(
        !path(&conservative).contains("field 'right' of 'Pair' retrieved"),
        "{conservative}"
    );
    let message = diagnostic(
        r#"extern fn opaque(x:Int){cpp_name:"opaque",code:"void opaque(Obj<std::int64_t>,Loc){}",}fn forward(x:Int){let alias:=x;opaque(alias);}readonly let source:=1;forward(source);"#,
    );
    assert!(
        message.contains("cannot prove readonly access is preserved")
            && message.contains("uncertain readonly propagation path:"),
        "{message}"
    );
    ordered(
        path(&message),
        &[
            "readonly binding 'source'",
            "binding 'alias' declared here",
            "argument 1 of 'opaque'",
        ],
    );
    let message = diagnostic(
        r#"struct Box{value:Int;}fn store(box:Box,x:Int){box.value:=x;}extern fn opaque()->Box{cpp_name:"opaque",code:"Obj<Box> opaque(Loc){return {};}",}readonly let source:=1;let box:=Box{value:0};store(box,source);let unknown:=opaque();unknown.value=2;"#,
    );
    assert!(
        message.contains("uncertain readonly propagation path:")
            && path(&message).contains("untracked")
            && path(&message).contains("return access of 'opaque' is unknown"),
        "{message}"
    );
}

#[test]
fn captured_and_static_callable_returns_keep_the_source_restriction() {
    let message = diagnostic(
        "readonly let source:=1;let getter:=()=>source;let selected:=getter();selected=2;",
    );
    assert!(
        path(&message).contains("readonly binding 'source'")
            && path(&message).contains("captured binding 'source' accessed here")
            && path(&message).contains("readonly access returned here"),
        "{message}"
    );
    let message = diagnostic(
        "fn change(x:Int){let local:=x;local=2;}let callback:=(x:Int)=>change(x);readonly let source:=1;callback(source);",
    );
    assert!(
        path(&message).contains("binding 'local' declared here")
            && path(&message).contains("argument 1 of '__assign__'"),
        "{message}"
    );
}

#[test]
fn long_histories_and_recursive_calls_are_bounded() {
    let mut source = "readonly let source:=1;let alias0:=source;".to_string();
    for index in 1..40 {
        source += &format!("let alias{index}:=alias{};", index - 1);
    }
    source += "alias39=2;";
    let message = diagnostic(&source);
    assert!(
        message.contains("additional readonly propagation steps omitted"),
        "{message}"
    );
    let message = diagnostic(
        "fn change(x:Int,n:Int){if n>0{change(x,n-1);}else{x=2;}}readonly let source:=1;change(source,1);",
    );
    assert!(
        path(&message).contains("argument 1 of '__assign__'")
            || path(&message).contains("steps omitted"),
        "{message}"
    );
}

#[test]
fn different_origins_and_shadowed_names_do_not_supply_false_histories() {
    let accepted = Case::new(
        "readonly let original:=1;let selected:=original;selected:=2;selected=3;wt original,selected;",
    );
    let fast = accepted.run("");
    let plain = accepted.run_with_options("", &["--no-opt"]);
    assert!(fast.status.success() && plain.status.success());
    assert_eq!(fast.stdout, b"1 3\n");
    assert_eq!(fast.stdout, plain.stdout);
    assert_eq!(fast.stderr, plain.stderr);
    let message = diagnostic(
        "readonly let left:=1;readonly let right:=2;let selected:=left;selected:=right;selected=3;",
    );
    assert!(
        path(&message).contains("readonly binding 'right' introduced here")
            && !path(&message).contains("'left'"),
        "{message}"
    );
    let message = diagnostic(
        "readonly let source:=1;\n{\nreadonly let source:=2;\nlet alias:=source;\nalias=3;\n}",
    );
    assert!(
        path(&message).contains("main.rusk:3:") && !path(&message).contains("main.rusk:1:"),
        "{message}"
    );
}
