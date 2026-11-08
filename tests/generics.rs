mod support;
use support::Case;

fn succeeds(source: &str, expected: &str) {
    let out = Case::new(source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn generic_functions_infer_types_and_prefer_concrete_overloads() {
    succeeds(
        r#"
fn identity<T>(value:T)->T { return value; }
fn twice<T>(value:T)->T { return value+value; }
fn twice(value:Int)->Int { return value+100; }
fn zero<T>()->T { return T(); }
fn count<T>(values:Vec<T>)->Int { return values.len(); }
fn repeat_add<T>(value:T,n:Int)->T {
    if n==0 { return value; }
    return repeat_add(value+value,n-1);
}
let x:=3; let shared:=identity(x); shared=4;
wt x,twice(2),twice("ab"),twice::<Int>(2),zero::<Int>(),count::<Int>([]);
wt repeat_add(3,2),repeat_add("a",2);
"#,
        "4 102 abab 4 0 0\n12 aaaa\n",
    );
}

#[test]
fn generic_structs_and_functions_support_nested_types() {
    succeeds(
        r#"
struct Pair<A,B> { first:A; second:B; }
fn Pair<A,B>(a:A,b:B)->Pair<A,B> { return Pair::<A,B> { first:a,second:b }; }
fn first<A,B>(pair:Pair<A,B>)->A { return pair.first; }
fn second<A,B>(pair:(A,B))->B { return pair[1]; }
let pair:=Pair(1,"hello"); pair.first()=2; wt pair.first,pair.second;
let detached = pair; detached.first=3; wt pair.first,detached.first;
let nested:=Pair(Pair(4,5),[6]); wt nested.first.first,nested.first.second,nested.second;
let values:Vec<Pair<Int,Str>>:=[]; values.push_back(pair);
let copied = values; copied[0].first=9; wt pair.first,copied[0].first;
wt second((1,"ok"));
"#,
        "2 hello\n2 3\n4 5 6\n2 9\nok\n",
    );
}

#[test]
fn generic_specialization_failures_have_source_context() {
    for source in [
        "fn f<T,T>(value:T) {}",
        "fn f<T>(a:T,a:T) {}",
        "fn f<T>(a:T) {} fn f<U>(b:U) {}",
        "struct Pair<T,T> { value:T; }",
        "struct Box<T> { value:T; } let x:Box<Int,Str>;",
        "struct Box<T> { value:T; } let x:Vec<Box<Missing>>;",
        "fn f<T>(a:T,b:T) {} f(1,true);",
        "fn f<T>(a:T) {} f::<Missing>(1);",
        "fn f<T>(a:T) {} f::<Int,Str>(1);",
        "fn zero<T>()->T { return T(); } zero();",
        "fn f<T>(a:Vec<T>) {} fn f<T>(a:T) {} f([1]);",
        "struct Node<T> { children:Vec<Node<T>>; } let n:Node<Int>;",
        "struct Box<T> { value:T; } struct Box { value:Int; }",
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
    let case = Case::new("fn bad<T>(value:T)->Int { return value.missing; }\nwt bad(1);");
    let out = case.compile();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        error.contains("main.rusk:1:")
            && error.contains("while instantiating")
            && error.contains("main.rusk:2:"),
        "{error}"
    );
}
