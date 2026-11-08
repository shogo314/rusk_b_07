mod support;
use support::Case;
fn succeeds(source: &str, expected: &str) {
    let output = Case::new(source).run("");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());
}
fn rejects(source: &str) {
    let output = Case::new(source).compile();
    assert!(!output.status.success(), "{source}");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains("readonly access") && message.contains("readonly modifier is here"),
        "{message}"
    );
}
#[test]
fn stores_do_not_restrict_independent_objects_or_other_fields() {
    succeeds(
        r#"
struct Pair{left:Int;right:Int;}
let pair:=Pair{left:0,right:0};let alias:=pair;
let other:=Pair{left:0,right:0};
readonly let source:=1;
pair.left:=source;
alias.right=2;other.left=3;other.right:=4;
wt alias.left,alias.right,other.left,other.right;
alias.left:=5;wt alias.left;
"#,
        "1 2 3 4\n5\n",
    );
}
#[test]
fn private_stores_do_not_contaminate_the_callers_heap() {
    succeeds(
        r#"
struct Box{value:Int;}
fn private(){readonly let value:=1;let box:=Box{value:0};box.value:=value;}
private();let other:=Box{value:0};other.value=7;wt other.value;
"#,
        "7\n",
    );
}
#[test]
fn aliases_parents_and_previously_extracted_children_see_later_stores() {
    for source in [
        "struct Box{value:Int;}let b:=Box{value:0};let alias:=b;readonly let r:=1;b.value:=r;alias.value=2;",
        "struct Box{value:Int;}struct Outer{box:Box;}let b:=Box{value:0};let outer:=Outer{box:b};readonly let r:=1;b.value:=r;outer.box.value=2;",
        "struct Box{value:Int;}struct Outer{box:Box;}let outer:=Outer{box:Box{value:0}};let child:=outer.box;readonly let r:=1;outer.box.value:=r;child.value=2;",
        "struct Box{value:Int;}let b:=Box{value:0};let f:=()=>{b.value=2;};readonly let r:=1;b.value:=r;f();",
    ] {
        rejects(source);
    }
}
#[test]
fn returned_root_projection_and_nested_aggregate_aliases_keep_their_identities() {
    for helper in [
        "fn wrap(b:Box)->Box{return b;}",
        "fn wrap(b:Box)->(Box,Int){return (b,0);}",
        "fn wrap(b:Box)->((Box,Int),Int){return ((b,0),0);}",
    ] {
        let access = if helper.contains("((Box") {
            "result[0][0]"
        } else if helper.contains("(Box,") {
            "result[0]"
        } else {
            "result"
        };
        rejects(&format!(
            "struct Box{{value:Int;}}{helper}let b:=Box{{value:0}};let result:=wrap(b);readonly let r:=1;b.value:=r;{access}.value=2;"
        ));
    }
    rejects(
        "struct Box{value:Int;}struct Outer{box:Box;}fn child(o:Outer)->Box{return o.box;}let b:=Box{value:0};let c:=child(Outer{box:b});readonly let r:=1;b.value:=r;c.value=2;",
    );
}
#[test]
fn escaping_to_unknown_code_keeps_later_stores_conservative() {
    rejects(
        r#"
struct Box{value:Int;}
fn save(b:Box,values:Vec<Box>){values.push_back(b);}
let b:=Box{value:0};let values:Vec<Box>:=[];save(b,values);
readonly let r:=1;b.value:=r;
values[0].value=2;
"#,
    );
    rejects(
        r#"
struct Box{value:Int;}
extern fn opaque(b:Box)->Box{cpp_name:"opaque",code:"Obj<RuskRecord0> opaque(Obj<RuskRecord0> b,Loc){return b;}",}
let b:=Box{value:0};let alias:=opaque(b);
readonly let r:=1;b.value:=r;alias.value=2;
"#,
    );
}
#[test]
fn branch_and_loop_alias_sets_do_not_assume_a_single_allocation_instance() {
    rejects(
        "struct Box{value:Int;}let a:=Box{value:0};let b:=Box{value:0};let selected:=a;if true{selected:=b;}readonly let r:=1;selected.value:=r;b.value=2;",
    );
    rejects(
        "struct Box{value:Int;}let saved:=Box{value:0};readonly let r:=1;for i in 0..2{let fresh:=Box{value:0};if i==0{saved:=fresh;}else{fresh.value:=r;saved.value=2;}}",
    );
    rejects(
        "struct Box{value:Int;}let saved:=Box{value:0};readonly let r:=1;for i in 0..2{let fresh:=Box{value:0};if i==0{fresh.value:=r;saved:=fresh;}else{saved.value=2;}}",
    );
    succeeds(
        "struct Box{value:Int;}readonly let r:=1;for i in 0..2{let a:=Box{value:0};let b:=Box{value:0};a.value:=r;b.value=i;wt b.value;}",
        "0\n1\n",
    );
}
#[test]
fn root_qualifiers_still_restrict_members_without_freezing_other_access_paths() {
    succeeds(
        "struct Box{value:Int;}let b:=Box{value:0};readonly let view:=b;b.value=3;wt view.value;",
        "3\n",
    );
    rejects("struct Box{value:Int;}let b:=Box{value:0};readonly let view:=b;view.value=3;");
}
