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
fn tuples_preserve_sharing_and_support_copy_destructuring() {
    succeeds(
        r#"
fn pair(x:Int)->(Int,Str) { return (x,"hello"); }
let x:=1; let p:=pair(x);
let (a,b):=p; a=3; wt x,b,p[0];
let (c,d) = p; c=4; d[0]='H'; wt x,b,c,d;
let (first,(second,_)) := (5,(6,7)); wt first,second;
let typed:(Vec<Int>,Int) = ([],8); typed[0].push_back(9); wt typed;
"#,
        "3 hello 3\n3 hello 4 Hello\n5 6\n9 8\n",
    );
}

#[test]
fn tuples_work_in_arrays_comparisons_and_for_patterns() {
    succeeds(
        r#"
let pairs:=[(2,"b"),(1,"z"),(1,"a")]; pairs.sort(); wt pairs;
for (number,word) in pairs { number+=10; wt number,word; }
wt pairs[0] == (11,"a"), pairs[1] > pairs[0];
let detached = pairs; detached[0][0]=99; wt pairs[0],detached[0];
let nested:=((1,2),[3,4]); wt nested,copy(nested)==nested;
"#,
        "1 a 1 z 2 b\n11 a\n11 z\n12 b\nYes Yes\n11 a 99 a\n1 2 3 4 Yes\n",
    );
}

#[test]
fn tuple_diagnostics_are_reported_by_rusk() {
    for source in [
        "let (a,b):=1;",
        "let (a,b):=(1,2,3);",
        "let (a,a):=(1,2);",
        "let (a,b):(Int,Int);",
        "let p:=(1,2); wt p[2];",
        "let p:=(1,2); let i:=0; wt p[i];",
        "let p:=(1,2); wt p[true];",
        "let (a,b):(Int,Str):=(1,2);",
        "let (a,b):=(1,2); wt _;",
        "for (a,b) in [1] {}",
        "for (a,b) in [(1,2)] { let a:=3; }",
        "for a in [1] { let a:=3; }",
        "let x:Int; false && (((x,x):=(1,2))[0]==1); wt x;",
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
fn destructuring_assignment_snapshots_rhs_and_returns_target_sharing() {
    succeeds(
        r#"
let a:=1; let b:=2; let held:=a;
(a,b)=(b,a); wt a,b,held;
(a,b):=(b,a); a=3; wt a,b,held;
let x:Int; let y:Str;
(x,y):=(4,"ok"); wt x,y;
let p:=(5,6); let old:=p[0]; p[0]:=7; old=8; wt p,old;
let array:=[9];
(a,array[0])=(array[0],a); wt a,array;
let result:=((a,b):=(10,20)); result[0]=11; wt a,b,result;
"#,
        "2 1 2\n3 2 2\n4 ok\n7 6 8\n9 3\n11 20 11 20\n",
    );
}
