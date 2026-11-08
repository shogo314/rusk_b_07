mod support;
use support::Case;

fn output(source: &str, expected: &str) {
    let out = Case::new(source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}
#[test]
fn nested_enum_tuple_struct_and_literal_patterns() {
    output(
        r#"
struct Point<T>{x:T;y:Int;}
enum Event<T>{Data(T),Empty}
let event:=Event::Data(Some((2,Point::<Int>{x:3,y:4})));
wt match event{Data(Some((n,Point{x:3,y})))=>n+y,Data(Some(_))|Data(None)=>-1,Empty=>0,};
wt match (-9223372036854775808,"a",'\n',true){(-9223372036854775808,"\u0061",'\n',true)=>1,_=>0,};
fn unit(){} wt match unit(){()=>8,};
wt match 42{n=>n+1,};
wt match Point::<Int>{x:5,y:6}{Point{x,..}=>x,};
"#,
        "6\n1\n8\n43\n5\n",
    );
}
#[test]
fn or_patterns_choose_the_first_complete_match_and_guard_once() {
    output(
        r#"
let pair:=(Some(1),Some(2));let log:Vec<Int>:=[];
wt match pair {
    (Some(x),_)|(_,Some(x)) if {log.push_back(copy(x));false}=>100,
    _=>0,
};wt log;
wt match pair{(Some(x),_)|(_,Some(x)) if x>0=>{x=9;x}_=>0,};
wt pair[0]?!,pair[1]?!;
wt match 5{0|1=>1,2|3|5|7=>2,_=>3,};
"#,
        "0\n1\n9\n9 2\n2\n",
    );
}
#[test]
fn guard_mutations_keep_decisions_and_shared_binding_targets_stable() {
    output(
        r#"
let e:=Some(1);let count:=0;
wt match e{Some(x) if {e=None;x=7;count+=1;false}=>100,Some(x)=>x,None=>0,},count,e.is_none();
struct Row{x:Int;}
let row:=Row{x:1};
wt match row{Row{x:1} if {row.x:=2;false}=>100,Row{x:n}=>n,},row.x;
let b:=true;
wt match b{false=>0,true if {b=false;false}=>1,true=>2,},b;
let original:=Some(1);
wt match original{Some(x) if {x:=9;true}=>x,Some(x)=>x,None=>0,},original?!;
wt match original{Some(x) if {x:=9;false}=>x,Some(x)=>x,None=>0,},original?!;
let text:="abc";
let chosen:=match Some('a'){Some(c) if {c:=text[0];false}=>c,Some(c)=>{text.push_back('!');c}None=>'?',};wt chosen,text;
let kept:=match Some('a'){Some(c) if {c:=text[0];true}=>c,Some(c)=>c,None=>'?',};
kept='Z';wt text;kept:='Q';text.push_back('?');wt kept,text;
"#,
        "7 1 Yes\n1 2\n2 No\n9 1\n1 1\na abc!\nZbc!\nQ Zbc!?\n",
    );
}
#[test]
fn if_let_statements_expressions_scopes_and_sharing() {
    output(
        r#"
let outer:=100;let row:=[1];let value:=Some(row);let count:=0;
fn mark(value:Option<Vec<Int>>,count:Int)->Option<Vec<Int>>{count+=1;return value;}
if let Some(outer)=mark(value,count){outer[0]=3;}else{wt outer;}
wt row,outer,count;
let answer:=if let Some(x)=value{x[0]+1}else{0};wt answer;
if let None=value{wt 0;}else{wt outer;}
if let 0|1=1{wt 7;}
let flag:=if let (true,Some(x))=(false,Some(1)){x}else{8};wt flag;
"#,
        "3 100 1\n4\n100\n7\n8\n",
    );
}
#[test]
fn while_let_rechecks_once_per_iteration_with_else_and_labeled_exits() {
    output(
        r#"
let steps:=0;let probes:=0;
fn next(steps:Int,probes:Int)->Option<Int>{probes+=1;return if steps<4{Some(steps+=1)}else{None};}
let sum:=0;
outer:while let Some(n)=next(steps,probes){if n==2{continue outer;}sum+=n;}else{wt "natural";}
wt sum,steps,probes;
steps=0;probes=0;
outer:while let Some(n)=next(steps,probes){loop{if n==2{break outer;}break;}}else{wt "wrong";}
wt steps,probes;
let current:=Some([1]);let held:Vec<Int>:=[];
while let Some(row)=current{held:=row;current=None;}else{wt held;}
"#,
        "natural\n8 4 5\n2 2\n1\n",
    );
}
#[test]
fn closures_capture_resolved_references_and_not_shadowed_pattern_names() {
    output(
        r#"
enum Tag{A,B}
let A:=9;let x:=7;
let f:=(tag:Tag)=>match tag{A=>A,B=>x,};let detached=f;
A=10;x=8;wt f(Tag::A),f(Tag::B),detached(Tag::A),detached(Tag::B);
let text:="abc";let shadow:=text[0];
let g:=(o:Option<Int>)=>if let Some(shadow)=o{shadow}else{0};
shadow:='Q';text.push_back('!');wt g(Some(3)),text;
let tag:=Tag::A;let nested:=()=>{()=>match tag{A=>A,B=>x,}};wt nested()();
let branch:=(b:Bool)=>{if b{wt x;}return x;};wt branch(false);
"#,
        "10 8 9 7\n3 abc!\n10\n8\n",
    );
}
#[test]
fn generic_patterns_keep_variant_names_distinct_from_type_parameters() {
    output(
        r#"
enum Named<A>{T(A),Empty}
struct Boxed<A>{value:A;}
fn pick<T>(value:Named<Boxed<T>>,fallback:T)->T{
    return match value{Named::<Boxed<T>>::T(Boxed::<T>{value})=>value,Empty=>fallback,};
}
fn condition<T>(value:Named<T>,fallback:T)->T{ return if let Named::<T>::T(x)=value{x}else{fallback}; }
wt pick(Named::T(Boxed::<Int>{value:5}),0),condition(Named::T(6),0);
"#,
        "5 6\n",
    );
}
#[test]
fn guard_initialization_and_diverging_guards_preserve_flow() {
    output(
        r#"
fn f(value:Option<Int>)->Int{
    let assigned:Int;
    return match value{Some(n) if {assigned:=n;true}=>assigned,None=>0,Some(_)=>-1,};
}
fn g(value:Option<Int>)->Int{
    match value{Some(_) if {return 7;}=>{wt "unreachable";},_=>{return 8;}}
}
wt f(Some(3)),f(None),g(Some(1)),g(None);
let n:=0;
outer:loop{
    match Some(1){Some(_) if {n+=1;break outer;}=>{wt "unreachable";},_=>{n+=100;}}
}wt n;
"#,
        "3 0 7 8\n1\n",
    );
}
#[test]
fn borrowed_payloads_escape_and_release_across_pattern_control_flow() {
    output(
        r#"
let s:="abc";let e:=Some(s[0]);
let kept:=match e{Some(c) if {e=None;true}=>c,Some(c)=>c,None=>'?',};
kept='Z';wt s;kept:='Q';s.push_back('!');wt kept,s;
let text:="a";let n:=0;
outer:while let Some(c)=if n<3{Some(text[0])}else{None}{n+=1;if n==1{continue outer;}if n==2{break outer;}}
text.push_back('!');wt text,n;
"#,
        "Zbc\nQ Zbc!\na! 2\n",
    );
}
#[test]
fn finite_product_coverage_agrees_with_an_independent_enumeration() {
    let masks = [
        (
            "(A(false),_)",
            (0..9).filter(|i| i / 3 == 0).fold(0u16, |m, i| m | 1 << i),
        ),
        (
            "(_,A(true))",
            (0..9).filter(|i| i % 3 == 1).fold(0, |m, i| m | 1 << i),
        ),
        ("(A(_),B)", (1 << 2) | (1 << 5)),
        ("(B,A(_))", (1 << 6) | (1 << 7)),
        ("(A(true),A(false))", 1 << 3),
        ("(B,B)", 1 << 8),
        (
            "(A(true)|B,_)",
            (0..9).filter(|i| i / 3 != 0).fold(0, |m, i| m | 1 << i),
        ),
        ("_", 511),
    ];
    for subset in 0..256u16 {
        let mut covered = 0u16;
        let mut unreachable = false;
        let mut arms = String::new();
        for (i, (pattern, mask)) in masks.iter().enumerate() {
            if subset & (1 << i) != 0 {
                unreachable |= mask & !covered == 0;
                covered |= mask;
                arms += &format!("{pattern}=>0,");
            }
        }
        let source =
            format!("enum E{{A(Bool),B}}let value:=(E::A(true),E::B);wt match value{{{arms}}};");
        let out = Case::new(&source).compile();
        assert_eq!(
            out.status.success(),
            covered == 511 && !unreachable,
            "subset={subset}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(
                err.contains(if unreachable {
                    "unreachable"
                } else {
                    "non-exhaustive"
                }),
                "{source}\n{err}"
            );
        }
    }
}
#[test]
fn invalid_patterns_and_guards_report_source_errors() {
    for (source, message) in [
        ("wt match true{true=>1,};", "false"),
        (
            "wt match Some(Some(1)){Some(Some(_))=>1,None=>0,};",
            "Some(None)",
        ),
        ("wt match 1{0=>0,};", "non-exhaustive"),
        ("wt match true{_ if true=>1,};", "non-exhaustive"),
        ("wt match 1{0|-0=>1,_=>0,};", "unreachable or-pattern"),
        (
            "wt match \"a\"{\"a\"=>1,\"\\u0061\"=>2,_=>0,};",
            "unreachable",
        ),
        ("wt match Some(1){Some(x)|None=>x,};", "same names"),
        (
            "wt match (Some(1),Some(true)){(Some(x),_)|(_,Some(x))=>0,_=>1,};",
            "same types",
        ),
        ("wt match (1,2){(x,x)=>0,};", "duplicate variable"),
        (
            "struct P{x:Int;y:Int;}let p:=P{x:1,y:2};wt match p{P{x}=>x,};",
            "missing struct pattern fields",
        ),
        (
            "struct P{x:Int;}let p:=P{x:1};wt match p{P{z,..}=>0,};",
            "unknown field",
        ),
        (
            "struct P{x:Int;}let p:=P{x:1};wt match p{P{x:a,x:b}=>0,};",
            "duplicate struct pattern field",
        ),
        ("if let Some(x)=Some(1){wt x;}wt x;", "unknown"),
        ("if let Some(x)=Some(1){wt x;}else{wt x;}", "unknown"),
        ("let x:=if let Some(n)=Some(1){n};", "syntax"),
        (
            "let x:=if let Some(n)=Some(1){n}else{false};",
            "type mismatch",
        ),
        ("while let Some(x)=Some(1){break;}wt x;", "unknown"),
        ("while let Some(x)=Some(1){break;}else{wt x;}", "unknown"),
        ("wt match Some(1){Some(x) if true=>x,_=>x,};", "unknown"),
    ] {
        let out = Case::new(source).compile();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && err.contains(message) && err.contains("main.rusk"),
            "{source}\n{err}"
        );
    }
}

#[test]
fn empty_or_one_payload_enums_preserve_tags_in_both_orders_and_with_unit_payloads() {
    output(
        r#"
enum Maybe<T>{Absent,Present(T)}
enum Many{A,B,Pair(Int,Int)}
fn unit(){}
let x:=Maybe::Present([1]);let alias:=x;let held:=match x{Present(row)=>row,Absent=>[],};
x=Maybe::Absent;held[0]=2;
wt match alias{Absent=>true,Present(_)=>false,},held;
let u:=Maybe::Present(unit());wt match u{Present(())=>1,Absent=>0,};
let option:=Some(None::<Int>());wt match option{Some(None)=>3,Some(Some(_))|None=>4,};
wt match Many::Pair(2,3){Many::Pair(x,y)=>x+y,Many::A|Many::B=>0,};
"#,
        "Yes 2\n1\n3\n5\n",
    );
}

#[test]
fn many_guards_evaluate_once_in_order_without_retrying_patterns() {
    let mut arms = String::new();
    for _ in 0..64 {
        arms += "_ if {count+=1;false}=>0,";
    }
    output(
        &format!("let count:=0;wt match 1{{{arms}_=>7,}},count;"),
        "7 64\n",
    );
}
#[test]
fn public_pattern_examples_match_samples_and_long_digit_inputs() {
    let cases = [
        (
            include_str!("../examples/aoj/itp1_3_c_patterns.test.rusk"),
            "3 2\n2 2\n5 1\n0 0\n",
            "2 3\n2 2\n1 5\n",
        ),
        (
            include_str!("../examples/aoj/itp1_8_b.test.rusk"),
            "123\n55\n1000\n0\n",
            "6\n10\n1\n",
        ),
    ];
    for (source, input, expected) in cases {
        let out = Case::new(source).run(input);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, expected.as_bytes());
    }
    let out = Case::new(include_str!("../examples/aoj/itp1_8_b.test.rusk"))
        .run(&format!("{}\n0\n", "9".repeat(1000)));
    assert!(out.status.success());
    assert_eq!(out.stdout, b"9000\n");
}

#[test]
fn resolved_captures_keep_the_established_copy_order() {
    output(
        r#"
extern symbol observation{code:"std::int64_t trace_value=0;",}
extern type Score{cpp_name:"std::int64_t",}
extern fn score(n:Int)->Score{cpp_name:"make_score",code:"Obj<std::int64_t> make_score(Obj<std::int64_t> n,Loc){return n;}",}
extern fn number(n:Score)->Int{cpp_name:"score_number",code:"Obj<std::int64_t> score_number(Obj<std::int64_t> n,Loc){return n;}",}
extern fn copy(n:Score)->Score{cpp_name:"score_copy",code:"Obj<std::int64_t> score_copy(Obj<std::int64_t> n,Loc){trace_value=trace_value*10+*n;return make_obj<std::int64_t>(*n);}",}
extern fn trace()->Int{cpp_name:"score_trace",code:"Obj<std::int64_t> score_trace(Loc){return make_obj<std::int64_t>(trace_value);}",}
let a:=score(1);let b:=score(2);let f:=()=>number(b)+number(a);
let detached=f;wt trace(),detached();
"#,
        "12 3\n",
    );
}
