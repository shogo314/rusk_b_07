mod support;
use std::io::Write;
use std::process::{Command, Stdio};
use support::Case;

fn succeeds(source: &str, input: &str, expected: &str) {
    let out = Case::new(source).run(input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}
#[test]
fn input_and_output() {
    succeeds(
        "in a, b: Int; in s: Str; in c: Char; wt a+b, s, c, true, false; wt;",
        "10 +32 hello Z",
        "42 hello Z Yes No\n\n",
    );
    succeeds(r#"wt "a\u0000b", '\n';"#, "", "a\0b \n\n");
    succeeds(
        "in n: Int; wt read_line(); wt read_line();",
        "12 remainder\nnext\n",
        " remainder\nnext\n",
    );
}
#[test]
fn sharing_copy_rebinding_and_scope() {
    succeeds(
        "let a := 10; let b := a; let c := copy(a); a += 1; wt a,b,c; a := 20; wt a,b; a = 30; wt a,b; { let a := b; a = 7; } wt a,b; let z: Int; z := 5; wt z;",
        "",
        "11 11 10\n20 11\n30 11\n30 7\n5\n",
    );
}
#[test]
fn precedence_and_assignment_chains() {
    succeeds(
        "let a:=1; let b:=2; a = b = 3; wt a,b, 2+3*4, -(2+3); a := b := 8; b += 1; wt a,b;",
        "",
        "3 3 14 -5\n9 9\n",
    );
}
#[test]
fn evaluation_order_preserves_objects() {
    succeeds(
        r#"
extern fn pair(a: Int, b: Int) -> Int {
 cpp_name: "pair_values",
 code: "Obj<std::int64_t> pair_values(Obj<std::int64_t> a, Obj<std::int64_t> b, Loc) { return make_obj<std::int64_t>(*a * 10 + *b); }",
}
let a:=1; wt pair(a, a = 2); wt pair(a, a := 3); wt a;
"#,
        "",
        "22\n23\n3\n",
    );
}
#[test]
fn integer_floor_division_and_boundaries() {
    succeeds(
        "wt -7/3,-7%3,7/-3,7%-3,-7/-3,-7%-3; wt -9223372036854775808,9223372036854775807; wt -9223372036854775808%-1;",
        "",
        "-3 2 -3 -2 2 -1\n-9223372036854775808 9223372036854775807\n0\n",
    );
}
#[test]
fn runtime_errors_have_source_locations() {
    for expression in [
        "9223372036854775807+1",
        "-9223372036854775808-1",
        "9223372036854775807*2",
        "-(-9223372036854775808)",
        "-9223372036854775808/-1",
        "1/0",
        "1%0",
    ] {
        let out = Case::new(&format!("wt {expression};")).run("");
        assert!(!out.status.success(), "{expression}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:1:"));
    }
}
#[test]
fn invalid_input_is_rejected() {
    let case = Case::new("in n: Int; wt n;");
    // Compile once, exercise the same generated executable with multiple inputs.
    assert!(case.run("-9223372036854775808").status.success());
    for input in ["", "x", "12x", "+-1", "9223372036854775808"] {
        let mut child = Command::new(case.0.join("main"))
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(!out.status.success(), "{input:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:1:"));
    }
}
#[test]
fn compile_errors_are_located_and_do_not_write_output() {
    for source in [
        "wt missing;",
        "let a: Int; wt a;",
        "let a:=1; let a:=2;",
        "let a:=1; a:=true;",
        "let a: Missing;",
        "let a;",
        "wt 9223372036854775808;",
        "{ extern type X { cpp_name: \"int\", } }",
        "wt ; @",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk"));
        assert!(!case.0.join("main.cpp").exists());
    }
}
#[test]
fn native_inplace_operator_is_preferred() {
    succeeds(
        r#"
extern fn __iadd__(a: Int, b: Int) {
 cpp_name: "custom_iadd",
 code: "void custom_iadd(Obj<std::int64_t> a, Obj<std::int64_t> b, Loc) { *a += *b + 100; }",
}
let a:=1; let b:=a; a+=2; wt a,b;
"#,
        "",
        "103 103\n",
    );
}

#[test]
fn custom_type_and_operator_are_defined_in_rusk() {
    succeeds(
        r#"
extern type Score { cpp_name: "double", }
extern fn score(x: Int) -> Score {
 cpp_name: "score_from_int",
 code: "Obj<double> score_from_int(Obj<std::int64_t> x, Loc) { return make_obj<double>(*x); }",
}
extern fn __add__(a: Score, b: Score) -> Score {
 cpp_name: "add_score",
 code: "Obj<double> add_score(Obj<double> a, Obj<double> b, Loc) { return make_obj<double>(*a + *b); }",
}
extern fn __write__(x: Score) {
 cpp_name: "write_score",
 code: "void write_score(Obj<double> x, Loc) { std::cout << *x; }",
}
wt score(2) + score(3);
"#,
        "",
        "5\n",
    );
}

#[test]
fn loops_can_read_and_sum_a_sequence() {
    succeeds(
        include_str!("../examples/sum.rusk"),
        "5\n1 -2 3 4 5\n",
        "11\n",
    );
    succeeds(
        "let a:=48; let b:=18; while b { let r:=a%b; a:=b; b:=r; } wt a;",
        "",
        "6\n",
    );
}

#[test]
fn conditionals_and_comparisons() {
    succeeds(
        r#"
in n: Int;
let result: Str;
if n < 0 { result := "negative"; } else { result := "nonnegative"; }
wt result;
wt 1==1, 1!=1, 1<=2, 2>1, 2>=2, "a"<"b", 'a'<'b', true==false;
if "" { wt "wrong"; } else { wt "empty"; }
if "hello" { wt "nonempty"; }
if false { wt "wrong"; }
if -1 { wt "nonzero"; }
"#,
        "-1",
        "negative\nYes No Yes Yes Yes Yes Yes No\nempty\nnonempty\nnonzero\n",
    );
}

#[test]
fn break_continue_and_loop_else() {
    succeeds(
        r#"
let i:=0;
let total:=0;
while i<5 {
    i+=1;
    if i==2 { continue; }
    total+=i;
} else { wt "finished",total; }
while true { break; } else { wt "wrong"; }
while false { wt "wrong"; } else { wt "zero"; }
loop { total+=1; if total==15 { break; } }
wt total;
let outer:=0;
while outer<2 {
    outer+=1;
    while true { break; } else { wt "wrong"; }
} else { wt "outer finished"; }
"#,
        "",
        "finished 13\nzero\n15\nouter finished\n",
    );
}

#[test]
fn control_flow_checks_definite_initialization() {
    for source in [
        "let x:Int; if true { x:=1; } wt x;",
        "let x:Int; if true { x:=1; } else { wt x; }",
        "let x:Int; while false { x:=1; } wt x;",
        "let x:Int; loop { x:=1; break; } wt x;",
        "let x:Int; while false {} else { x:=1; } wt x;",
        "break;",
        "continue;",
        "if true { let x:=1; } wt x;",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:"));
    }
    succeeds("let x:Int; while (x:=0) {} wt x;", "", "0\n");
}

#[test]
fn arrays_store_and_rebind_shared_elements() {
    succeeds(
        r#"
let x:=1;
let a:Vec<Int> := [x, 2];
let held:=a[0];
x=3; wt a;
a[0]=4; wt x,held,a;
let y:=10;
(a[0]):=y;
held=5; y=11;
wt a,x,held;
a[-1]+=1; wt a[-2],a[-1];
let b:=a;
push_back(b,x); x=6;
wt len(a),a;
let result: Int;
result := (a[0] := y := 12);
result=13;
wt a,y;
"#,
        "",
        "3 2\n4 4 4 2\n11 2 5 5\n11 3\n3 11 3 6\n13 3 6 13\n",
    );
}

#[test]
fn arrays_copy_values_and_preserve_only_outer_identity_on_assignment() {
    succeeds(
        r#"
let x:=1;
let a:Vec<Int> := [x,2];
let alias:=a;
let old_element:=a[0];
let independent:=copy(a);
a[0]=7; wt a, independent;
let source:Vec<Int> := [10,20];
a=source;
source[0]=99;
wt alias,old_element,source;
a[0]=11;
wt independent;
let before:=a[0];
a=a;
a[0]=12;
wt a,before;
a:=copy(source);
wt a,alias;
"#,
        "",
        "7 2 1 2\n10 20 7 99 20\n1 2\n12 20 11\n99 20 12 20\n",
    );
}

#[test]
fn array_element_lifetime_survives_reallocation_and_scope() {
    succeeds(
        r#"
let a:Vec<Int> := [1];
let held:=a[0];
let i:=0;
while i<200 { push_back(a, i+10); i+=1; }
held=5; wt a[0],len(a);
let escaped: Int;
{ let temporary:Vec<Int> := [9]; escaped:=temporary[0]; }
escaped+=1; wt escaped;
a=[]; held=6; wt len(a),held;
if a { wt "wrong"; } else { wt "empty"; }
if [1] { wt "nonempty"; }
"#,
        "",
        "5 201\n10\n0 6\nempty\nnonempty\n",
    );
}

#[test]
fn array_index_targets_are_evaluated_once_before_rhs() {
    succeeds(
        r#"
let a:Vec<Int> := [10,20,30];
let i:=0;
a[(i+=1)]+= (i+=1);
wt a,i;
a[0] = (a[0] := 99);
wt a;
let x:=1;
let literal := [x, (x:=2)];
wt literal;
let b:Vec<Int> := [3];
a[0] := (b[0] := 4);
b[0]=5; wt a,b;
"#,
        "",
        "10 22 30 2\n99 22 30\n1 2\n5 22 30 5\n",
    );
}

#[test]
fn invalid_arrays_are_rejected_by_rusk() {
    for source in [
        "let a := [];",
        "let a:Vec<Missing> := [];",
        "let a := [1,true];",
        "let a := [[],[1]];",
        "let a := [1]; wt a[true];",
        "let a := [1]; a[0]:=false;",
        "let a := [1]; push_back(a,false);",
        "let a := [1]; wt a[0][0];",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:"));
    }
    for source in [
        "let a:Vec<Int>:=[]; wt a[0];",
        "let a:Vec<Int>:=[1]; wt a[-2];",
        "let a:Vec<Int>:=[1]; a[1]:=2;",
        "let a:Vec<Int>:=[1]; a[1]=2;",
        "let a:Vec<Int>:=[1]; wt a[-9223372036854775808];",
        "let a:Vec<Int>:=[1]; wt a[9223372036854775807];",
    ] {
        let out = Case::new(source).run("");
        assert!(!out.status.success(), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("main.rusk:1:"), "{error}");
        assert!(error.contains("array index out of range"), "{error}");
    }
}

#[test]
fn reverse_array_example() {
    succeeds(
        include_str!("../examples/array.rusk"),
        "5 1 2 3 4 5",
        "5 4 3 2 1\n",
    );
    succeeds(include_str!("../examples/array.rusk"), "0", "\n");
}

#[test]
fn empty_arrays_use_parameter_context_and_concrete_external_types() {
    succeeds(
        r#"
fn empty_size(a: Vec<Int>) -> Int { return a.len(); }
extern fn first(a: Vec<Int>) -> Int {
    cpp_name: "first_vec_int",
    code: "Obj<std::int64_t> first_vec_int(Obj<IntVec> a, Loc loc) { return (*a)[vec_int_index(*a, 0, loc)]; }",
}
let a: Vec<Int> := [];
push_back(a,1);
first(a)=2;
wt empty_size([]),a;
__assign__(a,[]);
wt len(a);
"#,
        "",
        "0 2\n0\n",
    );
}

#[test]
fn repeated_arrays_copy_each_element_independently() {
    succeeds(
        r#"
let x:=7;
let a := [x;3];
a[0]=10;
x=8;
wt a,x;
let n:=0;
let empty := [(n+=1);0];
wt empty.len(),n;
let repeated := [(n+=1);(n+=1)];
wt repeated,n;
n+=1;
wt repeated,n;
"#,
        "",
        "10 7 7 8\n0 1\n3 3 3 3\n3 3 3 4\n",
    );
}

#[test]
fn method_calls_desugar_to_functions_and_preserve_evaluation_order() {
    succeeds(
        r#"
let a := [1,2];
a.push_back(3);
wt a.len(), a.copy()[1], a[0].copy();
let independent:=a.copy();
independent[0]=9;
wt a,independent;
let old:=a;
a.push_back((a:=[8])[0]);
wt old,a;
let value:=1;
wt value.__add__(2), value;
[0;0].len().__write__(); wt;
"#,
        "",
        "3 2 1\n1 2 3 9 2 3\n1 2 3 8 8\n3 1\n0\n",
    );
}

#[test]
fn invalid_repetition_and_method_calls_are_located() {
    for source in [
        "let a := [0;true];",
        "let a := [1;false];",
        "[1].missing();",
        "[1].len(2);",
        "let a := [1;2,3];",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk"));
    }
    for (source, message) in [
        ("let a := [0;-1];", "negative array length"),
        (
            "let a := [0;9223372036854775807];",
            "array length out of range",
        ),
    ] {
        let out = Case::new(source).run("");
        assert!(!out.status.success(), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("main.rusk:1:"), "{error}");
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn frequency_example_uses_independent_counters() {
    succeeds(
        include_str!("../examples/frequency.rusk"),
        "6 4\n0 1 0 3 1 0\n",
        "3 2 0 1\n",
    );
}

#[test]
fn rusk_functions_support_recursion_forward_calls_and_overloads() {
    succeeds(
        r#"
wt factorial(6), even(10), even(11);
fn factorial(n: Int) -> Int {
    if n<=1 { return 1; }
    return n*factorial(n-1);
}
fn even(n: Int) -> Bool {
    if n==0 { return true; } else { return odd(n-1); }
}
fn odd(n: Int) -> Bool {
    if n==0 { return false; } else { return even(n-1); }
}
fn measure(x: Int) -> Int { return x+1; }
fn measure(x: Vec<Int>) -> Int { return x.len(); }
wt measure(3), [1,2].measure();
"#,
        "",
        "720 Yes No\n4 2\n",
    );
    succeeds(include_str!("../examples/gcd.rusk"), "48 18", "6\n");
}

#[test]
fn function_arguments_and_returns_preserve_sharing() {
    succeeds(
        r#"
fn update(x: Int) { x=5; x:=99; }
fn first(a: Vec<Int>) -> Int { return a[0]; }
fn make() -> Vec<Int> { let a := [1,2]; return a; }
fn reset(a: Vec<Int>) { a=[7,8]; a:=[9]; }
let x:=1; update(x); wt x;
let a:=make(); let held:=a[0];
first(a)=4; wt a,held;
reset(a); wt a,held;
let saved:=make()[0]; saved+=1; wt saved;
"#,
        "",
        "5\n4 2 4\n7 8 4\n2\n",
    );
}

#[test]
fn function_returns_handle_empty_arrays_unit_and_early_exit() {
    succeeds(
        r#"
fn empty() -> Vec<Int> { return []; }
fn show(x: Int) {
    if x<0 { wt "negative"; return; }
    wt x;
}
fn done() -> Unit { return; }
fn delegate() { return done(); }
let a:=empty(); wt a.len();
show(-1); show(3); delegate();
"#,
        "",
        "0\nnegative\n3\n",
    );
}

#[test]
fn user_functions_extend_operators_with_existing_resolution() {
    succeeds(
        r#"
fn __iadd__(x: Int, y: Int) { x = x+y+10; }
let a:=1; a+=2; wt a;
"#,
        "",
        "13\n",
    );
}

#[test]
fn invalid_function_definitions_and_returns_are_located() {
    for source in [
        "return;",
        "return 1;",
        "fn f() -> Int {}",
        "fn f(x:Bool) -> Int { if x { return 1; } }",
        "fn f() -> Int { return; }",
        "fn f() { return 1; }",
        "fn f() -> Int { return true; }",
        "fn f(x:Int,x:Int) {}",
        "fn f(x:Int) {} fn f(x:Int) {}",
        "fn f() -> Int { return 1; } fn f() -> Bool { return true; }",
        "fn f(x:Missing) {}",
        "fn f() -> Missing { return 1; }",
        "fn outer() { fn inner() {} }",
        "{ fn nested() {} }",
        "let global:=1; fn f() -> Int { return global; }",
        "fn unused() { wt missing; }",
        "fn f() { break; }",
        "fn f() { continue; }",
        "fn f(x:Int) { let x:=1; }",
        "fn f() { extern type Local { cpp_name: \"int\", } }",
        "fn f() { let x:Int; wt x; }",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("main.rusk:"),
            "{source}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!case.0.join("main.cpp").exists());
    }
}

#[test]
fn runtime_errors_in_functions_report_the_operation_position() {
    let out = Case::new("fn divide(x: Int) -> Int {\n return 1/x;\n}\nwt divide(0);").run("");
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("main.rusk:2:"), "{error}");
}

#[test]
fn initialization_merges_only_function_branches_that_continue() {
    succeeds(
        r#"
fn choose(flag: Bool) -> Int {
    let x: Int;
    if flag { return 1; } else { x:=2; }
    return x;
}
fn choose_other(flag: Bool) -> Int {
    let x: Int;
    if flag { x:=3; } else { return 4; }
    return x;
}
wt choose(true),choose(false),choose_other(true),choose_other(false);
"#,
        "",
        "1 2 3 4\n",
    );
}

#[test]
fn prototype_multidimensional_vectors_and_basic_element_types() {
    succeeds(
        r#"
let grid := [[0;3];2];
grid[0][1]=7; wt grid;
let independent:=copy(grid); grid[0][1]=8; wt independent;
let alias:=grid[0]; grid=[[9]]; wt grid,alias;
let flags := [true,false]; wt flags, !flags;
let words := ["b","a"]; words.sort(); wt words,words.contains("a");
let letters := ['b','a']; letters.sort(); wt letters;
let nested := [[2],[1,3],[1,2]]; nested.sort(); wt nested;
wt nested==[[1,2],[1,3],[2]];
"#,
        "",
        "0 7 0 0 0 0\n0 7 0 0 0 0\n9 0 8 0\nYes No No\na b Yes\na b\n1 2 1 3 2\nYes\n",
    );
}

#[test]
fn prototype_for_ranges_and_arrays_preserve_progress_and_sharing() {
    succeeds(
        r#"
let saved:Vec<Int>:=[];
let end:=3;
for i in 0..end { saved.push_back(i); i=99; end=0; }
wt saved;
let a:Vec<Int>:=[];
for i in 0..3 { a.push_back(i); i:=10; }
wt a;
for x in a { x+=1; } wt a;
for row in [[1],[2]] { row[0]+=10; wt row; }
let total:=0;
for i in 0..5 { if i==2 { continue; } total+=i; } else { wt total; }
for i in 0..3 { break; } else { wt "wrong"; }
for i in 3..0 { wt "wrong"; } else { wt "empty"; }
for i in 9223372036854775806..9223372036854775807 { wt i; }
"#,
        "",
        "99 99 99\n0 1 2\n1 2 3\n11\n12\n8\nempty\n9223372036854775806\n",
    );
}

#[test]
fn prototype_short_circuit_and_typed_input() {
    succeeds(
        r#"
in n:Int;
in a(n):Vec<Int>;
in flag:Bool;
wt a,flag;
let x:=0;
wt false && (x=1), true || (x=2),x;
wt !(0),!"", (1<2 && 2<3) || false;
"#,
        "3 1 2 3 Yes",
        "1 2 3 Yes\nNo Yes 0\nYes Yes Yes\n",
    );
}

#[test]
fn prototype_strings_and_explicit_conversions() {
    succeeds(
        r#"
let s:="abc";
s[1]='z'; wt s,s[-1];
{ let c:=s[0]; c='A'; }
s.push_back('d'); s+="ef"; wt s,s.len(),s.contains("ze");
if s[0]=='A' { s.push_back('!'); }
wt s;
wt Int('A'),Char(66),Int(true),Bool(0),Str(123),Str('x'),Int("-12");
wt 'c'-'a','a'+2,'c'-2;
let detached:=s.copy(); detached[0]='x'; wt s,detached;
"#,
        "",
        "azc c\nAzcdef 6 No\nAzcdef!\n65 B 1 No 123 x -12\n2 c a\nAzcdef! xzcdef!\n",
    );
}

#[test]
fn prototype_sort_search_and_concatenation() {
    succeeds(
        r#"
let x:=3; let a:=[x,1,2,2]; let b:=a.sorted();
wt a,b,b.bisect_left(2),b.bisect_right(2),b.contains(9);
b[3]=7; wt x;
a.sort(); wt a; a.reverse(); wt a;
let c:=a+[8]; c[0]=9; wt a,c;
a+=a; wt a.len();
"#,
        "",
        "3 1 2 2 1 2 2 3 1 3 No\n7\n1 2 2 7\n7 2 2 1\n9 2 2 1 9 2 2 1 8\n8\n",
    );
}

#[test]
fn prototype_mutation_and_type_errors_are_diagnosed() {
    for (source, message) in [
        (
            "let a:=[1]; for x in a { a.push_back(2); }",
            "container changed during iteration",
        ),
        (
            "let a:=[1,2]; for x in a { a.sort(); }",
            "container changed during iteration",
        ),
        (
            "let s:=\"abc\"; let c:=s[0]; s.push_back('x');",
            "active character references",
        ),
        (
            "let s:=\"abc\"; let c:=s[0]; s=\"longer\";",
            "active character references",
        ),
        ("wt Char(128);", "ASCII range"),
        ("wt 'a'+100;", "ASCII range"),
        ("wt Int(\"12x\");", "integer conversion"),
        ("in a(-1):Vec<Int>;", "negative array length"),
    ] {
        let out = Case::new(source).run("");
        assert!(!out.status.success(), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains(message), "{source}: {error}");
        assert!(error.contains("main.rusk:"), "{error}");
    }
    for source in [
        "for x in 3 {}",
        "let s:=\"a\"; s[0]:='x';",
        "let x:Int; true || (x:=1); wt x;",
        "in a(3):Vec<Vec<Int>>;",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:"));
    }
}

#[test]
fn prototype_competitive_programming_examples() {
    succeeds(
        include_str!("../examples/prototype/array_sum.rusk"),
        "5 1 -2 3 4 5",
        "11\n",
    );
    succeeds(
        include_str!("../examples/prototype/sort_search.rusk"),
        "5 4 3 1 3 2 5 0 2 3 9",
        "0\n2\n4\n5\n",
    );
    succeeds(
        include_str!("../examples/prototype/grid_bfs.rusk"),
        "3 4\n....\n.##.\n....\n",
        "5\n",
    );
    succeeds(
        include_str!("../examples/prototype/pow_mod.rusk"),
        "3 13 1000",
        "323\n",
    );
}

#[test]
fn prototype_grid_bfs_matches_an_independent_oracle() {
    use std::collections::VecDeque;
    let case = Case::new(include_str!("../examples/prototype/grid_bfs.rusk"));
    let built = case.run("1 1\n.\n");
    assert!(built.status.success());
    let mut seed = 7u64;
    for _ in 0..30 {
        let h = 6;
        let w = 7;
        let mut grid = vec![vec![b'.'; w]; h];
        for row in &mut grid {
            for c in row {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                if seed >> 61 == 0 {
                    *c = b'#';
                }
            }
        }
        let mut distance = vec![vec![-1; w]; h];
        let mut queue = VecDeque::new();
        if grid[0][0] == b'.' {
            distance[0][0] = 0;
            queue.push_back((0, 0));
        }
        while let Some((r, c)) = queue.pop_front() {
            for (dr, dc) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let nr = r as isize + dr;
                let nc = c as isize + dc;
                if nr >= 0 && nr < h as isize && nc >= 0 && nc < w as isize {
                    let nr = nr as usize;
                    let nc = nc as usize;
                    if grid[nr][nc] == b'.' && distance[nr][nc] < 0 {
                        distance[nr][nc] = distance[r][c] + 1;
                        queue.push_back((nr, nc));
                    }
                }
            }
        }
        let rows = grid
            .iter()
            .map(|r| String::from_utf8(r.clone()).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let out = case.execute(&format!("{h} {w}\n{rows}\n"));
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            format!("{}\n", distance[h - 1][w - 1])
        );
    }
}

#[test]
fn prototype_sort_search_matches_linear_counting() {
    let case = Case::new(include_str!("../examples/prototype/sort_search.rusk"));
    let built = case.run("0 1 10");
    assert!(built.status.success());
    let mut seed = 19u64;
    for size in [0, 1, 2, 15, 80] {
        let mut values = vec![];
        for _ in 0..size {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            values.push(((seed >> 32) % 31) as i64 - 15);
        }
        let queries: Vec<_> = (-20..=20).collect();
        let expected = queries
            .iter()
            .map(|q| format!("{}\n", values.iter().filter(|v| *v <= q).count()))
            .collect::<String>();
        let input = format!(
            "{} {} {} {}",
            values.len(),
            queries.len(),
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
            queries
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        );
        let out = case.execute(&input);
        assert!(out.status.success());
        assert_eq!(out.stdout, expected.as_bytes());
    }
}

#[test]
fn prototype_convenience_operations_and_temporary_borrows() {
    succeeds(
        r#"
wt Int(),Bool(),Char("A"),Str(),Str(['a','b']),Bool([1]);
let initial:=10; wt [1,2].sum(initial),initial,sum([]);
fn append(s: Str) -> Bool { s.push_back('!'); return true; }
let s:="abc";
if s[0]=='a' && append(s) { wt s; }
let grid:Vec<Vec<Bool>> := [[false;2];2];
grid[0][0]=true; wt grid;
let empty_rows:Vec<Vec<Int>> := [[];2];
empty_rows[0].push_back(1); wt empty_rows[0],empty_rows[1].len();
"#,
        "",
        "0 No A  ab Yes\n13 10 0\nabc!\nYes No No No\n1 0\n",
    );
    let out = Case::new("wt [9223372036854775807,1,-1].sum();").run("");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("integer overflow"));
}

#[test]
fn sharing_matches_an_independent_object_model() {
    let case = Case::new(
        r#"
let slots := [0;8];
in n:Int;
for i in 0..n {
    in op,a,b,value:Int;
    if op==0 { slots[a]:=value; }
    if op==1 { slots[a]=value; }
    if op==2 { slots[a]:=slots[b]; }
    if op==3 { slots[a]+=value; }
    if op==4 { slots[a]:=copy(slots[b]); }
    wt slots;
}
"#,
    );
    let mut objects = vec![0i64; 8];
    let mut slots: Vec<usize> = (0..8).collect();
    let mut input = String::from("300\n");
    let mut expected = String::new();
    let mut seed = 314u64;
    for _ in 0..300 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed >> 32
        };
        let op = next() % 5;
        let a = (next() % 8) as usize;
        let b = (next() % 8) as usize;
        let value = (next() % 21) as i64 - 10;
        input.push_str(&format!("{op} {a} {b} {value}\n"));
        match op {
            0 | 4 => {
                let new_value = if op == 0 { value } else { objects[slots[b]] };
                slots[a] = objects.len();
                objects.push(new_value);
            }
            1 => objects[slots[a]] = value,
            2 => slots[a] = slots[b],
            3 => objects[slots[a]] += value,
            _ => unreachable!(),
        }
        expected.push_str(
            &slots
                .iter()
                .map(|&id| objects[id].to_string())
                .collect::<Vec<_>>()
                .join(" "),
        );
        expected.push('\n');
    }
    let out = case.run(&input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn nested_copy_breaks_internal_aliases_and_preserves_old_references() {
    succeeds(
        r#"
let x:=1; let row:=[x,x]; let grid:=[row,row];
let detached:=copy(grid);
detached[0][0]=7;
wt grid,detached;
let old_row:=grid[0]; let old_cell:=grid[0][0];
grid=grid;
grid[0][0]=8;
wt grid,old_row,old_cell;
let repeated:=[row;2]; repeated[0][0]=9;
wt repeated,row;
"#,
        "",
        "1 1 1 1 7 1 1 1\n8 1 1 1 1 1 1\n9 1 1 1 1 1\n",
    );
}

#[test]
fn character_references_escape_owners_and_release_after_rebinding() {
    succeeds(
        r#"
fn escaped() -> Char { let local:="abc"; return local[1]; }
let c:=escaped(); c='Z'; wt c;
let s:="abc"; let owner:=s;
let first:=s[0]; let alias:=first;
s:="other"; owner="XYZ"; wt first,alias,s;
first:='q'; alias:='r';
owner.push_back('!'); wt owner,first,alias;
let detached:=copy(owner[0]); owner.push_back('?'); wt detached,owner;
"#,
        "",
        "Z\nX X other\nXYZ! q r\nX XYZ!?\n",
    );
}

#[test]
fn character_borrows_survive_aliases_and_function_returns() {
    for source in [
        "let s:=\"abc\"; let c:=s[0]; let d:=c; c:='x'; s.push_back('!');",
        "fn first(s:Str)->Char { return s[0]; } let s:=\"abc\"; let c:=first(s); s+=\"!\";",
        "let s:=\"abc\"; let chars:=[s[0]]; s=\"x\";",
    ] {
        let out = Case::new(source).run("");
        assert_eq!(out.status.code(), Some(1), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            error.contains("active character references"),
            "{source}: {error}"
        );
        assert!(error.contains("main.rusk:1:"), "{error}");
    }
}

#[test]
fn temporary_character_borrows_release_on_control_flow_exits() {
    succeeds(
        r#"
fn first(s:Str)->Char { return s[0]; }
fn touch(s:Str) { let c:=s[0]; c='A'; return; }
let s:="a";
while s[0]=='a' { s.push_back('!'); s[0]='b'; continue; }
if first(s)=='b' { s.push_back('?'); }
for i in 0..3 { let c:=s[0]; if i==1 { break; } continue; }
s.push_back('.');
touch(s); s.push_back('#'); wt s;
let t:="a";
wt false && (t[0]=='a'), true || (t[0]=='a');
t.push_back('!'); wt t;
"#,
        "",
        "A!?.#\nNo Yes\na!\n",
    );
}

#[test]
fn iterators_detect_structural_changes_through_aliases() {
    for mutation in [
        "alias.reverse();",
        "alias += [3];",
        "alias=[4];",
        "alias=alias;",
    ] {
        let source = format!("let a:=[1,2]; let alias:=a; for x in a {{ {mutation} }}");
        let out = Case::new(&source).run("");
        assert_eq!(out.status.code(), Some(1), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            error.contains("container changed during iteration"),
            "{source}: {error}"
        );
        assert!(error.contains("main.rusk:1:"), "{error}");
    }
}

#[test]
fn iterators_keep_original_owner_and_observe_element_rebinding() {
    succeeds(
        r#"
let a:=[1,2,3]; let original:=a; let seen:Vec<Int>:=[];
for x in a {
    seen.push_back(x);
    if x==1 { a:=[9]; original[1]:=7; }
}
wt seen,a,original;
let outer:=0;
for i in 0..2 {
    for j in 0..2 { break; } else { wt "wrong"; }
    outer+=1;
} else { wt outer; }
"#,
        "",
        "1 7 3 9 1 7 3\n2\n",
    );
}

#[test]
fn integer_division_matches_floor_identity_for_all_small_sign_combinations() {
    let case = Case::new("in n:Int; for i in 0..n { in a,b:Int; wt a/b,a%b; }");
    let mut pairs = vec![];
    for a in -12i64..=12 {
        for b in -7i64..=7 {
            if b != 0 {
                pairs.push((a, b));
            }
        }
    }
    let mut input = format!("{}\n", pairs.len());
    let mut expected = String::new();
    for (a, b) in pairs {
        // Find floor(a/b) by its defining inequalities, without division.
        let q = (-12..=12)
            .find(|&q| {
                if b > 0 {
                    q * b <= a && a < (q + 1) * b
                } else {
                    q * b >= a && a > (q + 1) * b
                }
            })
            .unwrap();
        input.push_str(&format!("{a} {b}\n"));
        expected.push_str(&format!("{q} {}\n", a - b * q));
    }
    let out = case.run(&input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn objects_release_when_the_last_owner_leaves_scope() {
    succeeds(
        r#"
extern symbol lifetime_observer {
    code: "std::vector<std::weak_ptr<std::int64_t>> observed;",
}
extern fn watch(x:Int) {
    cpp_name: "watch_object",
    code: "void watch_object(Obj<std::int64_t> x, Loc) { observed.push_back(x); }",
}
extern fn living() -> Int {
    cpp_name: "living_objects",
    code: "Obj<std::int64_t> living_objects(Loc) { std::int64_t n=0; for (const auto& x:observed) if(!x.expired()) ++n; return make_obj<std::int64_t>(n); }",
}
fn make() -> Int { let x:=5; watch(x); return x; }
fn early() { let x:=6; watch(x); return; }
watch(1+2); wt living();
{
    let a:=make(); let b:=a;
    a:=0; wt living();
    b:=0; wt living();
}
let held:Vec<Int>:=[];
{ let x:=7; watch(x); held.push_back(x); }
wt living(); held=[]; wt living();
early(); wt living();
for i in 0..3 { watch(i); continue; }
wt living();
loop { let x:=8; watch(x); break; }
wt living();
"#,
        "",
        "0\n1\n0\n1\n0\n0\n0\n0\n",
    );
}

#[test]
fn copy_initializers_are_independent_while_shared_initializers_alias() {
    succeeds(
        r#"
let h:=10;
let top = h;
let shared := h;
top=3; shared=20;
wt h,top,shared;
let flag:=true; let detached_flag = flag; detached_flag=false;
let character:='a'; let detached_character = character; detached_character='z';
let word:="abc"; let detached_word = word; detached_word[0]='X';
wt flag,detached_flag,character,detached_character,word,detached_word;
{ let h = h; h=99; wt h; }
wt h;
fn first_copy(a:Vec<Int>)->Int { let x = a[0]; x+=1; return x; }
let values:=[7]; wt first_copy(values),values;
"#,
        "",
        "20 3 20\nYes No a z abc Xbc\n99\n20\n8 7\n",
    );
}

#[test]
fn copy_initializers_preserve_type_context_and_deep_copy_arrays() {
    succeeds(
        r#"
let empty:Vec<Int> = [];
let rows:Vec<Vec<Int>> = [[];2];
rows[0].push_back(1); wt empty.len(),rows[0],rows[1].len();
let x:=1; let original:=[[x,x],[x]];
let detached:Vec<Vec<Int>> = original;
detached[0][0]=9; x=2;
wt original,detached;
let words = ["a","b"]; let other = words;
other[0][0]='Z'; wt words,other;
let flags:Vec<Bool> = []; flags.push_back(true); wt flags;
"#,
        "",
        "0 1 0\n2 2 2 9 1 1\na b Z b\nYes\n",
    );
}

#[test]
fn copy_initializers_evaluate_rhs_once_and_release_character_borrows() {
    succeeds(
        r#"
let count:=0;
let saved = (count+=1);
count+=1; wt saved,count;
let source:=10;
let result = (source:=20);
source=30; wt result,source;
let s:="abc";
let letter:Char = s[0];
s.push_back('!'); letter='Z'; wt s,letter;
let empty:Vec<Int> = [(count+=1);0]; wt count,empty.len();
"#,
        "",
        "1 2\n20 30\nabc! Z\n3 0\n",
    );
}

#[test]
fn copy_initializers_use_the_type_copy_function() {
    succeeds(
        r#"
extern symbol copy_observer { code: "std::int64_t copies=0;", }
extern type Score { cpp_name: "std::int64_t", }
extern fn score() -> Score {
    cpp_name: "make_score",
    code: "Obj<std::int64_t> make_score(Loc) { return make_obj<std::int64_t>(7); }",
}
extern fn copy(x:Score) -> Score {
    cpp_name: "copy_score",
    code: "Obj<std::int64_t> copy_score(Obj<std::int64_t> x, Loc) { ++copies; return make_obj<std::int64_t>(*x); }",
}
extern fn copy_count() -> Int {
    cpp_name: "copy_count",
    code: "Obj<std::int64_t> copy_count(Loc) { return make_obj<std::int64_t>(copies); }",
}
let original:=score();
let shared:=original; wt copy_count();
let detached = original; wt copy_count();
"#,
        "",
        "0\n1\n",
    );
}

#[test]
fn invalid_copy_initializers_report_rusk_errors() {
    for source in [
        "let x = [];",
        "let x:Int = true;",
        "let x = x;",
        "let x:Int; let y = x;",
        "let x += 1;",
        "let x:Int; x=1;",
        "extern type Score { cpp_name: \"double\", } extern fn score()->Score { cpp_name: \"score\", code: \"Obj<double> score(Loc) { return make_obj<double>(1); }\", } let x = score();",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("main.rusk:"), "{source}: {error}");
        assert!(!case.0.join("main.cpp").exists(), "{source}");
    }
}
