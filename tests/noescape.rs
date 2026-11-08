mod support;
use support::Case;
fn succeeds(source: &str, expected: &str) {
    let output = Case::new(source).run("");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());
}
fn rejects(source: &str, expected: &str) {
    let output = Case::new(source).compile();
    assert!(!output.status.success(), "{source}");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains(expected)
            && message.contains("noescape modifier is here")
            && message.contains("parameter declared here"),
        "{message}"
    );
}
#[test]
fn noescape_is_independent_of_mutation_and_permissive_escape_does_not_force_escape() {
    succeeds(
        r#"
fn twice(noescape readonly fixed value:Int)->Int{return value*2;}
fn bump(mutable noescape value:Int){value+=1;}
fn append<T>(noescape values:Vec<T>,escape value:T){values.push_back(value);}
fn fresh(escape value:Int)->Int{return value+1;}
let x:=2;bump(x);wt x,twice(x),fresh(x);
let values:Vec<Int>:=[];append(values,x);x=5;wt values;
let f:=(fixed noescape readonly value:Int)=>value+2;wt f(x);
"#,
        "3 6 4\n5\n7\n",
    );
}
#[test]
fn copies_rebinding_local_aggregates_and_discarded_alias_returns_stay_scoped() {
    succeeds(
        r#"
fn alias(value:Int)->Int{return value;}
fn detached(noescape value:Int)->Int{return copy(alias(value));}
fn ignore(noescape value:Int){alias(value);}
fn replaced(noescape value:Int)->Int{value:=8;return value;}
fn local(noescape value:Int)->Int{let pair:=(value,0);return pair[0]+1;}
fn first(values:Vec<Int>)->Int{return values[0];}
fn copied_first(readonly noescape values:Vec<Int>)->Int{return copy(first(values));}
fn ignored_first(noescape values:Vec<Int>){first(values);}
fn cloned(readonly noescape values:Vec<Int>)->Vec<Int>{return copy(values);}
fn option_copy(readonly noescape value:Option<Int>)->Int{if value.is_some(){return copy(value?!);}return 0;}
let x:=1;ignore(x);wt detached(x),replaced(x),local(x),x;
let values:=[3,4];ignored_first(values);wt copied_first(values);wt cloned(values),option_copy(Some(7));
"#,
        "1 8 2 1\n3\n3 4 7\n",
    );
}
#[test]
fn returned_roots_members_payloads_and_aggregate_contents_escape() {
    for source in [
        "fn f(noescape x:Int)->Int{return x;}wt f(1);",
        "fn f(noescape x:Int)->Int{let alias:=x;return alias;}wt f(1);",
        "fn f(noescape x:Vec<Int>)->Int{return x[0];}wt f([1]);",
        "struct Box{value:Int;}fn f(noescape x:Box)->Int{return x.value;}wt f(Box{value:1});",
        "fn f(noescape x:Option<Int>)->Int{return x?!;}wt f(Some(1));",
        "fn f(noescape x:Int)->(Int,Int){return (0,x);}wt f(1);",
        "fn f(noescape x:Int)->Option<Int>{return Some(x);}f(1);",
        "fn f(noescape x:Int)->Vec<Int>{return [x];}wt f(1);",
        "fn f(noescape x:Int)->Int{let old:=x;x:=2;return old;}wt f(1);",
    ] {
        rejects(source, "noescape parameter 'x' may escape");
    }
}
#[test]
fn external_storage_and_unknown_external_effects_are_diagnosed_at_the_call() {
    rejects(
        "fn save(noescape x:Int,values:Vec<Int>){values.push_back(x);}let a:Vec<Int>:=[];save(1,a);",
        "noescape parameter 'x' may escape",
    );
    rejects(
        "struct Box{value:Int;}fn save(noescape x:Int,b:Box){b.value:=x;}save(1,Box{value:0});",
        "cannot prove noescape for parameter 'x'",
    );
    rejects(
        r#"extern fn foreign(x:Int){cpp_name:"foreign",code:"void foreign(Obj<std::int64_t>,Loc){}",}fn f(noescape x:Int){foreign(x);}f(1);"#,
        "cannot prove noescape for parameter 'x'",
    );
}
#[test]
fn callback_results_and_captures_are_checked_even_without_saving_the_callback() {
    succeeds(
        r#"
fn invoke<F>(noescape callback:F)->Int{return callback();}
fn discard<F>(noescape callback:F){callback();}
let x:=3;wt invoke(()=>x+1);discard(()=>x);
let wrapped:=(noescape value:Int)=>{let callback:=()=>value;callback()+1};wt wrapped(x);
"#,
        "4\n4\n",
    );
    rejects(
        "fn invoke<F>(noescape callback:F)->Int{return callback();}let x:=1;wt invoke(()=>x);",
        "noescape parameter 'callback' may escape",
    );
    rejects(
        "let factory:=(noescape value:Int)=>()=>value;let callback:=factory(1);wt callback();",
        "noescape parameter 'value' may escape",
    );
    rejects(
        "fn save<F>(noescape callback:F,values:Vec<F>){values.push_back(callback);}let f:=()=>1;let values:= [f];save(f,values);",
        "noescape parameter 'callback' may escape",
    );
}
#[test]
fn branches_loops_recursion_and_forwarding_use_inferred_callee_effects() {
    succeeds(
        r#"
fn recurse(noescape x:Int,n:Int)->Int{if n==0{return x+1;}return recurse(x,n-1);}
fn odd(noescape x:Int,n:Int)->Int{if n==0{return x+1;}return even(x,n-1);}
fn even(noescape x:Int,n:Int)->Int{if n==0{return x+2;}return odd(x,n-1);}
fn repeated(noescape x:Int){for i in 0..3{wt x+i;}}
wt recurse(1,2),odd(1,2);repeated(1);
"#,
        "2 2\n1\n2\n3\n",
    );
    for source in [
        "fn alias(x:Int)->Int{return x;}fn f(noescape x:Int)->Int{return alias(x);}wt f(1);",
        "fn f(noescape x:Int,flag:Bool)->Int{if flag{return x;}return 2;}wt f(1,true);",
        "fn f(noescape x:Int,n:Int)->Int{if n==0{return x;}return f(x,n-1);}wt f(1,2);",
        "fn f(noescape x:Int)->Int{let a:=2;for i in 0..2{a:=x;}return a;}wt f(1);",
    ] {
        rejects(source, "noescape parameter 'x' may escape");
    }
}
#[test]
fn parameter_only_syntax_conflicts_and_overloads_are_rejected() {
    for (source, expected) in [
        (
            "noescape let x:=1;",
            "escape modifiers are only supported on parameters",
        ),
        (
            "escape let x:=1;",
            "escape modifiers are only supported on parameters",
        ),
        (
            "noescape fixed let (x,y):=(1,2);",
            "escape modifiers are only supported on parameters",
        ),
        (
            "fn f(noescape noescape x:Int){}",
            "duplicate escape modifier",
        ),
        (
            "let f:=(escape escape x:Int)=>x;",
            "duplicate escape modifier",
        ),
        (
            "fn f(noescape escape x:Int){}",
            "conflicting escape modifiers",
        ),
        (
            "let f:=(escape noescape x:Int)=>x;",
            "conflicting escape modifiers",
        ),
        (
            r#"extern fn f(noescape x:Int){cpp_name:"f",code:"ignored",}"#,
            "escape modifiers on extern parameters require a Rusk function body",
        ),
        (
            r#"extern fn f(escape x:Int){cpp_name:"f",code:"ignored",}"#,
            "escape modifiers on extern parameters require a Rusk function body",
        ),
        (
            "fn f(noescape x:Int){}fn f(escape x:Int){}",
            "duplicate function",
        ),
    ] {
        let output = Case::new(source).compile();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn diagnostics_point_to_returns_and_retaining_calls_before_ir_or_cpp_output() {
    for (source, line, detail) in [
        (
            "fn bad(noescape value:Int)->Int {\nlet alias:=value;\nreturn alias;\n}\nwt bad(1);",
            3,
            "return value",
        ),
        (
            "fn bad(noescape value:Int,values:Vec<Int>) {\nlet alias:=value;\nvalues.push_back(alias);\n}\nlet a:Vec<Int>:=[];bad(1,a);",
            3,
            "argument 2 of 'push_back'",
        ),
        (
            "struct Box{value:Int;}fn bad(noescape value:Int,values:Vec<Int>) {\nlet box:=Box{value:0};\nbox.value:=value;\nvalues.push_back(value);\n}\nlet a:Vec<Int>:=[];bad(1,a);",
            4,
            "argument 2 of 'push_back'",
        ),
    ] {
        for stage in ["hir", "lir", "cpp"] {
            let case = Case::new(source);
            let output = case.compile_with_options(&["--emit", stage]);
            assert!(!output.status.success());
            let message = String::from_utf8_lossy(&output.stderr);
            assert!(
                message.contains(&format!("main.rusk:{line}:"))
                    && message.contains("main.rusk:1:")
                    && message.contains(detail)
                    && message.contains("through parameter 1"),
                "{message}"
            );
            assert!(!case.0.join("main.cpp").exists());
        }
    }
    let case = Case::new("fn f(noescape x:Int,escape y:Int)->Int{return x+y;}wt f(1,2);");
    for stage in ["ast", "hir", "lir"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(dump.contains("Noescape") && dump.contains("Escape"));
        if stage != "ast" {
            assert!(dump.contains("AnnotateEscape"));
            let function = &dump[dump.find("name: \"f\",").unwrap()..];
            let effects = &function[function.find("effects: Summary").unwrap()..];
            for parameter in effects.split("ParameterEffects {").skip(1).take(2) {
                assert!(parameter.split('}').next().unwrap().contains("escape: No"));
            }
        }
    }
}

#[test]
fn private_field_stores_stay_scoped_through_aliases_branches_and_loop_rebinding() {
    succeeds(
        r#"
struct Box{value:Int;}
fn local(noescape x:Int)->Int{
 let b:=Box{value:0};let alias:=b;alias.value:=x;
 alias.value+=1;return b.value+1;
}
fn tuple(noescape x:Int)->Int{let pair:=(0,1);pair[0]:=x;return pair[0]+pair[1];}
fn choose(noescape x:Int,flag:Bool)->Int{
 let a:=Box{value:0};let b:=Box{value:1};let chosen:=a;
 if flag{chosen:=b;}chosen.value:=x;return chosen.value+2;
}
fn repeated(noescape x:Int)->Int{
 let b:=Box{value:0};let total:=0;
 for i in 0..3{let alias:=b;b:=Box{value:0};alias.value:=x;total+=alias.value;}
 return total;
}
fn old_snapshot(noescape x:Int)->Int{
 let b:=Box{value:7};let old:=b.value;b.value:=x;return old;
}
fn generic<T>(noescape x:T){let box:=(x,x);box[0]:=x;}
let x:=3;wt local(x),x,tuple(x),choose(x,true),choose(x,false),repeated(x),old_snapshot(x);
generic([1,2]);
"#,
        "5 4 5 6 6 12 7\n",
    );
}

#[test]
fn published_destinations_remain_conservative_before_and_after_the_store() {
    let cases = [
        "fn f(noescape x:Int)->Box{let b:=Box{value:0};b.value:=x;return b;}f(1);",
        "fn f(noescape x:Int)->Box{let b:=Box{value:0};let alias:=b;alias.value:=x;return b;}f(1);",
        "fn f(noescape x:Int,items:Vec<Box>){let b:=Box{value:0};items.push_back(b);b.value:=x;}let items:Vec<Box>:=[];f(1,items);",
        "fn f(noescape x:Int,items:Vec<Box>){let b:=Box{value:0};b.value:=x;items.push_back(b);}let items:Vec<Box>:=[];f(1,items);",
        "fn f(noescape x:Int,outer:Box){let b:=Box{value:0};b:=outer;b.value:=x;}f(1,Box{value:0});",
        "fn f(noescape x:Int)->Option<Box>{let b:=Box{value:0};let outer:=Some(b);b.value:=x;return outer;}f(1);",
        "fn f(noescape x:Int,flag:Bool)->Option<Box>{let b:=Box{value:0};let result:Option<Box>:=None;if flag{result:=Some(b);}b.value:=x;return result;}f(1,true);",
        "fn f(noescape x:Int,items:Vec<Box>){let b:=Box{value:0};for i in 0..2{b.value:=x;items.push_back(b);b:=Box{value:0};}}let items:Vec<Box>:=[];f(1,items);",
    ];
    for source in cases {
        rejects(
            &format!("struct Box{{value:Int;}}{source}"),
            "noescape for parameter 'x'",
        );
    }
}

#[test]
fn private_store_does_not_hide_escape_through_members_callbacks_or_unknown_calls() {
    for tail in [
        "return b.value;",
        "let value:=b.value;return value;",
        "let read:=()=>b.value;return read();",
    ] {
        rejects(
            &format!(
                "struct Box{{value:Int;}}fn f(noescape x:Int)->Int{{let b:=Box{{value:0}};b.value:=x;{tail}}}wt f(1);"
            ),
            "cannot prove noescape for parameter 'x'",
        );
    }
    rejects(
        r#"struct Box{value:Int;}
extern fn foreign(b:Box){cpp_name:"foreign",code:"void foreign(auto,Loc){}",}
fn f(noescape x:Int){let b:=Box{value:0};b.value:=x;foreign(b);}f(1);
"#,
        "cannot prove noescape for parameter 'x'",
    );
}

#[test]
fn private_stores_are_inferred_without_annotations_and_preserved_in_ir() {
    let case = Case::new(
        "struct Box{value:Int;}\nfn plain(x:Int)->Int{let b:=Box{value:0};b.value:=x;return b.value+1;}\nfn explicit(escape x:Int)->Int{let b:=Box{value:0};b.value:=x;return b.value+1;}\nwt plain(1),explicit(2);",
    );
    for stage in ["hir", "lir"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        for name in ["plain", "explicit"] {
            let function = &dump[dump.find(&format!("name: \"{name}\",")).unwrap()..];
            let effects = &function[function.find("effects: Summary").unwrap()..];
            let parameter = effects
                .split("ParameterEffects {")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            assert!(
                parameter.contains("escape: No") && parameter.contains("retained: No"),
                "{parameter}"
            );
        }
    }
}
