mod support;
use support::Case;

fn succeeds(source: &str, expected: &str) {
    let case = Case::new(source);
    let out = case.run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
    let plain = case.run_with_options("", &["--no-opt"]);
    assert_eq!(out.stdout, plain.stdout);
    assert_eq!(out.stderr, plain.stderr);
    assert_eq!(out.status.code(), plain.status.code());
}
fn rejects(source: &str, expected: &str) {
    let out = Case::new(source).compile();
    assert!(!out.status.success(), "{source}");
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(
        message.contains("main.rusk:") && message.contains(expected),
        "{message}"
    );
}

#[test]
fn fixed_restricts_the_name_while_updates_members_and_new_aliases_remain_mutable() {
    succeeds(
        r#"
fixed let x := 1;x = 2;x += 3;wt x;
let alias := x;alias := 9;wt x,alias;
rebindable let y := 1;y := 7;wt y;
fixed let pair := (1,2);pair[0] := 8;pair[1] = 9;wt pair;
fixed let values := [1];values[0] := 3;values.push_back(4);wt values;
fixed let (a,(b,c)) = (1,(2,3));a=10;b+=20;wt a,b,c;
{let x:=100;x:=101;wt x;}wt x;
"#,
        "5\n5 9\n7\n8 9\n3 4\n10 22 3\n101\n5\n",
    );
}

#[test]
fn parameters_generics_lambdas_and_captures_keep_binding_contracts() {
    succeeds(
        r#"
fn update(fixed x:Int)->Int{x+=1;return x;}
fn assign<T>(fixed target:T,source:T){target=source;}
fn replace(rebindable x:Int)->Int{x:=x+10;return x;}
fixed let n:=1;wt update(n),n;
assign(n,8);wt n,replace(n),n;
let f:=(fixed x:Int)=>{x+=2;x};wt f(3);
fixed let captured:=0;let increment:=()=>{captured+=1;captured};
wt increment(),increment(),captured;
let named:=update;wt named(n),n;
fixed let shadowed:=1;let g:=(shadowed:Int)=>{shadowed:=10;shadowed};wt g(0),shadowed;
"#,
        "2 2\n8 18 8\n5\n1 2 2\n9 9\n10 1\n",
    );
}

#[test]
fn uninitialized_fixed_bindings_can_be_initialized_on_disjoint_paths() {
    succeeds(
        r#"
fn choose(condition:Bool)->Int{
 fixed let answer:Int;
 if condition{answer:=3;}else{answer:=7;}
 return answer;
}
wt choose(true),choose(false);
fixed let x:Int;fixed let y:Int;(x,y):=(11,12);wt x,y;
fixed let once:Int;loop{once:=42;wt once;break;}
"#,
        "3 7\n11 12\n42\n",
    );
}

#[test]
fn fresh_declarations_reset_each_iteration_and_breaks_do_not_create_false_backedges() {
    succeeds(
        r#"
for i in 0..4 {
 fixed let local:Int;local:=i;
 if i==1{continue;}
 wt local;
}
fixed let once:Int;
outer:loop{loop{once:=8;wt once;break outer;}}
fixed let last:Int;while true{last:=9;wt last;break;}
"#,
        "0\n2\n3\n8\n9\n",
    );
}

#[test]
fn initialized_fixed_bindings_cannot_be_rebound_directly_or_through_captures() {
    for source in [
        "fixed let x:=1;x:=2;",
        "fixed let x=1;x:=x;",
        "fixed let (x,y):=(1,2);(x,y):=(3,4);",
        "fn f(fixed x:Int){x:=2;}f(1);",
        "fn f<T>(fixed x:T,y:T){x:=y;}f(1,2);",
        "let f:=(fixed x:Int)=>(x:=2);wt f(1);",
        "fixed let x:=1;let f:=()=>(x:=2);f();",
        "fixed let x:=1;let f:=()=>{let inner:=()=>(x:=2);inner()};f();",
        "fn f(){fixed let x:=1;return;x:=2;}",
    ] {
        rejects(source, "cannot rebind fixed variable");
    }
}

#[test]
fn conditional_initialization_and_loop_backedges_cannot_initialize_fixed_twice() {
    for source in [
        "fixed let x:Int;if true{x:=1;}x:=2;",
        "fixed let x:Int;x:={x:=1;2};",
        "fixed let x:Int;(x,x):=(1,2);",
        "fixed let x:Int;loop{x:=1;}",
        "fixed let x:Int;while true{x:=1;}",
        "fixed let x:Int;for i in 0..2{x:=i;}",
        "fixed let x:Int;loop{if true{x:=1;continue;}break;}",
        "fixed let x:Int;wt match true{true if {x:=1;false}=>0,_=>{x:=2;0},};",
    ] {
        rejects(source, "cannot rebind fixed variable");
    }
}

#[test]
fn modifier_conflicts_and_unsupported_external_binding_contracts_are_diagnosed() {
    for source in [
        "fixed fixed let x:=1;",
        "rebindable rebindable let x:=1;",
        "fn f(fixed fixed x:Int){}",
    ] {
        rejects(source, "duplicate binding modifier");
    }
    for source in [
        "fixed rebindable let x:=1;",
        "rebindable fixed let x:=1;",
        "let f:=(fixed rebindable x:Int)=>x;",
    ] {
        rejects(source, "conflicting binding modifiers");
    }
    rejects(
        r#"extern fn f(fixed x:Int){cpp_name:"f",code:"ignored",}"#,
        "binding modifiers on extern parameters require a Rusk function body",
    );
    rejects(
        "fn f(fixed x:Int){}fn f(rebindable x:Int){}",
        "duplicate function",
    );
}

#[test]
fn annotations_are_visible_in_ir_and_errors_point_to_assignment_and_declaration() {
    let source = "fixed let x: Int;\nif true {x := 1;}\nx := 2;\n";
    for stage in ["hir", "lir", "cpp"] {
        let case = Case::new(source);
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(!out.status.success());
        let message = String::from_utf8_lossy(&out.stderr);
        assert!(
            message.contains("main.rusk:3:") && message.contains("main.rusk:1:"),
            "{message}"
        );
        assert!(
            message.contains("binding declared here") && message.contains("fixed modifier is here"),
            "{message}"
        );
        assert!(!case.0.join("main.cpp").exists());
    }
    let case = Case::new("fixed let x:=1;rebindable let y:=x;y:=2;wt x,y;");
    for stage in ["ast", "hir", "lir"] {
        let out = case.compile_with_options(&["--emit", stage]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(dump.contains("Fixed") && dump.contains("Rebindable"));
        if stage != "ast" {
            assert!(dump.contains("AnnotateBinding"));
        }
    }
}
