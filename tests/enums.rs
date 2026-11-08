mod support;
use support::Case;

fn output(source: &str, input: &str, expected: &str) {
    let out = Case::new(source).run(input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn variants_generics_and_payload_hints() {
    output(
        r#"
enum Result<T,E> { Ok(T), Err(E) }
enum Event { Stop, Pair(Int, Vec<Int>) }
let x:Result<Int,Str>:=Result::Ok(4);
wt match x { Result::Ok(n)=>n+1, Result::Err(_)=>0, };
let e:=Event::Pair(2,[3]);
let held:=match e { Event::Pair(n,row)=>{row[0]+=n;row} Event::Stop=>[], };
e=Event::Stop;wt held;
let empty:=Result::<Vec<Int>,Str>::Ok([]);
wt match empty { Ok(row)=>row.len(),Err(_)=>-1, };
let absent:Result<Int,Option<Int>>:=Result::Err(None);
wt match absent { Ok(n)=>n,Err(o)=>o??7, };
"#,
        "",
        "5\n5\n0\n7\n",
    );
}

#[test]
fn match_binding_scopes_tuple_patterns_and_generic_functions() {
    output(
        r#"
enum Packet<T> { Data(T, (Int,Str)), Empty }
fn describe<T>(p:Packet<T>,fallback:T)->T {
    return match p { Packet::Data(value,(_,label))=>{wt label;value} Packet::Empty=>fallback, };
}
fn empty<T>()->Packet<T>{return Packet::<T>::Empty;}
let p:=Packet::Data([3],(8,"payload"));
let f:=(p:Packet<Vec<Int>>)=>{match p {Data(row,(n,s))=>{wt n,s;row} Empty=>[],}};
wt f(p),describe(p,[4]);
let q:Packet<Int>:=empty::<Int>();wt describe(q,9);
let name:=100;wt match Option::Some(2) {Some(name)=>name,None=>0,},name;
"#,
        "",
        "8 payload\n3 payload\n3\n9\n2 100\n",
    );
}

#[test]
fn variant_names_are_not_replaced_by_generic_type_substitution() {
    output(
        r#"
enum Named<A> { T(A), Empty }
enum Tag<A> { T, Data(A) }
fn wrap<T>(x:T)->Named<T>{return Named::<T>::T(x);}
fn get<T>(x:Named<T>,fallback:T)->T{
    return match x{Named::T(value)=>value,Named::Empty=>fallback,};
}
fn unqualified<T>(x:Named<T>,fallback:T)->T{
    return match x{T(value)=>value,Empty=>fallback,};
}
fn tag<T>()->Tag<T>{return Tag::<T>::T;}
wt get(wrap(3),0),unqualified(wrap(4),0);
let t:=tag::<Int>();wt match t{Tag::T=>5,Tag::Data(_)=>0,};
"#,
        "",
        "3 4\n5\n",
    );
}

#[test]
fn copies_and_assignments_preserve_outer_identity_and_detach_payloads() {
    output(
        r#"
enum Cell { Vacant, Full(Vec<Int>, Vec<Int>) }
let row:=[1];let c:=Cell::Full(row,row);let alias:=c;let independent=c;
let held:=match c {Full(a,b)=>{a[0]=2;wt b;a} Vacant=>[],};
c=Cell::Vacant;held[0]=3;
wt match alias {Vacant=>true,Full(_,_)=>false,};
wt match independent {Full(a,b)=>{a[0]=4;wt b;a} Vacant=>[],};
c=Cell::Full(held,held);held[0]=8;
wt match c {Full(a,b)=>{a[0]=9;wt b;a} Vacant=>[],},held;
c=c;wt match alias {Full(a,b)=>{wt a;b} Vacant=>[],};
"#,
        "",
        "2\nYes\n1\n4\n3\n9 8\n9\n3\n",
    );
}

#[test]
fn enums_nest_in_records_vectors_dicts_and_other_enums() {
    output(
        r#"
enum Answer<T> { Found(T), Missing }
struct Holder<T> { value:T; }
let a:=Answer::Found(Some([1]));
let h:=Holder::<Answer<Option<Vec<Int>>>> {value:a};
let rows:Vec<Answer<Option<Vec<Int>>>>:=[h.value,Answer::Missing];
let detached=rows;
let d:=Dict::<Str,Answer<Option<Vec<Int>>>>();d.set("a",rows[0]);
rows[0]=Answer::Missing;
wt match d.get("a")?! {Found(o)=>o.is_none(),Missing=>true,};
wt match detached[0] {Found(o)=>o?!.len(),Missing=>-1,};
wt match Answer::<Int>::Missing {Found(n)=>n,_=>5,};
"#,
        "",
        "Yes\n1\n5\n",
    );
}

#[test]
fn match_evaluates_scrutinee_once_and_only_selected_arm() {
    output(
        r#"
enum Choice { A(Int), B, C }
let count:=0;
fn choose(n:Int)->Choice{n+=1;return Choice::A(n);}
wt match choose(count) {A(n)=>n,B=>count+=100,C=>count+=1000,},count;
let row:=[2];let e:=Some(row);
wt match e {Some(x)=>{e=None;x[0]+=1;x[0]} None=>0,},row;
"#,
        "",
        "1 1\n3 3\n",
    );
}

#[test]
fn returns_breaks_and_continues_from_arms_use_surrounding_control_flow() {
    output(
        r#"
enum Step { Skip, Add(Int), Stop }
fn extract(s:Step)->Int{let n:=match s{Add(n)=>n,Skip=>{return -1;}Stop=>{return -2;}};return n;}
wt extract(Step::Skip),extract(Step::Stop),extract(Step::Add(5));
let sum:=0;
outer:for i in 0..5 {
    let s:=if i==1{Step::Skip}else{if i==4{Step::Stop}else{Step::Add(i)}};
    let n:=match s {Skip=>{continue outer;}Stop=>{break outer;}Add(n)=>n,};
    sum+=n;
}wt sum;
"#,
        "",
        "-1 -2 5\n5\n",
    );
}

#[test]
fn option_compatibility_and_enum_constructors_interoperate() {
    output(
        r#"
let n:Option<Int>:=Option::None;wt n??3;
let a:=Option::Some(4);wt a?!,a.is_some(),!a;
let b:=Some(Some(7));wt match b {Some(inner)=>{match inner{Some(n)=>n,None=>0,}}None=>-1,};
wt None::<Int>()??8,Option::<Int>()??9;
let values:=[1,2,3];wt match values.find(2){Some(i)=>i,None=>-1,};
let d:=Dict::<Int,Int>();wt match d.get(0){Some(n)=>n,None=>10,};
wt match try_read::<Int>(){Some(n)=>n,None=>-1,};
wt match try_read::<Int>(){Some(n)=>n,None=>-1,};
"#,
        "12\n",
        "3\n4 Yes No\n7\n8 9\n1\n10\n12\n-1\n",
    );
}

#[test]
fn enum_payload_can_hold_closures_and_borrowed_characters() {
    output(
        r#"
enum Boxed<T> { Value(T), Empty }
let text:="abc";let c:=text[0];let f0:=()=>c;let e:=Boxed::Value(f0);
let f:=match e{Value(f)=>f,Empty=>f0,};
e=Boxed::Empty;c:='Q';text:="new";f()='Z';wt text,f();
let borrows:=Boxed::Value(text[0]);let old:=match borrows{Value(c)=>c,Empty=>'?',};
borrows=Boxed::Empty;old:='x';text.push_back('!');wt old,text;
"#,
        "",
        "new Z\nx new!\n",
    );
}

#[test]
fn invalid_enums_patterns_and_inference_have_source_locations() {
    let cases = [
        ("enum E {}", "requires at least one variant"),
        ("enum E { A, A }", "duplicate enum variant"),
        ("enum E<T,T>{A(T)}", "duplicate type parameter"),
        (
            "enum E { A(Int), B } let e:=E::A(true);",
            "argument type mismatch",
        ),
        (
            "enum E { A(Int), B } let e:=E::A();",
            "argument count mismatch",
        ),
        (
            "enum E { A, B } let x:=E::A;wt match x{A=>1,};",
            "non-exhaustive match",
        ),
        (
            "enum E { A, B } let x:=E::A;wt match x{A=>1,A=>2,B=>3,};",
            "unreachable match arm",
        ),
        (
            "enum E { A, B } let x:=E::A;wt match x{_=>1,B=>2,};",
            "unreachable match arm",
        ),
        (
            "enum E { A(Int) } let x:=E::A(1);wt match x{A=>1,};",
            "payload count mismatch",
        ),
        (
            "enum E { A(Int) } let x:=E::A(1);wt match x{A((a,b))=>a,};",
            "expected a tuple",
        ),
        (
            "enum E { A(Int,Int) } let x:=E::A(1,2);wt match x{A(a,a)=>a,};",
            "duplicate variable",
        ),
        (
            "enum E { A, B } let x:=E::A;wt match x{A=>1,B=>false,};",
            "type mismatch",
        ),
        (
            "enum E { A } enum F { A } let x:=E::A;wt match x{F::A=>1,};",
            "another enum",
        ),
        ("enum E { A } let x:=E::Missing;", "unknown enum variant"),
        (
            "enum E<T>{A(T),B} let x:=E::B;",
            "cannot infer enum type parameter",
        ),
        (
            "enum E<T>{A(T)} let x:=E::<Int,Int>::A(1);",
            "type argument count mismatch",
        ),
        (
            "enum E { A(Int) } let x:=E::A(1);match x{A(n)=>{wt n;}}wt n;",
            "unknown",
        ),
        ("enum E { A(E) }", "recursive enum"),
        ("struct R{e:E;} enum E{A(R)}", "recursive enum"),
        ("enum E<T>{A(E<Vec<T>>)} let x:E<Int>;", "recursive enum"),
        ("enum E<T>{A(E<(T,T)>)} let x:E<Int>;", "recursive enum"),
        ("enum Option<T>{Other(T)}", "duplicate type"),
    ];
    for (source, message) in cases {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "unexpected success: {source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            error.contains("main.rusk:") && error.contains(message),
            "{source}\n{error}"
        );
    }
}

#[test]
fn enum_operations_are_target_independent_in_hir_and_lir() {
    let case = Case::new("enum E{A(Int),B} let e:=E::A(3);wt match e{A(n)=>n,B=>0,};");
    for stage in ["hir", "lir"] {
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let ir = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(ir.contains("Variant") && ir.contains("Enum"), "{ir}");
        assert!(
            !ir.contains("std::") && !ir.contains("OptionPresent") && !ir.contains("OptionValue")
        );
    }
}

#[test]
fn enum_copy_hooks_follow_logical_types_and_payload_order() {
    output(
        r#"
extern symbol observation { code:"std::int64_t copy_trace=0;", }
extern type Score { cpp_name:"std::int64_t", }
extern fn score(n:Int)->Score {
    cpp_name:"make_score",
    code:"Obj<std::int64_t> make_score(Obj<std::int64_t> n,Loc){return n;}",
}
extern fn copy(n:Score)->Score {
    cpp_name:"copy_score",
    code:"Obj<std::int64_t> copy_score(Obj<std::int64_t> n,Loc){copy_trace=copy_trace*10+*n;return make_obj<std::int64_t>(*n);}",
}
extern fn trace()->Int {
    cpp_name:"read_copy_trace",
    code:"Obj<std::int64_t> read_copy_trace(Loc){return make_obj<std::int64_t>(copy_trace);}",
}
enum Scores { Pair(Score,Int,Score), Empty }
let s:=Scores::Pair(score(1),9,score(2));wt trace();
let detached=s;wt trace();
s=Scores::Empty;wt trace();
s=detached;wt trace();
"#,
        "",
        "0\n12\n12\n1212\n",
    );
}

#[test]
fn deeply_nested_finite_enums_do_not_repeat_cycle_analysis_exponentially() {
    let mut ty = "Int".to_string();
    for _ in 0..24 {
        ty = format!("Option<{ty}>");
    }
    let case = Case::new(&format!("let nested:{ty}:=None;wt nested.is_none();"));
    let out = case.compile();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
