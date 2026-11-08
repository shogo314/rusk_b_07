mod support;
use support::Case;

fn summary<'a>(dump: &'a str, name: &str) -> &'a str {
    let start = dump.find(&format!("name: \"{name}\",")).unwrap();
    let function = &dump[start..];
    let effects = &function[function.find("effects: Summary {").unwrap()..];
    &effects[..effects.find("\n            },").unwrap()]
}
fn parameter(summary: &str, index: usize) -> &str {
    summary
        .split("ParameterEffects {")
        .nth(index + 1)
        .unwrap()
        .split('}')
        .next()
        .unwrap()
}
fn dumps(source: &str, check: impl Fn(&str)) {
    let case = Case::new(source);
    for stage in ["hir", "lir"] {
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        check(&dump);
    }
}

#[test]
fn scalar_arithmetic_and_copy_are_readonly_and_nonescaping_but_can_fail() {
    dumps(
        r#"
fn add_one(x:Int)->Int{return x+1;}
fn bump(x:Int){x+=1;}
fn detached(x:Int)->Int{return copy(x);}
wt add_one(1);let x:=1;bump(x);wt x,detached(x);
"#,
        |dump| {
            let read = summary(dump, "add_one");
            let p = parameter(read, 0);
            assert!(p.contains("read: May") && p.contains("write: No") && p.contains("escape: No"));
            assert!(read.contains("fresh: true") && read.contains("may_fail: May"));
            assert!(read.contains("io: No") && read.contains("external_write: No"));
            let p = parameter(summary(dump, "bump"), 0);
            assert!(p.contains("write: May") && p.contains("escape: No"));
            let copy = summary(dump, "detached");
            assert!(copy.contains("fresh: true") && copy.contains("unknown: false"));
            assert!(parameter(copy, 0).contains("escape: No"));
        },
    );
}

#[test]
fn returning_discarding_and_copying_element_references_have_distinct_escape_effects() {
    dumps(
        r#"
fn first(a:Vec<Int>)->Int{return a[0];}
fn ignored(a:Vec<Int>){first(a);}
fn forwarded(a:Vec<Int>)->Int{return first(a);}
fn copied(a:Vec<Int>)->Int{return copy(first(a));}
fn string_first(s:Str)->Char{return s[0];}
fn string_copy(s:Str)->Char{return copy(s[0]);}
let a:=[3,4];ignored(a);wt first(a),forwarded(a),copied(a),string_first("abc"),string_copy("abc");
"#,
        |dump| {
            for name in ["first", "forwarded", "string_first"] {
                let s = summary(dump, name);
                let p = parameter(s, 0);
                assert!(
                    p.contains("write: No")
                        && p.contains("escape: May")
                        && p.contains("retained: No"),
                    "{name}: {s}"
                );
                assert!(s.contains("Element") && s.contains("parameter: 0"));
            }
            for name in ["ignored", "copied", "string_copy"] {
                let s = summary(dump, name);
                let p = parameter(s, 0);
                assert!(
                    p.contains("escape: No") && p.contains("retained: No"),
                    "{name}: {s}"
                );
            }
            assert!(summary(dump, "copied").contains("fresh: true"));
        },
    );
}

#[test]
fn native_contracts_keep_io_structural_changes_and_failure_observable() {
    dumps(
        r#"
fn total(a:Vec<Int>)->Int{return a.sum();}
fn string_length(s:Str)->Int{return s.len();}
fn output(x:Int){wt x;}
fn input()->Int{in x:Int;return x;}
fn expand(s:Str){s.push_back('!');}
let a:=[1,2];output(total(a));let s:="abc";expand(s);
"#,
        |dump| {
            let total = summary(dump, "total");
            assert!(
                parameter(total, 0).contains("write: No")
                    && parameter(total, 0).contains("escape: No")
            );
            assert!(total.contains("may_fail: May") && total.contains("io: No"));
            assert!(summary(dump, "string_length").contains("may_fail: May"));
            for name in ["output", "input"] {
                let s = summary(dump, name);
                assert!(
                    s.contains("io: May") && s.contains("may_diverge: May"),
                    "{s}"
                );
            }
            assert!(summary(dump, "input").contains("may_fail: May"));
            let p = parameter(summary(dump, "expand"), 0);
            assert!(
                p.contains("write: May")
                    && p.contains("structural: May")
                    && p.contains("retained: No")
            );
        },
    );
}

#[test]
fn foreign_code_and_uncontracted_container_operations_stay_unknown() {
    dumps(
        r#"
extern fn opaque(x:Int)->Int {
 cpp_name:"looks_pure", code:"Obj<std::int64_t> looks_pure(Obj<std::int64_t> x,Loc){return x;}",
}
fn external(x:Int)->Int{return opaque(x);}
fn lookup(a:Vec<Int>)->Bool{return a.contains(1);}
wt external(3),lookup([1]);
"#,
        |dump| {
            assert!(summary(dump, "opaque").contains("source: Opaque"));
            for name in ["external", "lookup"] {
                let s = summary(dump, name);
                assert!(
                    s.contains("external_write: Unknown")
                        && parameter(s, 0).contains("retained: Unknown"),
                    "{s}"
                );
            }
        },
    );
}

#[test]
fn user_externs_cannot_claim_the_embedded_runtimes_trusted_contracts() {
    let case = Case::new(
        r#"
extern fn pretend(x:Int)->Int {
 effects:"read_fresh",cpp_name:"pretend",code:"ignored",
}
wt pretend(3);
"#,
    );
    let out = case.compile();
    assert!(!out.status.success());
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(
        message.contains("main.rusk:")
            && message.contains("reserved for the embedded standard library"),
        "{message}"
    );
}

#[test]
fn contracted_operations_preserve_shared_and_independent_return_values() {
    let case = Case::new(
        r#"
fn first(a:Vec<Int>)->Int{return a[0];}
fn detached(a:Vec<Int>)->Int{return copy(first(a));}
let a:=[3];let held:=first(a);let independent:=detached(a);
held=7;wt a,held,independent;
let s:="abc";let letter:=s[0];let copied:=copy(letter);
letter:='Q';s.push_back('!');wt copied,s;
"#,
    );
    let optimized = case.run("");
    assert!(
        optimized.status.success(),
        "{}",
        String::from_utf8_lossy(&optimized.stderr)
    );
    assert_eq!(optimized.stdout, b"7 7 3\na abc!\n");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert_eq!(optimized.stdout, plain.stdout);
    assert_eq!(optimized.stderr, plain.stderr);
    assert_eq!(optimized.status.code(), plain.status.code());
}
