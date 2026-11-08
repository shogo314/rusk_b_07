mod support;
use support::Case;
fn output(source: &str, expected: &str) {
    let out = Case::new(source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn blocks_return_shared_tail_values_and_unit_for_discarded_tails() {
    output(
        r#"
fn consume(value:Unit)->Int{return 9;}
let source:=[1];let selected:={let local:=source;local};selected[0]=7;
let detached={source};detached[0]=8;
let x:={let local:=10;local*2};
let nested:={{let local:=3;local+1}};
let u:Unit:={};let discarded:Unit:={x;};
let discarded_if:Unit:={if true{1}else{2};};
let discarded_block:Unit:={{1};};
wt source,detached,x,nested,consume(u),consume(discarded),consume(discarded_if),consume(discarded_block);
let result:={if true{let n:=4;n+1}else{0}};wt result;
{let scoped:=1;wt scoped;};
"#,
        "7 8 20 4 9 9 9 9\n5\n1\n",
    );
}

#[test]
fn labeled_break_and_continue_skip_inner_else_and_preserve_outer_else() {
    output(
        r#"
let trace:Vec<Int>:=[];
outer: for i in 0..3 {
    inner: for j in 0..3 {trace.push_back(i*10+j);if j==1{continue outer;}}
    else{trace.push_back(-1);}
}else{trace.push_back(999);}
wt trace;trace=[];
outer: for i in 0..3{for j in 0..3{trace.push_back(i*10+j);if i==1&&j==1{break outer;}}else{trace.push_back(100);}}
else{trace.push_back(-1);}
wt trace;
let outer:=7;
outer: loop {wt outer;break outer;}
outer: while true{break outer;}
"#,
        "0 1 10 11 20 21 999\n0 1 2 100 10 11\n7\n",
    );
}

#[test]
fn exits_from_expression_operands_do_not_evaluate_later_operands() {
    output(
        r#"
fn mark(log:Vec<Int>,value:Int)->Int{log.push_back(value);return value;}
fn add(a:Int,b:Int)->Int{return a+b;}
let log:Vec<Int>:=[];
outer: loop{let x:=add(mark(log,1),{break outer;});log.push_back(99);}
outer: loop{let x:=[mark(log,2),{break outer;},mark(log,99)];}
outer: loop{let x:=mark(log,3)+{break outer;};}
wt log;
fn choose(n:Int)->Int{let value:=if n<0{return 9;}else{let x:=n+1;x};return value*2;}
fn finish()->Int{let value:={return 7;};}
fn both(flag:Bool)->Int{let value:=if flag{return 3;}else{return 4;};}
wt choose(-1),choose(2),finish(),both(true),both(false);
"#,
        "1 2 3\n9 6 7 3 4\n",
    );
}

#[test]
fn short_circuiting_and_value_branches_allow_diverging_alternatives() {
    output(
        r#"
outer: loop {
    let a:=if false{break outer;}else{10};
    let b:=if true{20}else{continue outer;};
    let c:=Some(30)??{break outer;};
    let flag:=false&&{break outer;true};
    wt a,b,c,flag;
    wt 3<2<{break outer;9};
    wt 1>2<{break outer;9}<10;
    break outer;
}
let initialized:Int;
outer: loop{let x:=if true{initialized:=5}else{break outer;};wt initialized,x;break;}
let n:=0;
outer: while {n+=1;if n==3{break outer;}true}{wt n;}
outer: loop{let x:=None::<Int>()??{break outer;};wt 99;}
"#,
        "10 20 30 No\nNo\nNo\n5 5\n1\n2\n",
    );
}

#[test]
fn block_closures_capture_only_free_variables_and_infer_return_types() {
    output(
        r#"
let outer:=10;
let f:=(n:Int)=>{if n<0{return outer;}let local:=n+1;local*2};
wt f(-1),f(2);
let detached=f;outer=20;wt f(-1),detached(-1);
let x:=99;let local:=()=>{let x:=1;x+2};wt local(),x;
let held:={let state:=0;()=>{state+=1;state}};wt held(),held();
let nested:=()=>{let temp:=5;(x:Int)=>{let y:=temp+x;y}};wt nested()(2);
fn calculate<T>(value:T)->T{return {let held:=value;held};}
wt calculate(8);
let explicit:=(flag:Bool)=>{if flag{return 4;}else{return 5;}};wt explicit(true),explicit(false);
"#,
        "10 6\n20 10\n3 99\n1 2\n7\n8\n4 5\n",
    );
}

#[test]
fn labels_accept_comments_and_do_not_capture_record_field_prefixes() {
    output(
        r#"
struct Entry{value:Int;}
let forbidden:=7;let entry:=Entry{value:forbidden};wt entry.value;
outer // name comment
: // colon comment
for i in 0..3 {wt i;break outer;}
while true{inner: loop{break inner;}break;}
"#,
        "7\n0\n",
    );
}

#[test]
fn invalid_labels_types_and_escapes_are_located() {
    for source in [
        "let value:={let local:=1;local};wt local;",
        "let value:Int:={let local:=1;};",
        "let value:=if true{let x:=1;x}else{false};",
        "outer: loop{inner: loop{break missing;}}",
        "outer: loop{outer: loop{break;}}",
        "outer: loop{break;}break outer;",
        "outer: loop{let f:=()=>{break outer;};break;}",
        "let f:=()=>{if true{return 1;}false};",
        "fn f()->Int{let x:=1;x}",
        "outer: {break outer;}",
        "loop{break 1;}",
        "if: loop{break;}",
        "fn f()->Int{return 1;wt unknown;}",
        "let x:Int;while {x:={break;};true}{}wt x;",
        "let x:Int;let flag:=true;while {if flag{break;}x:=1;false}{}wt x;",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "unexpected success: {source}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("main.rusk:"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn escaping_borrows_and_jump_cleanup_keep_their_exact_lifetimes() {
    let source = include_str!("../examples/beta05/blocks_and_labels.rusk");
    let case = Case::new(source);
    let out = case.run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"a b\nZbc!\n1 2\n");
    for stage in ["hir", "lir"] {
        let status = case.compile_with_options(&["--emit", stage]);
        assert!(status.status.success());
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(!dump.contains("Probe") && !dump.contains("<diverged>"));
    }
    let invalid = Case::new("let text:=\"abc\";let held:={text[0]};text.push_back('!');").run("");
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("main.rusk:"));
}

#[test]
fn condition_breaks_preserve_only_initialization_which_precedes_every_exit() {
    output("let x:Int;while {x:=7;break;true}{}wt x;", "7\n");
}
