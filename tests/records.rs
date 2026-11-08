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
fn fields_share_update_and_rebind() {
    succeeds(
        r#"
struct Point { x: Int; y: Int; }
fn Point(x:Int,y:Int)->Point { return Point { y:y, x:x }; }
let x:=1; let point:=Point(x,2); let held:=point.x;
point.x=3; wt x,held,point.x;
point.x:=9; held=4; wt x,held,point.x;
let detached = point; detached.x=10; wt point.x,detached.x;
let alias:=point; let old:=point.x;
point=Point(20,30); wt alias.x,alias.y,old;
point=point; point.x=21; wt point.x,old;
"#,
        "3 3 3\n4 4 9\n9 10\n20 30 9\n21 9\n",
    );
}

#[test]
fn record_vectors_and_nested_copies_are_independent() {
    succeeds(
        r#"
struct Wrapper { points: Vec<Point>; }
struct Point { x: Int; }
let x:=1; let point:=Point { x:x };
let points:=[point,point]; let nested:=Wrapper { points:points };
let detached = nested; detached.points[0].x=7;
wt point.x,detached.points[0].x,detached.points[1].x;
let repeated:=[nested;2]; repeated[0].points[0].x=8;
wt repeated[0].points[0].x,repeated[0].points[1].x,repeated[1].points[0].x;
let saved:Point;
{ let temporary:=Point { x:9 }; saved:=temporary; }
let empty:Vec<Point>:=[]; empty.push_back(saved);
for p in empty { p.x+=1; }
wt empty[0].x;
let copied = empty; empty=[]; copied[0].x=11; wt saved.x,copied[0].x;
"#,
        "1 7 1\n8 1 1\n10\n10 11\n",
    );
}

#[test]
fn field_receivers_are_evaluated_once_before_rhs() {
    succeeds(
        r#"
struct Cell { value: Int; }
fn counted(count:Int,cell:Cell)->Cell { count+=1; return cell; }
let count:=0; let a:=Cell { value:1 }; let old:=a;
counted(count,a).value := (a:=Cell { value:2 }).value;
wt count,old.value,a.value;
old.value = (old.value := 3);
wt old.value;
let state:=0;
struct Pair { first:Int; second:Int; }
let pair:=Pair { second:(state+=1), first:copy(state) };
state=9; wt pair.first,pair.second,state;
"#,
        "1 2 2\n3\n1 9 9\n",
    );
}

#[test]
fn invalid_records_are_diagnosed_before_cpp() {
    for source in [
        "struct X { value:Int; value:Int; }",
        "struct X { value:Missing; }",
        "struct X { value:Int; } struct X { other:Int; }",
        "struct X { value:Int; } let x:=X { wrong:1 };",
        "struct X { a:Int; b:Int; } let x:=X { a:1 };",
        "struct X { value:Int; } let x:=X { value:true };",
        "struct X { value:Int; } let x:=X { value:1, value:2 };",
        "struct X { value:Int; } let x:=X { value:1 }; wt x.missing;",
        "struct X { value:Int; } let x:=X { value:1 }; x.value:=true;",
        "let x:=1; wt x.value;",
        "struct X { value:Int; } let x:=X();",
        "struct X { next:X; }",
        "struct X { children:Vec<X>; }",
        "struct X { other:Y; } struct Y { other:X; }",
        "fn f() { struct X { value:Int; } }",
        "struct X { value:Int; } let x:=X { value:1 }; wt x==x;",
        "struct X { value:Int; } let x:=[X { value:1 }]; x.sort();",
    ] {
        let case = Case::new(source);
        let out = case.compile();
        assert!(!out.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("main.rusk:"),
            "{source}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!case.0.join("main.cpp").exists(), "{source}");
    }
}

#[test]
fn field_copying_dispatches_by_rusk_type_even_with_shared_cpp_types() {
    succeeds(
        r#"
extern symbol observed_copies { code: "int copied_fields=0;", }
extern type Score { cpp_name: "std::int64_t", }
extern fn score()->Score {
    cpp_name: "make_score",
    code: "Obj<std::int64_t> make_score(Loc) { return make_obj<std::int64_t>(7); }",
}
extern fn copy(value:Score)->Score {
    cpp_name: "copy_score",
    code: "Obj<std::int64_t> copy_score(Obj<std::int64_t> value, Loc) { ++copied_fields; return make_obj<std::int64_t>(*value); }",
}
extern fn copy_count()->Int {
    cpp_name: "count_score_copies",
    code: "Obj<std::int64_t> count_score_copies(Loc) { return make_obj<std::int64_t>(copied_fields); }",
}
struct Holder { value:Score; }
let original:=Holder { value:score() };
let detached = original;
detached=original;
let repeated:=[original;2];
let array:=[original]; let copied = array;
wt copy_count();
"#,
        "5\n",
    );
}
