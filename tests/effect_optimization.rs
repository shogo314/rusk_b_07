mod support;
use support::Case;
fn equivalent(source: &str, expected: &str) {
    let case = Case::new(source);
    let fast = case.run("");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert!(
        fast.status.success(),
        "{}",
        String::from_utf8_lossy(&fast.stderr)
    );
    assert_eq!(fast.status.code(), plain.status.code());
    assert_eq!(fast.stdout, expected.as_bytes());
    assert_eq!(fast.stdout, plain.stdout);
    assert_eq!(fast.stderr, plain.stderr);
}
fn signature<'a>(cpp: &'a str, name: &str) -> &'a str {
    let lines: Vec<_> = cpp.lines().collect();
    let index = lines
        .iter()
        .position(|line| {
            line.starts_with("// Rusk function ") && line.ends_with(&format!(": {name}"))
        })
        .unwrap();
    lines[index + 1]
}
#[test]
fn inferred_contracts_select_borrowing_without_requiring_annotations() {
    let case = Case::new(
        r#"
fn plain(x:Int)->Int{return x+1;}
fn annotated(readonly noescape x:Int)->Int{return x+1;}
fn write(x:Int){x+=1;}
fn escape_result(x:Int)->Int{return x;}
extern fn opaque(x:Int)->Int{cpp_name:"opaque",code:"Obj<std::int64_t> opaque(Obj<std::int64_t> x,Loc){return x;}",}
fn unknown(x:Int)->Int{return opaque(x);}
let x:=1;write(x);wt plain(x),annotated(x),escape_result(x),unknown(x);
"#,
    );
    let output = case.compile();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let fast = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    for name in ["plain", "annotated"] {
        assert!(signature(&fast, name).contains("const Obj<std::int64_t>& rusk_p0"));
    }
    for name in ["write", "escape_result", "unknown"] {
        assert!(!signature(&fast, name).contains("const Obj"));
    }
    let output = case.compile_with_options(&["--no-opt"]);
    assert!(output.status.success());
    let plain = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    for name in ["plain", "annotated"] {
        assert!(!signature(&plain, name).contains("const Obj"));
    }
}
#[test]
fn borrowed_arguments_keep_snapshots_rebinding_shared_updates_and_character_lifetimes() {
    equivalent(
        r#"
fn add(a:Int,b:Int)->Int{return a+b;}
fn after(a:Int,b:Int)->Int{b=8;return a+1;}
fn rebind(a:Int,b:Int)->Int{let saved:=a;a:=b;return saved+a;}
fn code(c:Char)->Int{return Int(c);}
let x:=1;wt add(x,{x:=4;x}),x;
wt after(x,x),x;wt rebind(x,2),x;
let s:="abc";wt code(s[0]);s.push_back('!');wt s;
let condition:=true;
wt if condition{condition=false;11}else{22},condition;
"#,
        "5 4\n9 8\n10 8\n97\nabc!\n11 No\n",
    );
    let case =
        Case::new("fn extend(c:Char,s:Str){wt c;s.push_back('!');}let s:=\"abc\";extend(s[0],s);");
    let fast = case.run("");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert!(!fast.status.success());
    assert_eq!(fast.status.code(), plain.status.code());
    assert_eq!(fast.stdout, b"a\n");
    assert_eq!(fast.stdout, plain.stdout);
    assert_eq!(fast.stderr, plain.stderr);
    assert!(String::from_utf8_lossy(&fast.stderr).contains("character references"));
}
#[test]
fn scalar_alias_elision_stops_at_mutation_rebinding_capture_storage_and_return() {
    equivalent(
        r#"
let a:=true;let alias:=a;alias=false;wt a,alias;
let b:=true;let old:=b;b:=false;wt b,old;
let captured:=true;let read:=()=>captured;captured=false;wt read();
let value:=true;let stored:=[value];value=false;wt stored;
fn identity(x:Bool)->Bool{return x;}
let x:=true;let y:=identity(x);x=false;wt x,y;
fn fresh()->Bool{let flag:=true;let alias:=flag;return alias;}
let result:=fresh();result=false;wt result;
let input:=true;let snapshot:=input && (input=false);wt snapshot,input;
"#,
        "No No\nNo Yes\nNo\nNo\nNo No\nNo\nNo No\n",
    );
}
fn allocations(program: &str) -> (i64, i64) {
    let source = format!(
        r##"
extern symbol counter {{
 code:"#include <new>\ninline std::int64_t allocation_total=0;\nvoid* operator new(std::size_t size){{++allocation_total;if(auto p=std::malloc(size))return p;throw std::bad_alloc();}}\nvoid operator delete(void* p)noexcept{{std::free(p);}}\nvoid operator delete(void* p,std::size_t)noexcept{{std::free(p);}}",
}}
extern fn allocations()->Int{{cpp_name:"allocations",code:"Obj<std::int64_t> allocations(Loc){{return make_obj<std::int64_t>(allocation_total);}}",}}
let before:=allocations();{program}let after:=allocations();wt total,after-before;
"##
    );
    let case = Case::new(&source);
    let read = |output: std::process::Output| {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let numbers: Vec<i64> = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(numbers[0], 1000);
        numbers[1]
    };
    (
        read(case.run("")),
        read(case.run_with_options("", &["--no-opt"])),
    )
}
#[test]
fn fresh_boolean_snapshots_and_variant_tests_avoid_per_iteration_allocations() {
    for source in [
        "let total:=0;let i:=0;while i<1000{if i>=0 && i<1000{total+=1;}i+=1;}",
        "enum Flag{On,Off}let flag:=Flag::On;let total:=0;let i:=0;while i<1000{if let Flag::On = flag {total+=1;}i+=1;}",
    ] {
        let (fast, plain) = allocations(source);
        assert!(fast < 30, "optimized allocations: {fast}");
        assert!(plain > 1000, "reference allocations: {plain}");
    }
}
