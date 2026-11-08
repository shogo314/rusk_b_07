mod support;
use support::Case;

fn succeeds(source: &str, expected: &str) {
    let case = Case::new(source);
    let fast = case.run("");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert!(
        fast.status.success(),
        "{}",
        String::from_utf8_lossy(&fast.stderr)
    );
    assert_eq!(fast.stdout, expected.as_bytes());
    assert_eq!(fast.stdout, plain.stdout);
    assert_eq!(fast.stderr, plain.stderr);
    assert_eq!(fast.status.code(), plain.status.code());
}

#[test]
fn captures_copy_update_and_rebind_follow_object_semantics() {
    succeeds(
        r#"
let counter:=1;
let bump:=(amount:Int)=>(counter+=amount);
let shared:=bump;let detached=bump;
wt bump(2),shared(3),counter; wt detached(10),counter;
counter:=100; wt bump(1),counter;
let n:=5;let rebind:=()=> (n:=n+1);wt rebind(),rebind(),n;
let text:="abc";let f:=()=>text[0];let c:=f();c='Z';wt text;
let stored:=[bump];stored[0](1);wt shared(0);
struct Box<F>{callback:F;}
fn Box<F>(callback:F)->Box<F>{return Box::<F>{callback:callback};}
let boxed:=Box(bump);wt boxed.callback(2);
"#,
        "3 6 6\n11 6\n7 100\n6 6 5\nZbc\n8\n10\n",
    );
}

#[test]
fn typed_lambda_parameters_named_functions_and_nested_generics_work() {
    succeeds(
        r#"
fn square(x:Int)->Int{return x*x;}
fn apply<T,F>(value:T,operation:F)->T{return operation(value);}
fn twice<T,F>(value:T,operation:F)->T{return apply(operation(value),operation);}
let f:=square;let same:=square;f:=same;wt f(3),twice(2,square);
let x:=100;let shadow:=(x:Int)=>x+1;wt shadow(3),x;
wt ((x:Int)=>x*2)(7);
let captured:="hi";let nested:=(x:Int)=>apply(x,(y:Int)=>y+captured.len());wt nested(3);
let plus:=(a:Int,b:Int)=>a+b;assert(3.plus(4)==7);let len:=0;assert([1,2].len()==2);
let count:=0;let unit:=(x:Int)=>assert(x>0);unit(1);
"#,
        "9 16\n4 100\n14\n5\n",
    );
}

#[test]
fn closures_keep_captured_character_owners_alive() {
    succeeds(
        r#"
fn use<F>(operation:F)->Char{return operation();}
let text:="abc";let c:=text[1];let f:=()=>c;
c:='Q';text:="new";wt use(f),text;
let detached=f;let held:=use(f);held='Z';wt use(f),use(detached),text;
"#,
        "b new\nZ b new\n",
    );
}

#[test]
fn callable_errors_are_rusk_diagnostics() {
    for source in [
        "let x:Int;let f:=()=>x;",
        "let f:=(x:Int,x:Int)=>x;",
        "let f:=(x:Int)=>x;wt f(true);",
        "let f:=(x:Int)=>x;wt f();",
        "let f:=1;f();",
        "fn f(x:Int)->Int{return x;} fn f(x:Str)->Str{return x;}let g:=f;",
        "let a:=(x:Int)=>x;let b:=(x:Int)=>x+1;a:=b;",
        "let x:=1;let f:=()=>x;wt f.x;",
        "let x:=1;let f:=()=>x;f.x:=2;",
        "let x:=1;let f:=()=>x;f.x=2;",
        "let x:=1;let f:=()=>x;(f.x,x):=(2,3);",
        "let f:=()=>missing;",
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
