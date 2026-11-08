mod support;
use support::Case;

fn equivalent(source: &str, input: &str, expected: &str) {
    let case = Case::new(source);
    let fast = case.run(input);
    let plain = case.run_with_options(input, &["--no-opt"]);
    assert!(
        fast.status.success(),
        "{}",
        String::from_utf8_lossy(&fast.stderr)
    );
    assert_eq!(fast.status.code(), plain.status.code());
    assert_eq!(fast.stdout, plain.stdout);
    assert_eq!(fast.stderr, plain.stderr);
    assert_eq!(fast.stdout, expected.as_bytes());
}

#[test]
fn promoted_temporaries_preserve_sharing_and_evaluation_order() {
    equivalent(
        r##"
fn mutate(a:Int,b:Int)->Int { a=8; return b; }
fn first(a:Int,b:Int)->Int { return a; }
fn fresh()->Int { return 1+2; }
let a:=fresh(); let b:=a; a=4; wt a,b;
let c = a; c=7; wt a,c;
wt mutate(1+2,3+4),first(a,a=9),first(a,a:=10),a;
let pairs:=[(1+2,3+4),(5+6,7+8)];
let held:=pairs[0][0]; pairs[0][0]=12; wt held,pairs;
let x:=1; let y:=2; let old:=x;
(x,y)=(y+1,x+2); wt x,y,old;
wt ((1+2)=5),((1+2)+=4),((1+2)=mutate(4,5));
"##,
        "",
        "4 4\n4 7\n7 9 9 10\n12 12 7 11 15\n3 3 3\n5 7 5\n",
    );
}

#[test]
fn short_circuit_loops_and_scalar_boundaries_match_plain() {
    equivalent(
        r##"
let n:=0; let alias:=n;
while n<4 { n+=1; if n==2 { continue; } wt n; } else { wt alias; }
for i in -2..3 { if i==1 { break; } wt i*i; } else { wt 999; }
let x:=1; wt false && (x=2),true || (x=3),x;
wt -9223372036854775808,9223372036854775807;
wt -7/3,7/-3,-7%3,7%-3;
let s:="abc"; let c:=s[0]; wt c=='a',!c,copy(c); c:='z'; s.push_back('!'); wt s;
"##,
        "",
        "1\n3\n4\n4\n4\n1\n0\nNo Yes 1\n-9223372036854775808 9223372036854775807\n-3 -3 2 -2\nYes No a\nabc!\n",
    );
}

#[test]
fn optimized_failures_keep_diagnostics_and_prior_output() {
    for source in [
        "wt 42; wt 9223372036854775807+1;",
        "wt 42; wt -(-9223372036854775808);",
        "wt 42; wt -9223372036854775808/-1;",
        "wt 42; wt 1/0;",
        "wt 42; wt 1%0;",
        "wt 42; in x:Int;",
        "wt 42; let a:=[1]; for x in a { a.push_back(2); }",
    ] {
        let case = Case::new(source);
        let fast = case.run("invalid");
        let plain = case.run_with_options("invalid", &["--no-opt"]);
        assert!(!fast.status.success(), "{source}");
        assert_eq!(fast.status.code(), plain.status.code());
        assert_eq!(fast.stdout, b"42\n");
        assert_eq!(fast.stdout, plain.stdout);
        assert_eq!(fast.stderr, plain.stderr);
    }
}

#[test]
fn arithmetic_loop_reduces_actual_heap_allocations() {
    let case = Case::new(
        r##"
extern symbol allocation_counter {
 code:"#include <new>\ninline std::int64_t allocations=0;\nvoid* operator new(std::size_t size){ ++allocations; if(auto p=std::malloc(size)) return p; throw std::bad_alloc(); }\nvoid operator delete(void* p) noexcept { std::free(p); }\nvoid operator delete(void* p, std::size_t) noexcept { std::free(p); }",
}
extern fn allocation_count()->Int {
 cpp_name:"allocation_count",
 code:"Obj<std::int64_t> allocation_count(Loc){ return make_obj<std::int64_t>(allocations); }",
}
let before:=allocation_count();
let total:=0; for i in 0..10000 { total+=i*2+1; }
let after:=allocation_count(); wt total,after-before;
"##,
    );
    let fast = case.run("");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert!(fast.status.success() && plain.status.success());
    let numbers = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .split_whitespace()
            .map(|x| x.parse::<u64>().unwrap())
            .collect::<Vec<_>>()
    };
    let fast = numbers(&fast.stdout);
    let slow = numbers(&plain.stdout);
    assert_eq!(fast[0], 100_000_000);
    assert_eq!(fast[0], slow[0]);
    assert!(
        fast[1] * 2 < slow[1],
        "allocations: optimized={}, plain={}",
        fast[1],
        slow[1]
    );
}

#[test]
fn external_functions_and_custom_operators_remain_opaque() {
    equivalent(
        r##"
extern fn retain_value(value:Int)->Int {
 cpp_name:"escape",
 code:"Obj<std::int64_t> escape(Obj<std::int64_t> value,Loc){ *value=99; return value; }",
}
let held:=retain_value(1+2); held+=1; wt held;
struct Weight { value:Int; }
fn __add__(a:Weight,b:Weight)->Weight { a.value+=b.value; return a; }
let a:=Weight{value:1}; let b:=a+Weight{value:2}; wt a.value,b.value;
"##,
        "",
        "100\n3 3\n",
    );
}
