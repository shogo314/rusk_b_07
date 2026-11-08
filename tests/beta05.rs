mod support;
use support::Case;

fn output(source: &str, input: &str, expected: &str) {
    let out = Case::new(source).run(input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn branches_and_coalescing_are_lazy_and_preserve_sharing() {
    output(
        r#"
let count:=0;let a:=[1];let b:=[2];
let picked:=if (count+=1)==1 {a} else {b};picked[0]=7;
let none:Option<Vec<Int>>:=None;let some:=Some(a);
let fallback:=()=>[count+=10];
let held:=some??fallback();held[0]=8;
let absent:=none??fallback();
wt a,b,count,absent;
let chosen:=if false {fallback()} else {a};chosen[0]=9;
wt a,count;
let nested:=None::<Option<Int>>()??Some(4);wt nested?!;
let n:Option<Int>:=None;wt n??5,n.is_none(),!some;
"#,
        "",
        "8 2 11 11\n9 11\n4\n5 Yes No\n",
    );
}

#[test]
fn comparison_chains_evaluate_once_in_order_and_stop_early() {
    output(
        r#"
let log:Vec<Int>:=[];
let mark:=(n:Int)=>log.push_back(n);
fn record(log:Vec<Int>,n:Int)->Int{log.push_back(n);return n;}
wt record(log,1)<record(log,2)<=record(log,2)!=record(log,3)==record(log,3);
wt log;log=[];
wt record(log,5)<record(log,2)<record(log,9);wt log;
let shared:=2;
fn change(x:Int)->Int{x=0;return 1;}
wt 1<shared<change(shared),shared;
"#,
        "",
        "Yes\n1 2 2 3 3\nNo\n5 2\nYes 0\n",
    );
}

#[test]
fn option_updates_keep_outer_identity_and_old_payload_alive() {
    output(
        r#"
let row:=[1,2];let o:=Some(row);let alias:=o;let held:=o?!;
let detached=o;row[0]=7;wt detached.value(),o.value();
o=None;wt alias.is_none(),held;
o=Some([9]);wt alias.value(),held;
let source:=Some([4]);o=source;source.value()[0]=8;wt o.value(),source.value();
let nested:=Some(Some([3]));let copied=nested;copied.value().value()[0]=6;
wt nested.value().value(),copied.value().value();
"#,
        "",
        "1 2 7 2\nYes 7 2\n9 7 2\n4 8\n3 6\n",
    );
}

#[test]
fn dictionary_values_share_rebind_and_survive_deletion() {
    output(
        r#"
let d:=Dict::<Str,Vec<Int>>();let row:=[1];d.set("a",row);
let held:=d.get("a")?!;row[0]=2;wt held;
d["a"]:=[3];wt held,d["a"];
let current:=d["a"];d["a"]=[4];wt current;
let detached=d;d.erase("a");current[0]=5;
wt held,current,detached["a"],d.len(),d.get("a").is_none();
d["new"].push_back(7);wt d["new"],d.len();
let options:=Dict::<Str,Option<Int>>();options["a"]:=None;wt options.get("a")?!.is_none();
"#,
        "",
        "2\n2 3\n4\n2 5 4 0 Yes\n7 1\nYes\n",
    );
}

#[test]
fn keys_are_protected_on_insertion_export_and_iteration() {
    output(
        r#"
let key:=(1,("a",true));let d:=Dict::<(Int,(Str,Bool)),Int>();d.set(key,2);
key[1][0]="changed";wt d.contains((1,("a",true))),d.contains(key);
let keys:=d.keys();keys[0][0]=9;
for (k,v) in d {k[1][0]="other";v=7;d.set((1,("a",true)),v);}
wt d.get((1,("a",true)))??0;
let s:=Set::<(Int,(Str,Bool))>();let k:=(2,("b",false));s.insert(k);k[0]=8;
let exported:=s.get((2,("b",false)))?!;exported[0]=9;
for v in s {v[0]=10;s.insert((2,("b",false)));}
wt s.contains((2,("b",false))),s.len();
let chars:=Set::<Char>();chars.insert('a');wt chars.contains('a');
"#,
        "",
        "Yes No\n7\nYes 1\nYes\n",
    );
}

#[test]
fn dictionary_and_set_match_reference_after_many_operations() {
    let source = r#"let d:=Dict::<Int,Int>();let s:=Set::<Int>();
in n:Int;for i in 0..n{in op,key,value:Int;
if op==0{d.set(key,value);s.insert(key);}
else{if op==1{d.erase(key);s.erase(key);}else{wt d.get(key)??(-1),s.contains(key),d.len(),s.len();}}
}
let total:=0;for (k,v) in d{total+=k+v;}wt total;
let sum:=0;for k in s{sum+=k;}wt sum;"#;
    let mut input = String::from("240\n");
    let mut map = std::collections::BTreeMap::new();
    let mut set = std::collections::BTreeSet::new();
    let mut seed = 314159u64;
    let mut expected = String::new();
    for _ in 0..240 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let key = ((seed >> 32) % 31) as i64;
        let value = (seed % 100) as i64;
        input += &format!("{} {key} {value}\n", seed % 4);
        match seed % 4 {
            0 => {
                map.insert(key, value);
                set.insert(key);
            }
            1 => {
                map.remove(&key);
                set.remove(&key);
            }
            _ => {
                expected += &format!(
                    "{} {} {} {}\n",
                    map.get(&key).unwrap_or(&-1),
                    if set.contains(&key) { "Yes" } else { "No" },
                    map.len(),
                    set.len()
                );
            }
        }
    }
    expected += &format!(
        "{}\n{}\n",
        map.iter().map(|(k, v)| k + v).sum::<i64>(),
        set.iter().sum::<i64>()
    );
    output(source, &input, &expected);
}

#[test]
fn collection_iterators_compose_with_lazy_operations_and_copy() {
    output(
        r#"
let d:=Dict::<Int,Int>();for i in 0..5 {d.set(i,i*10);}
let it:=d.map((p:(Int,Int))=>p[0]+p[1]);let snapshot=it;
wt it.fold(0,(a:Int,b:Int)=>a+b),snapshot.fold(0,(a:Int,b:Int)=>a+b);
let s:=Set::<Int>();for i in 0..5{s.insert(i);}
wt s.filter((x:Int)=>x%2==0).fold(0,(a:Int,b:Int)=>a+b);
let detached=d;d.clear();wt detached.len(),d.len();
"#,
        "",
        "110 110\n6\n5 0\n",
    );
}

#[test]
fn structured_input_and_eof_read_preserve_token_order() {
    output(
        r#"
in a(2,2):Vec<Vec<Int>>,p:(Int,(Str,Char)),rows(2):Vec<(Int,Str)>;
wt a,p,rows;
let total:=0;while true{let value:=try_read::<Int>();if value.is_none(){break;}total+=value?!;}
wt total,try_read::<Str>().is_none();
"#,
        "1 2 3 4 5 hi z 6 x 7 y 10 20 \n\t",
        "1 2 3 4 5 hi z 6 x 7 y\n30 Yes\n",
    );
    output(
        "in empty(0,3):Vec<Vec<Int>>;wt empty.len();wt try_read::<Char>().is_none();",
        "",
        "0\nYes\n",
    );
}

#[test]
fn find_reports_first_index_or_absence() {
    output(
        "let a:=[3,1,3];wt a.find(3)??(-1),a.find(9)??(-1);let empty:Vec<Int>:=[];wt empty.find(1).is_none();",
        "",
        "0 -1\nYes\n",
    );
}

#[test]
fn invalid_types_and_uninitialized_paths_report_compile_errors() {
    for source in [
        "let x:=if true{1}else{false};",
        "wt 1??2;",
        "wt 1?!;",
        "wt Some(1)??false;",
        "let x:=None;",
        "let s:=Set::<Vec<Int>>();",
        "struct Key{x:Int;}let s:=Set::<Key>();",
        "struct Recursive{next:Option<Recursive>;}let x:Recursive;",
        "let d:=Dict::<Int,Int>();d[true]:=2;",
        "let d:=Dict::<Int,Int>();d[1]:=false;",
        "in a(2):Vec<Option<Int>>;",
        "in a(2):Vec<Vec<Int>>;",
        "in a(2,3):Vec<Int>;",
        "let x:Int;let y:=if true{x:=1}else{2};wt x;",
        "let x:Int;wt 3<2<(x:=4);wt x;",
        "let x:=try_read::<Vec<Int>>();",
        "let x:Int;let o:=Some(1);let y:=o??(x:=2);wt x;",
        "fn Some(x:Int)->Int{return x;}wt Some(1);",
        "struct Item{x:Int;}fn __lt__(a:Item,b:Item)->Int{return 1;}wt Item{x:1}<Item{x:2}<Item{x:3};",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "unexpected success: {source}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("main.rusk:"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn absent_values_and_structural_changes_fail_at_source_location() {
    for (source, message) in [
        ("let x:Option<Int>:=None;wt x?!;", "absent Option value"),
        (
            "let d:=Dict::<Int,Int>();d.set(1,2);for p in d{d.set(2,3);}",
            "container changed during iteration",
        ),
        (
            "let s:=Set::<Int>();s.insert(1);for x in s{s.erase(x);}",
            "container changed during iteration",
        ),
        (
            "let s:=Set::<Int>();s.insert(1);for x in s{s=Set::<Int>();}",
            "container changed during iteration",
        ),
        (
            "let d:=Dict::<Int,Int>();d.set(1,2);let it:=d.map((p:(Int,Int))=>p[1]);d.clear();wt it.to_vec();",
            "container changed during iteration",
        ),
        ("wt try_read::<Int>()??0;", "invalid"),
        ("in a(-1,2):Vec<Vec<Int>>;", "negative array length"),
        ("in a(0,-1):Vec<Vec<Int>>;", "negative array length"),
        (
            "let s:=Set::<Int>();s.insert(1);wt s.map((x:Int)=>s.insert(2)).to_vec();",
            "container changed during iteration",
        ),
    ] {
        let out = Case::new(source).run("bad");
        assert!(!out.status.success(), "unexpected success: {source}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            error.contains(message) && error.contains("main.rusk:"),
            "{source}\n{error}"
        );
    }
}

#[test]
fn contextual_constructors_and_values_without_default_constructors() {
    output(
        r#"
struct Item{x:Int;}
let d:=Dict::<Int,Item>();d.set(1,Item{x:7});wt d.get(1)?!.x,d.get(2).is_none();
let o:Option<Vec<Int>>:=Some([]);wt o?!.len();
let explicit:=Some::<Vec<Int>>([]);wt explicit?!.len();
let options:=Dict::<Str,Option<Int>>();options.set("a",None);wt options.get("a")?!.is_none();
let x:Int;let y:=if true{x:=1}else{x:=2};wt x,y;
fn unwrap_or<T>(o:Option<T>,fallback:T)->T{return o??fallback;}
wt unwrap_or(Some(3),8),unwrap_or(None::<Int>(),8);
"#,
        "",
        "7 Yes\n0\n0\nYes\n1 1\n3 8\n",
    );
    let out = Case::new("struct Item{x:Int;}let d:=Dict::<Int,Item>();wt d[1].x;").compile();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("zero-argument constructor"));
}

#[test]
fn documented_example_runs_and_unwrap_reports_operator_position() {
    output(
        include_str!("../examples/beta05/features.rusk"),
        "",
        "2 0\n1 Yes\n9 2\nYes 9 2\nYes\nsmall\n0\n",
    );
    let out = Case::new("let o:Option<Int>:=None;\nwt o?!;").run("");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:2:5: absent Option value"));
}

#[test]
fn nested_containers_and_option_callbacks_copy_recursively() {
    output(
        r#"
struct Box<T>{value:T;}
let d:=Dict::<Str,Option<Vec<Int>>>();d.set("a",Some([1]));
let all:=[Box::<Dict<Str,Option<Vec<Int>>>>{value:d}];let detached=all;
all[0].value.get("a")?!.value()[0]=5;
wt all[0].value.get("a")?!.value(),detached[0].value.get("a")?!.value();
let state:=0;let callback:=()=>state+=1;let o:=Some(callback);let copied=o;
wt o?!(),copied?!(),state;
"#,
        "",
        "5 1\n1 1 1\n",
    );
}

#[test]
fn rusk_library_find_supports_nested_values_and_custom_equality() {
    output(
        r#"
struct Item { key: Int; calls: Int; }
fn __eq__(a: Item, b: Item) -> Bool { a.calls += 1; return a.key == b.key; }
let calls := 0;
let a := [Item { key: 3, calls: calls }, Item { key: 7, calls: calls }, Item { key: 7, calls: calls }];
wt find(a, Item { key: 7, calls: 0 }) ?? -1, calls;
wt find::<Int>([4, 5], 5) ?? -1;
let nested: Vec<Vec<Int>> := [[], [1]];
wt nested.find([]) ?? -1;
wt ["a", "b"].find("b") ?? -1, ['a', 'b'].find('x').is_none();
"#,
        "",
        "1 2\n1\n0\n1 Yes\n",
    );
}

#[test]
fn rusk_library_algorithms_return_independent_values() {
    output(
        r#"
let a := 3; let b := 7;
let low := min(a, b); let high := max(a, b); let positive := abs(a);
low = 100; high = 200; positive = 300;
let initial := 10; let total := [a, b].sum(initial); total = 0;
wt a, b, initial, total;
wt sum([]), sum([], 9), min(a, a), max(b, b), abs(-4), abs(0);
"#,
        "",
        "3 7 10 0\n0 9 3 7 4 0\n",
    );
}

#[test]
fn rusk_library_failures_keep_the_user_call_location() {
    for source in [
        "wt 0;\nwt sum([9223372036854775807,1]);",
        "wt 0;\nwt abs(-9223372036854775808);",
    ] {
        let out = Case::new(source).run("");
        assert!(!out.status.success());
        let message = String::from_utf8_lossy(&out.stderr);
        assert!(message.contains("main.rusk:2:4:"), "{message}");
        assert!(message.contains("integer overflow"), "{message}");
    }
}

#[test]
fn find_checks_structural_changes_even_when_comparison_matches() {
    for matches in ["true", "false"] {
        let source = format!(
            r##"
extern symbol mutation {{ code: "#include <functional>\nstd::function<void()> rusk_test_mutate;", }}
struct Item {{ key: Int; }}
extern fn install(a: Vec<Item>) {{
    cpp_name: "rusk_test_install",
    code: "void rusk_test_install(auto a, Loc) {{ rusk_test_mutate = [a] {{ a->clear(); ++a->version; }}; }}",
}}
extern fn mutate() {{
    cpp_name: "rusk_test_do_mutate",
    code: "void rusk_test_do_mutate(Loc) {{ auto f = std::move(rusk_test_mutate); f(); }}",
}}
fn __eq__(a: Item, b: Item) -> Bool {{ mutate(); return {matches}; }}
let a := [Item {{ key: 1 }}]; install(a);
wt a.find(Item {{ key: 1 }}) ?? -1;
"##
        );
        let out = Case::new(&source).run("");
        assert!(!out.status.success());
        let message = String::from_utf8_lossy(&out.stderr);
        assert!(message.contains("container changed"), "{message}");
        assert!(!message.contains("<stdlib/"), "{message}");
    }
}
