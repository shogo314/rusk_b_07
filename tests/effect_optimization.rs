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

#[test]
fn closed_integer_and_character_aliases_avoid_allocations() {
    for source in [
        "let total:=0;let i:=0;while i<1000{let value:=i+1;let alias:=value;if alias>0{total+=1;}i+=1;}",
        "let total:=0;let i:=0;while i<1000{let value:='a';let alias:=value;if alias=='a'{total+=1;}i+=1;}",
    ] {
        let (fast, plain) = allocations(source);
        assert!(fast < 30, "optimized allocations: {fast}");
        assert!(plain > 1000, "reference allocations: {plain}");
    }
}

#[test]
fn scalar_aliases_preserve_shared_updates_and_escaping_ownership() {
    equivalent(
        r#"
let a:=12;let alias:=a;alias=19;wt a,alias;
let b:=21;let old:=b;b:=34;wt b,old;
let c:='a';let ca:=c;ca='z';wt c,ca;
let captured:=10;let read:=()=>captured;captured=20;wt read();
let value:=30;let stored:=[value];value=40;wt stored;
fn identity(x:Int)->Int{return x;}
let x:=50;let y:=identity(x);x=60;wt x,y;
fn fresh()->Int{let value:=70;let alias:=value;return alias;}
let result:=fresh();result=80;wt result;
let i:=0;while i<3{let value:=i+100;let alias:=value;wt alias;i+=1;}
"#,
        "19 19\n34 21\nz z\n20\n40\n60 60\n80\n100\n101\n102\n",
    );
}

#[test]
fn vector_access_lowering_keeps_owning_elements_and_evaluation_order() {
    equivalent(
        r#"
let a:=[1,2];let old:=a[0];a[0]:=3;a.push_back(4);old=9;wt old,a;
let b:=[[1],[2]];let saved:=b[0];b=[];saved.push_back(3);wt saved;
let values:=[1,2];let selected:=values[{values=[7,8];0}];wt selected,values;
let rows:=[[1],[2]];let held:Vec<Int>:=[];
for row in rows{held:=row;row[0]+=10;}rows=[];wt held;
let flags:=[true];let flag:=flags[0];flags[0]=false;wt flag;
let chars:=['a'];let ch:=chars[0];chars=[];ch='z';wt ch;
"#,
        "9 3 2 4\n1 3\n7 7 8\n12\nNo\nz\n",
    );
}

#[test]
fn lowered_vector_access_preserves_bounds_and_structural_change_diagnostics() {
    for source in [
        "let a:=[1];wt a[-2];",
        "let a:=[1];wt a[1];",
        "let a:=[1,2];for x in a{a.push_back(3);wt x;}",
        "let a:=[1];let it:=a.__iter__();wt it.__iter_next__();wt it.__iter_next__();",
        "let text:=\"ab\";let chars:=[text[0]];{let c:=chars[0];let alias:=c;chars=[];wt alias;text.push_back('!');}",
    ] {
        let case = Case::new(source);
        let fast = case.run("");
        let plain = case.run_with_options("", &["--no-opt"]);
        assert!(!fast.status.success());
        assert_eq!(fast.status.code(), plain.status.code());
        assert_eq!(fast.stdout, plain.stdout);
        assert_eq!(fast.stderr, plain.stderr);
    }
}

#[test]
fn element_alias_borrowing_preserves_release_points_and_read_snapshots() {
    equivalent(
        r#"
let values:=[1,2,3];let total:=0;
for x in values{let alias:=x;let second:=alias;total+=second;}
wt total;
let flags:=[true,false];for flag in flags{let alias:=flag;wt alias;}
let text:="ab";let chars:=[text[0]];
{let c:=chars[0];let alias:=c;wt alias;}
chars=[];text.push_back('!');wt text;
let a:=[3];let old:=a[0];let alias:=old;a[0]:=7;wt alias,a;
"#,
        "6\nYes\nNo\na\nab!\n3 7\n",
    );
    let case =
        Case::new("let a:=[1,2,3];let total:=0;for x in a{let alias:=x;total+=alias;}wt total;");
    assert!(case.compile().status.success());
    let fast = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    assert!(fast.contains("const Obj<std::int64_t>& rusk_v"), "{fast}");
    assert!(case.compile_with_options(&["--no-opt"]).status.success());
    let plain = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    assert!(!plain.contains("const Obj<std::int64_t>& rusk_v"));
}

#[test]
fn mutable_inline_scalars_keep_alias_updates_and_promote_on_return() {
    equivalent(
        r#"
fn total(n:Int)->Int{
 let value:=0;let alias:=value;
 for i in 0..n{alias+=i;}return value;
}
fn flag()->Bool{let value:=true;let alias:=value;alias=false;return value;}
fn letter()->Char{let value:='a';let alias:=value;alias='z';return alias;}
let result:=total(5);let saved:=result;result=99;wt result,saved,flag(),letter();
fn choose(n:Int)->Int{let value:=1;let alias:=value;if n>0{alias+=2;return value;}alias+=4;return alias;}
wt choose(1),choose(0);
let x:=1;let alias:=x;x:=2;alias=3;wt x,alias;
let captured:=1;let read:=()=>captured;captured+=1;wt read();
let stored:=1;let values:=[stored];stored+=2;wt values;
let numbers:=[1,2];for x in numbers{x+=10;}wt numbers;
"#,
        "99 99 No z\n3 5\n2 3\n2\n3\n11 12\n",
    );
    let case =
        Case::new("fn sum()->Int{let total:=0;for i in 0..10{total+=i;}return total;}wt sum();");
    assert!(case.compile().status.success());
    let cpp = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    assert!(
        cpp.contains("RuskInlineScalar<std::int64_t>& rusk_v"),
        "{cpp}"
    );
    let body = cpp
        .split("// Rusk function ")
        .find(|body| {
            body.lines()
                .next()
                .is_some_and(|line| line.ends_with(": sum"))
        })
        .unwrap();
    let returned = body
        .split("Obj<std::int64_t> rusk_return=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert!(
        cpp.contains(&format!("RuskInlineScalar<std::int64_t>& {returned}=")),
        "{cpp}"
    );
}

#[test]
fn element_slot_borrowing_keeps_shared_updates_and_character_lifetimes() {
    equivalent(
        r#"
let values:=[1,2,3];let total:=0;
for x in values{let alias:=x;total+=alias;}wt total;
let flags:=[true,false];for flag in flags{wt flag;}
let text:="ab";let chars:=[text[0],text[1]];
for ch in chars{let alias:=ch;wt alias;}
chars=[];text.push_back('!');wt text;
let numbers:=[1];let other:=numbers[0];
for x in numbers{other=8;wt x;}wt numbers;
"#,
        "6\nYes\nNo\na\nb\nab!\n8\n8\n",
    );
    let case = Case::new("let a:=[1,2,3];let total:=0;for x in a{total+=x;}wt total;");
    assert!(case.compile().status.success());
    let fast = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    let next = fast
        .split_once("int main() {")
        .unwrap()
        .1
        .lines()
        .find(|line| line.contains("iterator exhausted"))
        .unwrap();
    assert!(
        next.starts_with("const Obj<std::int64_t>& rusk_v"),
        "{next}"
    );
    assert!(next.contains("() -> const auto&"), "{next}");
    let owner = next
        .split('&')
        .nth(1)
        .unwrap()
        .split('=')
        .next()
        .unwrap()
        .trim();
    assert!(!fast.contains(&format!("{owner}.reset()")), "{fast}");
    assert!(case.compile_with_options(&["--no-opt"]).status.success());
    let plain = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    assert!(!plain.contains("() -> const auto&"));
}

#[test]
fn element_slots_keep_ownership_when_source_or_storage_may_change() {
    let foreign_drop = r#"
extern fn deferred(a:Vec<Int>)->Int{
 cpp_name:"deferred",
 code:"Obj<std::int64_t> deferred(Obj<IntVec> a,Loc){return Obj<std::int64_t>(new std::int64_t(0),[a](std::int64_t* p){(*a)[0]=make_obj<std::int64_t>(9);delete p;});}",
}
let a:=[1];{let token:=deferred(a);for x in a{{let alias:=token;}wt x;}}wt a;
"#;
    equivalent(foreign_drop, "1\n9\n");
    equivalent(
        r#"
let value:={let a:=[7];a[0]};wt value;
fn replace(a:Vec<Int>){a[0]:=9;}
let a:=[1];for x in a{replace(a);wt x;}wt a;
let b:=[2];for x in b{b[0]:=8;wt x;}wt b;
let c:=[3];let i:=0;for x in c{i:=1;wt x;}wt i;
"#,
        "7\n1\n9\n2\n8\n3\n1\n",
    );
    for source in [
        "let a:=[1];let x:={let b:=[7];b[0]};wt x;",
        "fn see(x:Int){wt x;}let a:=[1];for x in a{see(3);wt x;}",
        "let a:=[1];for x in a{a[0]:=7;wt x;}",
        "let a:=[1];let i:=0;for x in a{i:=1;wt x;}wt i;",
        foreign_drop,
    ] {
        let case = Case::new(source);
        assert!(case.compile().status.success());
        let cpp = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(!cpp.contains("() -> const auto&"), "{cpp}");
        let access = cpp
            .split_once("int main() {")
            .unwrap()
            .1
            .lines()
            .find(|line| {
                line.contains("iterator exhausted")
                    || (line.contains("=(*") && line.contains("checked_index("))
            })
            .unwrap();
        assert!(access.starts_with("Obj<std::int64_t> "), "{access}");
    }
}

#[test]
fn inferred_borrowed_calls_keep_one_element_owner_and_borrow_its_aliases() {
    equivalent(
        r#"
fn next(x:Int)->Int{return x+1;}
fn annotated(readonly noescape x:Int)->Int{return x+1;}
let a:=[1,2,3];let total:=0;
for x in a{let alias:=x;total+=next(alias)+annotated(x);}wt total;
let callback:=(x:Int)=>x+2;for x in a{wt callback(x);}
fn rebound(x:Int)->Int{let old:=x;x:=10;return old+x;}
for x in a{wt rebound(x);}
fn code(c:Char)->Bool{return c=='a';}
let text:="ab";let chars:=[text[0],text[1]];
for ch in chars{wt code(ch);}chars=[];text.push_back('!');wt text;
let local:=3;let alias:=local;wt next(local);alias=8;wt local;
"#,
        "18\n3\n4\n5\n11\n12\n13\nYes\nNo\nab!\n4\n8\n",
    );
    for parameter in ["x:Int", "readonly noescape x:Int", "mutable escape x:Int"] {
        let case = Case::new(&format!(
            "fn next({parameter})->Int{{return x+1;}}let a:=[1,2];for x in a{{wt next(x);}}"
        ));
        assert!(case.compile().status.success());
        let fast = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(signature(&fast, "next").contains("const Obj<std::int64_t>& rusk_p0"));
        let main = fast.split_once("int main() {").unwrap().1;
        let getter = main
            .lines()
            .find(|line| line.contains("iterator exhausted"))
            .unwrap();
        assert!(getter.starts_with("Obj<std::int64_t> "), "{getter}");
        let owner = getter
            .split_whitespace()
            .nth(1)
            .unwrap()
            .split('=')
            .next()
            .unwrap();
        let aliases = main
            .lines()
            .filter(|line| {
                line.starts_with("const Obj<std::int64_t>& ")
                    && line.ends_with(&format!("={owner};"))
            })
            .count();
        assert!(aliases >= 2, "{main}");
        assert_eq!(
            main.matches(&format!("{owner}.reset();")).count(),
            1,
            "{main}"
        );
        assert!(!main.contains("() -> const auto&"));
        assert!(case.compile_with_options(&["--no-opt"]).status.success());
        let plain = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(!plain.contains("const Obj<std::int64_t>& rusk_v"));
    }
}

#[test]
fn element_call_borrowing_rejects_mutation_escape_and_opaque_calls() {
    equivalent(
        r#"
fn update(x:Int){x+=1;}
fn identity(x:Int)->Int{return x;}
fn after(x:Int,other:Int)->Int{other=9;return x+0;}
let a:=[1];for x in a{update(x);wt x;}wt a;
let b:=[2];for x in b{let saved:=identity(x);saved=7;wt x;}wt b;
let c:=[3];for x in c{wt after(x,x);}wt c;
fn stored(x:Int,values:Vec<Int>){values.push_back(x);}
let held:Vec<Int>:=[];let d:=[4];for x in d{stored(x,held);}d=[];held[0]=8;wt held;
"#,
        "2\n2\n7\n7\n9\n9\n8\n",
    );
    for source in [
        "fn use_value(x:Int){x+=1;}let a:=[1];for x in a{use_value(x);}",
        "fn use_value(x:Int)->Int{return x;}let a:=[1];for x in a{wt use_value(x);}",
        r#"extern fn use_value(x:Int)->Int{
cpp_name:"use_value",code:"Obj<std::int64_t> use_value(Obj<std::int64_t> x,Loc){return make_obj<std::int64_t>(*x+1);}",
}let a:=[1];for x in a{wt use_value(x);}"#,
        r#"extern fn foreign(){cpp_name:"foreign",code:"void foreign(Loc){}",}
fn use_value(x:Int)->Int{foreign();return x+1;}
let a:=[1];for x in a{wt use_value(x);}"#,
    ] {
        let case = Case::new(source);
        let compiled = case.compile();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let cpp = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        let main = cpp.split_once("int main() {").unwrap().1;
        assert!(!main.contains("const Obj<std::int64_t>& rusk_v"), "{main}");
    }
}

#[test]
fn borrowed_parameter_aliases_survive_branches_loops_recursion_and_caller_rebinding() {
    equivalent(
        r#"
fn across(x:Int,n:Int)->Int{
 let alias:=x;let total:=0;
 for i in 0..n{let local:=alias;if i%2==0{total+=local;}else{total+=x+1;}}
 if n<0{return x+0;}return total;
}
fn relay(x:Int)->Int{return across(x,2);}
fn recur(x:Int)->Int{if x==0{return 0;}return x+recur(x-1);}
wt across(3,3),relay(5),recur(5);
let value:=4;wt across(value,{value:=9;2}),value;
fn yes(flag:Bool)->Bool{let alias:=flag;if alias{return true;}return false;}
wt yes(true),yes(false);
fn code(c:Char)->Bool{let alias:=c;return alias=='a';}
let text:="ab";wt code(text[0]);text.push_back('!');wt text;
"#,
        "10 11 15\n9 9\nYes No\nYes\nab!\n",
    );
    for parameter in ["x:Int", "readonly noescape x:Int", "mutable escape x:Int"] {
        let case = Case::new(&format!(
            "fn across({parameter})->Int{{let alias:=x;let total:=0;for i in 0..3{{let local:=alias;if i>0{{total+=local;}}else{{total+=x;}}}}return total;}}wt across(2);"
        ));
        assert!(case.compile().status.success());
        let fast = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        let body = fast
            .split("// Rusk function ")
            .find(|body| {
                body.lines()
                    .next()
                    .is_some_and(|line| line.ends_with(": across"))
            })
            .unwrap();
        let aliases: Vec<_> = body
            .lines()
            .filter(|line| {
                line.contains("const Obj<std::int64_t>& ") && line.ends_with("=rusk_p0;")
            })
            .collect();
        assert!(aliases.len() >= 3, "{body}");
        for alias in aliases {
            let id = alias
                .split('&')
                .nth(1)
                .unwrap()
                .split('=')
                .next()
                .unwrap()
                .trim();
            assert!(!body.contains(&format!("{id}.reset()")), "{body}");
        }
        assert!(!body.contains("rusk_p0.reset()"));
        assert!(case.compile_with_options(&["--no-opt"]).status.success());
        let plain = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(!plain.contains("const Obj<std::int64_t>& rusk_v"));
    }
}

#[test]
fn borrowed_parameter_aliases_keep_ownership_for_rebinding_storage_and_capture() {
    equivalent(
        r#"
struct Holder{value:Int;}
fn rebound(x:Int)->Int{let saved:=x;x:=7;return saved+x;}
fn split(x:Int)->Int{let alias:=x;alias:=8;return x+alias;}
fn stored(x:Int)->Int{let holder:=Holder{value:x};return holder.value+1;}
fn captured(x:Int)->Int{let getter:=()=>x+1;return getter();}
let value:=2;wt rebound(value),split(value),stored(value),captured(value),value;
fn change(x:Int){x+=1;}change(value);wt value;
fn identity(x:Int)->Int{return x;}let held:=identity(value);value=8;wt held;
"#,
        "9 10 3 3 2\n3\n8\n",
    );
    for source in [
        "fn inspect(x:Int)->Int{let saved:=x;x:=7;return saved+x;}wt inspect(2);",
        "fn inspect(x:Int)->Int{let alias:=x;alias:=8;return x+alias;}wt inspect(2);",
        "struct Holder{value:Int;}fn inspect(x:Int)->Int{let holder:=Holder{value:x};return holder.value+1;}wt inspect(2);",
        "fn inspect(x:Int)->Int{let getter:=()=>x+1;return getter();}wt inspect(2);",
    ] {
        let case = Case::new(source);
        assert!(case.compile().status.success());
        let cpp = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        let body = cpp
            .split("// Rusk function ")
            .find(|body| {
                body.lines()
                    .next()
                    .is_some_and(|line| line.ends_with(": inspect"))
            })
            .unwrap();
        assert!(
            !body
                .lines()
                .any(|line| line.contains("const Obj<std::int64_t>& ")
                    && line.ends_with("=rusk_p0;")),
            "{body}"
        );
        assert!(
            body.lines()
                .any(|line| line.starts_with("Obj<std::int64_t> ") && line.ends_with("=rusk_p0;")),
            "{body}"
        );
    }
}

#[test]
fn element_aliases_cross_branches_loops_and_preserve_replaced_elements() {
    equivalent(
        r#"
let values:=[1,2,3];let total:=0;
for x in values{let alias:=x;if x%2==0{total+=alias;}else{total+=x+1;}}
wt total,find(values,2)??-1,find(values,9)??-1;
let a:=[4];let old:=a[0];let alias:=old;
if true{a[0]:=9;}else{a=[];}wt old,alias,a;
let b:=[5];let held:=b[0];let result:=0;
for i in 0..3{let alias:=held;if i==1{continue;}result+=alias;}
b=[];wt held,result;
let flags:=[true,false];for flag in flags{let alias:=flag;if alias{wt flag;}else{wt alias;}}
fn take(text:Str,flag:Bool){let chars:=[text[0]];let c:=chars[0];let alias:=c;if flag{wt alias;return;}wt c;}
let text:="ab";take(text,true);text.push_back('!');take(text,false);text.push_back('?');wt text;
"#,
        "8 1 -1\n4 4 9\n5 10\nYes\nNo\na\na\nab!?\n",
    );
    let case = Case::new(
        "let a:=[1,2];let total:=0;for x in a{let alias:=x;if x>0{total+=alias;}else{total-=x;}}wt total;",
    );
    assert!(case.compile().status.success());
    let cpp = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    let main = cpp.split_once("int main() {").unwrap().1;
    let getter = main
        .lines()
        .find(|line| line.contains("iterator exhausted"))
        .unwrap();
    assert!(!getter.contains("() -> const auto&"), "{getter}");
    let owner = getter.split('=').next().unwrap().trim();
    assert!(owner.starts_with("rusk_v"), "{getter}");
    assert!(
        main.contains(&format!("Obj<std::int64_t> {owner};")),
        "{main}"
    );
    assert!(
        main.lines()
            .any(|line| line.contains("const Obj<std::int64_t>& ")
                && line.ends_with(&format!("={owner};"))),
        "{main}"
    );
    assert!(main.contains(&format!("{owner}.reset();")), "{main}");
    assert!(case.compile_with_options(&["--no-opt"]).status.success());
    let plain = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
    assert!(!plain.contains("const Obj<std::int64_t>& rusk_v"));
}

#[test]
fn element_alias_flow_preserves_character_release_at_joins_and_early_exits() {
    equivalent(
        r#"
fn run(flag:Bool){
 let text:="ab";let chars:=[text[0]];
 {let c:=chars[0];let alias:=c;if flag{chars=[];}else{chars[0]:='z';chars=[];}wt alias;}
 text.push_back('!');wt text;
}
run(true);run(false);
let a:=[1,2,3];let saved:=0;
for x in a{if x>1{saved:=x;}if x==2{continue;}wt x;}a=[];wt saved;
"#,
        "a\nab!\na\nab!\n1\n3\n3\n",
    );
    for source in [
        "let text:=\"ab\";let chars:=[text[0]];let c:=chars[0];let alias:=c;if true{chars=[];}else{chars[0]:='z';}wt alias;text.push_back('!');",
        "let a:=[1,2];for x in a{if x>0{a.push_back(3);}wt x;}",
    ] {
        let case = Case::new(source);
        let fast = case.run("");
        let plain = case.run_with_options("", &["--no-opt"]);
        assert!(!fast.status.success());
        assert_eq!(fast.status.code(), plain.status.code());
        assert_eq!(fast.stdout, plain.stdout);
        assert_eq!(fast.stderr, plain.stderr);
    }
}
