mod support;
use support::Case;

fn succeeds(source: &str, expected: &str) {
    let case = Case::new(source);
    let run = case.run("");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, expected.as_bytes());
}
fn rejects(source: &str, expected: &str) {
    let run = Case::new(source).compile();
    assert!(!run.status.success(), "{source}");
    let error = String::from_utf8_lossy(&run.stderr);
    assert!(error.contains(expected), "{error}");
    assert!(
        error.contains("main.rusk:") && error.contains("readonly modifier is here"),
        "{error}"
    );
}

#[test]
fn readonly_is_an_access_path_and_is_independent_of_binding_identity() {
    succeeds(
        r#"
let original:=1;
readonly let view:=original;
original=2;wt view;
view:=3;wt original,view;
let detached=view;detached+=1;wt view,detached;
mutable let other:=9;other=10;other:=11;wt other;
readonly fixed let both:=7;wt both;
fixed mutable let writable:=1;writable+=2;wt writable;
readonly let answer:Int;
if true{answer:=5;}else{answer:=6;}wt answer;
"#,
        "2\n2 3\n3 4\n11\n7\n3\n5\n",
    );
}

#[test]
fn copied_values_are_writable_and_read_helpers_need_no_annotation() {
    succeeds(
        r#"
fn twice(value:Int)->Int{return value*2;}
fn first(values:Vec<Int>)->Int{return values[0];}
readonly let numbers:=[1,2];wt numbers.len(),twice(first(numbers)),numbers;
let detached=numbers;detached[0]=8;wt detached,numbers;
readonly let text:="abc";let c=text[0];c='Z';wt text,c;
fn read(readonly fixed value:Int)->Int{return value+1;}
let f:=(readonly value:Int)=>value+2;
wt read(3),f(3);
"#,
        "2 2 1 2\n8 2 1 2\nabc Z\n4 5\n",
    );
}

#[test]
fn root_aliases_elements_fields_payloads_and_mutable_annotations_cannot_erase_readonly() {
    for source in [
        "readonly let x:=1;x=2;",
        "readonly let x:=1;x+=2;",
        "readonly let x:=1;let a:=x;a=2;",
        "readonly let x:=1;mutable let a:=x;a=2;",
        "readonly let a:=[1];a[0]=2;",
        "readonly let a:=[1];a[0]:=2;",
        "readonly let a:=[1];a.push_back(2);",
        "struct Box{value:Int;}readonly let b:=Box{value:1};b.value=2;",
        "struct Box{value:Int;}readonly let b:=Box{value:1};b.value:=2;",
        "readonly let pair:=(1,2);pair[0]:=3;",
        "readonly let pair:=(1,2);let (a,b):=pair;a=3;",
        "readonly let value:=Some(1);if let Some(x)=value{x=2;}",
        "readonly let x:=1;let option:=Some(x);option?! = 2;",
        "fn unbox(readonly value:Option<Int>)->Int{return value?!;}let x:=unbox(Some(1));x=2;",
        "readonly let x:Int;x:=1;x=2;",
        "readonly let x:=1;x:=2;x=3;",
    ] {
        rejects(source, "cannot modify readonly access");
    }
}

#[test]
fn restrictions_cross_unannotated_calls_returns_recursion_and_callbacks() {
    for source in [
        "fn bump(x:Int){x+=1;}readonly let x:=1;bump(x);",
        "fn bump<T>(x:T,y:T){x=y;}readonly let x:=1;bump(x,2);",
        "fn alias(x:Int)->Int{return x;}readonly let x:=1;let a:=alias(x);a=2;",
        "fn first(a:Vec<Int>)->Int{return a[0];}readonly let a:=[1];let x:=first(a);x=2;",
        "fn view(readonly x:Int)->Int{return x;}let x:=view(1);x=2;",
        "fn fresh()->Int{readonly let x:=1;return x;}let x:=fresh();x=2;",
        "fn view(x:Int,n:Int)->Int{if n==0{return x;}return view(x,n-1);}readonly let x:=1;let a:=view(x,3);a=2;",
        "readonly let x:=1;let f:=()=>{x=2;};f();",
        "readonly let x:=1;let alias:=x;let f:=()=>{alias=2;};f();",
        "fn invoke<F>(f:F){f();}readonly let x:=1;let a:=x;invoke(()=>{a=2;});",
        "let wrap:=(x:Int)=>{readonly let local:=x;()=>{local}};let f:=wrap(1);let x:=f();x=2;",
    ] {
        rejects(source, "readonly access");
    }
}

#[test]
fn branches_and_loop_backedges_preserve_possible_restrictions() {
    for source in [
        "readonly let r:=1;let x:=0;if true{x:=r;}x=2;",
        "readonly let r:=1;let x:=0;for i in 0..2{x=3;x:=r;}",
        "readonly let r:=1;let x:=if true{r}else{2};x=3;",
        "readonly let r:=1;let x:=match true{true=>r,false=>2,};x=3;",
    ] {
        rejects(source, "cannot modify readonly access");
    }
    succeeds("readonly let r:=1;let x:=r;x:=2;x=3;wt r,x;", "1 3\n");
}

#[test]
fn stored_references_cannot_be_recovered_as_writable_through_other_aliases() {
    for source in [
        "struct Box{value:Int;}let b:=Box{value:0};let alias:=b;readonly let r:=1;b.value:=r;alias.value=2;",
        "fn store(b:Vec<Int>,v:Int){b.push_back(v);}let b:Vec<Int>:=[];readonly let r:=1;store(b,r);b[0]=2;",
        "fn store(b:Vec<Int>){readonly let r:=1;b.push_back(r);}let b:Vec<Int>:=[];store(b);b[0]=2;",
        "readonly let r:=1;let values:=[r];let x:=values[0];x=2;",
        "readonly let r:=1;let pair:=(r,0);pair[0]=2;",
    ] {
        rejects(source, "readonly access");
    }
}

#[test]
fn unknown_external_effects_are_distinguished_from_known_writes() {
    rejects(
        r#"extern fn foreign(x:Int){cpp_name:"foreign",code:"void foreign(Obj<std::int64_t>,Loc){}",}readonly let x:=1;foreign(x);"#,
        "cannot prove readonly access is preserved",
    );
    for source in [
        "fn f(readonly x:Int){x=2;}",
        "let f:=(readonly x:Int)=>{x+=1;x};",
    ] {
        rejects(source, "cannot modify readonly access");
    }
}

#[test]
fn conflicts_external_contracts_and_overloads_are_checked() {
    for (source, expected) in [
        ("readonly readonly let x:=1;", "duplicate access modifier"),
        ("mutable mutable let x:=1;", "duplicate access modifier"),
        ("readonly mutable let x:=1;", "conflicting access modifiers"),
        (
            "fn f(readonly mutable x:Int){}",
            "conflicting access modifiers",
        ),
        (
            "let f:=(mutable readonly x:Int)=>x;",
            "conflicting access modifiers",
        ),
        (
            r#"extern fn f(readonly x:Int){cpp_name:"f",code:"ignored",}"#,
            "access modifiers on extern parameters require a Rusk function body",
        ),
        (
            "fn f(readonly x:Int){}fn f(mutable x:Int){}",
            "duplicate function",
        ),
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn errors_precede_output_and_ir_keeps_declared_and_returned_restrictions() {
    for stage in ["hir", "lir", "cpp"] {
        let case = Case::new("readonly let x:=1;\nlet alias:=x;\nalias=2;\n");
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(!out.status.success());
        let message = String::from_utf8_lossy(&out.stderr);
        assert!(
            message.contains("main.rusk:3:") && message.contains("main.rusk:1:"),
            "{message}"
        );
        assert!(!case.0.join("main.cpp").exists());
    }
    let case = Case::new("fn view(readonly fixed x:Int)->Int{return x;}wt view(1);");
    for stage in ["ast", "hir", "lir"] {
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(dump.contains("Readonly") && dump.contains("Fixed"));
        if stage != "ast" {
            assert!(dump.contains("AnnotateAccess") && dump.contains("access: Summary"));
        }
    }
}

#[test]
fn diagnostics_name_the_checked_binding_without_claiming_an_assignment_history() {
    for source in [
        "readonly let original:=1;let alias:=original;alias=2;",
        "readonly let original:=1;let alias:=0;alias:=original;alias=2;",
        "readonly let original:=1;let alias:=original;{let alias:=0;wt alias;}alias=2;",
    ] {
        let case = Case::new(source);
        for stage in ["hir", "lir", "cpp"] {
            let output = case.compile_with_options(&["--emit", stage]);
            let message = String::from_utf8(output.stderr).unwrap();
            assert!(!output.status.success());
            assert!(
                message.contains("readonly access originates at 'original'")
                    && message.contains("checked access:")
                    && message.contains("access uses binding 'alias'"),
                "{message}"
            );
            assert!(!case.0.join("main.cpp").exists());
        }
    }
}

#[test]
fn diagnostics_describe_fields_getters_and_field_rebinding() {
    for (source, expected) in [
        (
            "struct Box{value:Int;}readonly let box:=Box{value:1};box.value=2;",
            "access selects field 'value' of 'Box'",
        ),
        (
            "struct Box{value:Int;}readonly let box:=Box{value:1};box.value:=2;",
            "access uses binding 'box'",
        ),
        (
            "struct Box{value:Int;}fn get(b:Box)->Int{return b.value;}readonly let box:=Box{value:1};get(box)=2;",
            "access uses result of 'get'",
        ),
        (
            "readonly let alias:=1;let change:=()=>{alias=2;};change();",
            "access uses captured binding 'alias'",
        ),
    ] {
        let case = Case::new(source);
        let output = case.compile();
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success());
        assert!(
            message.contains("readonly modifier is here") && message.contains(expected),
            "{message}"
        );
    }
}

#[test]
fn checked_access_locations_distinguish_shadowed_bindings() {
    let case = Case::new(
        "readonly let original:=1;\nlet alias:=0;\n{let alias:=original;\nalias=2;}\nwt alias;",
    );
    let output = case.compile();
    let message = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success());
    let checked = message.split("checked access:").nth(1).unwrap();
    assert!(
        checked.contains("main.rusk:3:") && !checked.contains("main.rusk:2:"),
        "{message}"
    );
}
