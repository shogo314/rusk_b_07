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
fn rejects(source: &str, expected: &str) {
    let output = Case::new(source).compile();
    assert!(!output.status.success(), "{source}");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains(expected)
            && message.contains("noescape modifier is here")
            && message.contains("parameter declared here"),
        "{message}"
    );
}
#[test]
fn noescape_is_independent_of_mutation_and_permissive_escape_does_not_force_escape() {
    succeeds(
        r#"
fn twice(noescape readonly fixed value:Int)->Int{return value*2;}
fn bump(mutable noescape value:Int){value+=1;}
fn append<T>(noescape values:Vec<T>,escape value:T){values.push_back(value);}
fn fresh(escape value:Int)->Int{return value+1;}
let x:=2;bump(x);wt x,twice(x),fresh(x);
let values:Vec<Int>:=[];append(values,x);x=5;wt values;
let f:=(fixed noescape readonly value:Int)=>value+2;wt f(x);
"#,
        "3 6 4\n5\n7\n",
    );
}
#[test]
fn copies_rebinding_local_aggregates_and_discarded_alias_returns_stay_scoped() {
    succeeds(
        r#"
fn alias(value:Int)->Int{return value;}
fn detached(noescape value:Int)->Int{return copy(alias(value));}
fn ignore(noescape value:Int){alias(value);}
fn replaced(noescape value:Int)->Int{value:=8;return value;}
fn local(noescape value:Int)->Int{let pair:=(value,0);return pair[0]+1;}
fn first(values:Vec<Int>)->Int{return values[0];}
fn copied_first(readonly noescape values:Vec<Int>)->Int{return copy(first(values));}
fn ignored_first(noescape values:Vec<Int>){first(values);}
fn cloned(readonly noescape values:Vec<Int>)->Vec<Int>{return copy(values);}
fn option_copy(readonly noescape value:Option<Int>)->Int{if value.is_some(){return copy(value?!);}return 0;}
let x:=1;ignore(x);wt detached(x),replaced(x),local(x),x;
let values:=[3,4];ignored_first(values);wt copied_first(values);wt cloned(values),option_copy(Some(7));
"#,
        "1 8 2 1\n3\n3 4 7\n",
    );
}
#[test]
fn returned_roots_members_payloads_and_aggregate_contents_escape() {
    for source in [
        "fn f(noescape x:Int)->Int{return x;}wt f(1);",
        "fn f(noescape x:Int)->Int{let alias:=x;return alias;}wt f(1);",
        "fn f(noescape x:Vec<Int>)->Int{return x[0];}wt f([1]);",
        "struct Box{value:Int;}fn f(noescape x:Box)->Int{return x.value;}wt f(Box{value:1});",
        "fn f(noescape x:Option<Int>)->Int{return x?!;}wt f(Some(1));",
        "fn f(noescape x:Int)->(Int,Int){return (0,x);}wt f(1);",
        "fn f(noescape x:Int)->Option<Int>{return Some(x);}f(1);",
        "fn f(noescape x:Int)->Vec<Int>{return [x];}wt f(1);",
        "fn f(noescape x:Int)->Int{let old:=x;x:=2;return old;}wt f(1);",
    ] {
        rejects(source, "noescape parameter 'x' may escape");
    }
}
#[test]
fn external_storage_and_unknown_external_effects_are_diagnosed_at_the_call() {
    rejects(
        "fn save(noescape x:Int,values:Vec<Int>){values.push_back(x);}let a:Vec<Int>:=[];save(1,a);",
        "noescape parameter 'x' may escape",
    );
    rejects(
        "struct Box{value:Int;}fn save(noescape x:Int,b:Box){b.value:=x;}save(1,Box{value:0});",
        "cannot prove noescape for parameter 'x'",
    );
    rejects(
        r#"extern fn foreign(x:Int){cpp_name:"foreign",code:"void foreign(Obj<std::int64_t>,Loc){}",}fn f(noescape x:Int){foreign(x);}f(1);"#,
        "cannot prove noescape for parameter 'x'",
    );
}
#[test]
fn callback_results_and_captures_are_checked_even_without_saving_the_callback() {
    succeeds(
        r#"
fn invoke<F>(noescape callback:F)->Int{return callback();}
fn discard<F>(noescape callback:F){callback();}
let x:=3;wt invoke(()=>x+1);discard(()=>x);
let wrapped:=(noescape value:Int)=>{let callback:=()=>value;callback()+1};wt wrapped(x);
"#,
        "4\n4\n",
    );
    rejects(
        "fn invoke<F>(noescape callback:F)->Int{return callback();}let x:=1;wt invoke(()=>x);",
        "noescape parameter 'callback' may escape",
    );
    rejects(
        "let factory:=(noescape value:Int)=>()=>value;let callback:=factory(1);wt callback();",
        "noescape parameter 'value' may escape",
    );
    rejects(
        "fn save<F>(noescape callback:F,values:Vec<F>){values.push_back(callback);}let f:=()=>1;let values:= [f];save(f,values);",
        "noescape parameter 'callback' may escape",
    );
}
#[test]
fn branches_loops_recursion_and_forwarding_use_inferred_callee_effects() {
    succeeds(
        r#"
fn recurse(noescape x:Int,n:Int)->Int{if n==0{return x+1;}return recurse(x,n-1);}
fn odd(noescape x:Int,n:Int)->Int{if n==0{return x+1;}return even(x,n-1);}
fn even(noescape x:Int,n:Int)->Int{if n==0{return x+2;}return odd(x,n-1);}
fn repeated(noescape x:Int){for i in 0..3{wt x+i;}}
wt recurse(1,2),odd(1,2);repeated(1);
"#,
        "2 2\n1\n2\n3\n",
    );
    for source in [
        "fn alias(x:Int)->Int{return x;}fn f(noescape x:Int)->Int{return alias(x);}wt f(1);",
        "fn f(noescape x:Int,flag:Bool)->Int{if flag{return x;}return 2;}wt f(1,true);",
        "fn f(noescape x:Int,n:Int)->Int{if n==0{return x;}return f(x,n-1);}wt f(1,2);",
        "fn f(noescape x:Int)->Int{let a:=2;for i in 0..2{a:=x;}return a;}wt f(1);",
    ] {
        rejects(source, "noescape parameter 'x' may escape");
    }
}
#[test]
fn parameter_only_syntax_conflicts_and_overloads_are_rejected() {
    for (source, expected) in [
        (
            "noescape let x:=1;",
            "escape modifiers are only supported on parameters",
        ),
        (
            "escape let x:=1;",
            "escape modifiers are only supported on parameters",
        ),
        (
            "noescape fixed let (x,y):=(1,2);",
            "escape modifiers are only supported on parameters",
        ),
        (
            "fn f(noescape noescape x:Int){}",
            "duplicate escape modifier",
        ),
        (
            "let f:=(escape escape x:Int)=>x;",
            "duplicate escape modifier",
        ),
        (
            "fn f(noescape escape x:Int){}",
            "conflicting escape modifiers",
        ),
        (
            "let f:=(escape noescape x:Int)=>x;",
            "conflicting escape modifiers",
        ),
        (
            r#"extern fn f(noescape x:Int){cpp_name:"f",code:"ignored",}"#,
            "escape modifiers on extern parameters require a Rusk function body",
        ),
        (
            r#"extern fn f(escape x:Int){cpp_name:"f",code:"ignored",}"#,
            "escape modifiers on extern parameters require a Rusk function body",
        ),
        (
            "fn f(noescape x:Int){}fn f(escape x:Int){}",
            "duplicate function",
        ),
    ] {
        let output = Case::new(source).compile();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn diagnostics_point_to_returns_and_retaining_calls_before_ir_or_cpp_output() {
    for (source, line, detail) in [
        (
            "fn bad(noescape value:Int)->Int {\nlet alias:=value;\nreturn alias;\n}\nwt bad(1);",
            3,
            "return value",
        ),
        (
            "fn bad(noescape value:Int,values:Vec<Int>) {\nlet alias:=value;\nvalues.push_back(alias);\n}\nlet a:Vec<Int>:=[];bad(1,a);",
            3,
            "argument 2 of 'push_back'",
        ),
        (
            "struct Box{value:Int;}fn bad(noescape value:Int,values:Vec<Int>) {\nlet box:=Box{value:0};\nbox.value:=value;\nvalues.push_back(value);\n}\nlet a:Vec<Int>:=[];bad(1,a);",
            4,
            "argument 2 of 'push_back'",
        ),
        (
            "struct Box{value:Int;}fn identity(b:Box)->Box{return b;}fn bad(noescape value:Int,values:Vec<Int>) {\nlet box:=identity(Box{value:0});\nbox.value:=value;\nvalues.push_back(value);\n}\nlet a:Vec<Int>:=[];bad(1,a);",
            4,
            "argument 2 of 'push_back'",
        ),
        (
            "struct Box{value:Int;}fn packed(b:Box)->(Box,Int){return (b,0);}fn bad(noescape value:Int,values:Vec<Int>) {\nlet box:=Box{value:0};\nlet held:=packed(box);\nbox.value:=value;\nvalues.push_back(value);\n}\nlet a:Vec<Int>:=[];bad(1,a);",
            5,
            "argument 2 of 'push_back'",
        ),
    ] {
        for stage in ["hir", "lir", "cpp"] {
            let case = Case::new(source);
            let output = case.compile_with_options(&["--emit", stage]);
            assert!(!output.status.success());
            let message = String::from_utf8_lossy(&output.stderr);
            assert!(
                message.contains(&format!("main.rusk:{line}:"))
                    && message.contains("main.rusk:1:")
                    && message.contains(detail)
                    && message.contains("through parameter 1"),
                "{message}"
            );
            assert!(!case.0.join("main.cpp").exists());
        }
    }
    let case = Case::new("fn f(noescape x:Int,escape y:Int)->Int{return x+y;}wt f(1,2);");
    for stage in ["ast", "hir", "lir"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        assert!(dump.contains("Noescape") && dump.contains("Escape"));
        if stage != "ast" {
            assert!(dump.contains("AnnotateEscape"));
            let function = &dump[dump.find("name: \"f\",").unwrap()..];
            let effects = &function[function.find("effects: Summary").unwrap()..];
            for parameter in effects.split("ParameterEffects {").skip(1).take(2) {
                assert!(parameter.split('}').next().unwrap().contains("escape: No"));
            }
        }
    }
}

#[test]
fn private_field_stores_stay_scoped_through_aliases_branches_and_loop_rebinding() {
    succeeds(
        r#"
struct Box{value:Int;}
fn local(noescape x:Int)->Int{
 let b:=Box{value:0};let alias:=b;alias.value:=x;
 alias.value+=1;return b.value+1;
}
fn tuple(noescape x:Int)->Int{let pair:=(0,1);pair[0]:=x;return pair[0]+pair[1];}
fn choose(noescape x:Int,flag:Bool)->Int{
 let a:=Box{value:0};let b:=Box{value:1};let chosen:=a;
 if flag{chosen:=b;}chosen.value:=x;return chosen.value+2;
}
fn repeated(noescape x:Int)->Int{
 let b:=Box{value:0};let total:=0;
 for i in 0..3{let alias:=b;b:=Box{value:0};alias.value:=x;total+=alias.value;}
 return total;
}
fn old_snapshot(noescape x:Int)->Int{
 let b:=Box{value:7};let old:=b.value;b.value:=x;return old;
}
fn generic<T>(noescape x:T){let box:=(x,x);box[0]:=x;}
let x:=3;wt local(x),x,tuple(x),choose(x,true),choose(x,false),repeated(x),old_snapshot(x);
generic([1,2]);
"#,
        "5 4 5 6 6 12 7\n",
    );
}

#[test]
fn published_destinations_remain_conservative_before_and_after_the_store() {
    let cases = [
        "fn f(noescape x:Int)->Box{let b:=Box{value:0};b.value:=x;return b;}f(1);",
        "fn f(noescape x:Int)->Box{let b:=Box{value:0};let alias:=b;alias.value:=x;return b;}f(1);",
        "fn f(noescape x:Int,items:Vec<Box>){let b:=Box{value:0};items.push_back(b);b.value:=x;}let items:Vec<Box>:=[];f(1,items);",
        "fn f(noescape x:Int,items:Vec<Box>){let b:=Box{value:0};b.value:=x;items.push_back(b);}let items:Vec<Box>:=[];f(1,items);",
        "fn f(noescape x:Int,outer:Box){let b:=Box{value:0};b:=outer;b.value:=x;}f(1,Box{value:0});",
        "fn f(noescape x:Int)->Option<Box>{let b:=Box{value:0};let outer:=Some(b);b.value:=x;return outer;}f(1);",
        "fn f(noescape x:Int,flag:Bool)->Option<Box>{let b:=Box{value:0};let result:Option<Box>:=None;if flag{result:=Some(b);}b.value:=x;return result;}f(1,true);",
        "fn f(noescape x:Int,items:Vec<Box>){let b:=Box{value:0};for i in 0..2{b.value:=x;items.push_back(b);b:=Box{value:0};}}let items:Vec<Box>:=[];f(1,items);",
    ];
    for source in cases {
        rejects(
            &format!("struct Box{{value:Int;}}{source}"),
            "noescape for parameter 'x'",
        );
    }
}

#[test]
fn private_store_does_not_hide_escape_through_members_callbacks_or_unknown_calls() {
    for tail in [
        "return b.value;",
        "let value:=b.value;return value;",
        "let read:=()=>b.value;return read();",
    ] {
        rejects(
            &format!(
                "struct Box{{value:Int;}}fn f(noescape x:Int)->Int{{let b:=Box{{value:0}};b.value:=x;{tail}}}wt f(1);"
            ),
            "cannot prove noescape for parameter 'x'",
        );
    }
    rejects(
        r#"struct Box{value:Int;}
extern fn foreign(b:Box){cpp_name:"foreign",code:"void foreign(auto,Loc){}",}
fn f(noescape x:Int){let b:=Box{value:0};b.value:=x;foreign(b);}f(1);
"#,
        "cannot prove noescape for parameter 'x'",
    );
}

#[test]
fn private_stores_are_inferred_without_annotations_and_preserved_in_ir() {
    let case = Case::new(
        "struct Box{value:Int;}\nfn plain(x:Int)->Int{let b:=Box{value:0};b.value:=x;return b.value+1;}\nfn explicit(escape x:Int)->Int{let b:=Box{value:0};b.value:=x;return b.value+1;}\nfn nested(x:Int)->Int{let b:=(Box{value:0},0);b[0].value:=x;return b[0].value+1;}\nfn identity(b:Box)->Box{return b;}\nfn via_call(x:Int)->Int{let b:=Box{value:0};let alias:=identity(b);alias.value:=x;return b.value+1;}\nfn packed(b:Box)->(Box,Int){return (b,0); }\nfn via_result(x:Int)->Int{let b:=Box{value:0};let h:=packed(b);b.value:=x;return h[0].value+1;}\nwt plain(1),explicit(2),nested(3),via_call(4),via_result(5);",
    );
    for stage in ["hir", "lir"] {
        let output = case.compile_with_options(&["--emit", stage]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dump = std::fs::read_to_string(case.0.join("main.cpp")).unwrap();
        for name in ["plain", "explicit", "nested", "via_call", "via_result"] {
            let function = &dump[dump.find(&format!("name: \"{name}\",")).unwrap()..];
            let effects = &function[function.find("effects: Summary").unwrap()..];
            let parameter = effects
                .split("ParameterEffects {")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            assert!(
                parameter.contains("escape: No") && parameter.contains("retained: No"),
                "{parameter}"
            );
        }
    }
}

#[test]
fn nested_private_destinations_follow_fields_payloads_and_weak_stores() {
    succeeds(
        r#"
struct Cell{value:Int;}
struct Holder{cell:Cell;count:Int;}
enum Wrapped{Full(Cell),Empty}
fn nested(noescape x:Int)->Int{
 let child:=Cell{value:0};let outer:=Holder{cell:child,count:1};
 let alias:=outer.cell;alias.value:=x;return outer.cell.value+outer.count;
}
fn stored(noescape x:Int)->Int{
 let outer:=Holder{cell:Cell{value:0},count:0};let child:=Cell{value:1};
 outer.cell:=child;child.value:=x;return outer.cell.value+1;
}
fn payload(noescape x:Int)->Int{
 let wrapped:=Wrapped::Full(Cell{value:0});
 let child:=match wrapped{Full(c)=>c,Empty=>Cell{value:0},};
 child.value:=x;return child.value+2;
}
fn generic<T>(noescape x:T){let child:=(x,0);let parent:=(child,0);parent[0][0]:=x;}
fn snapshot(noescape x:Int)->Int{
 let outer:=(Cell{value:0},0);let old:=outer[0];outer[0]:=Cell{value:1};
 old.value:=x;return old.value+outer[0].value;
}
fn repeated(noescape x:Int)->Int{
 let outer:=(Cell{value:0},0);let total:=0;
 for i in 0..3{let old:=outer[0];outer[0]:=Cell{value:1};old.value:=x;total+=old.value;}
 return total;
}
let x:=3;wt nested(x),stored(x),payload(x),snapshot(x),repeated(x);generic([1,2]);
"#,
        "4 4 5 4 9\n",
    );
}

#[test]
fn publishing_one_child_does_not_publish_its_private_parent_or_sibling() {
    succeeds(
        r#"
struct Cell{value:Int;}
struct Pair{left:Cell;right:Cell;}
fn read(c:Cell)->Int{return c.value+1;}
fn scoped(noescape x:Int)->Int{
 let pair:=Pair{left:Cell{value:0},right:Cell{value:7}};
 let published:Vec<Cell>:=[];published.push_back(pair.right);
 wt read(pair.right);pair.left.value:=x;return pair.left.value+1;
}
wt scoped(3);
"#,
        "8\n4\n",
    );
}

#[test]
fn nested_publications_reach_children_before_and_after_stores() {
    for body in [
        "fn f(noescape x:Int)->Holder{let child:=Cell{value:0};let outer:=Holder{cell:child};child.value:=x;return outer;}f(1);",
        "fn f(noescape x:Int)->Holder{let child:=Cell{value:0};child.value:=x;let outer:=Holder{cell:child};return outer;}f(1);",
        "fn f(noescape x:Int,outer:Holder){let child:=Cell{value:0};outer.cell:=child;child.value:=x;}f(1,Holder{cell:Cell{value:0}});",
        "fn f(noescape x:Int,outer:Holder){let child:=Cell{value:0};child.value:=x;outer.cell:=child;}f(1,Holder{cell:Cell{value:0}});",
        "fn f(noescape x:Int,external:Cell){let outer:=Holder{cell:Cell{value:0}};outer.cell:=external;outer.cell.value:=x;}f(1,Cell{value:0});",
        "fn replace(outer:Holder,c:Cell){outer.cell:=c;}fn f(noescape x:Int,external:Cell){let outer:=Holder{cell:Cell{value:0}};replace(outer,external);outer.cell.value:=x;}f(1,Cell{value:0});",
        "fn f(noescape x:Int,items:Vec<Holder>,flag:Bool){let child:=Cell{value:0};let outer:=Holder{cell:child};if flag{items.push_back(outer);}child.value:=x;}let items:Vec<Holder>:=[];f(1,items,true);",
        "fn f(noescape x:Int,items:Vec<Holder>){let outer:=Holder{cell:Cell{value:0}};for i in 0..2{let old:=outer.cell;outer.cell:=Cell{value:0};old.value:=x;items.push_back(outer);}}let items:Vec<Holder>:=[];f(1,items);",
        "let f:=(noescape x:Int)=>{let child:=Cell{value:0};let outer:=Holder{cell:child};child.value:=x;()=>outer.cell.value};let callback:=f(1);wt callback();",
        "enum Wrapped{Full(Cell),Empty}fn f(noescape x:Int)->Wrapped{let child:=Cell{value:0};let outer:=Wrapped::Full(child);child.value:=x;return outer;}f(1);",
    ] {
        rejects(
            &format!("struct Cell{{value:Int;}}struct Holder{{cell:Cell;}}{body}"),
            "cannot prove noescape for parameter 'x'",
        );
    }
}

#[test]
fn nonretaining_calls_forward_private_roots_members_choices_recursion_and_captures() {
    succeeds(
        r#"
struct Cell{value:Int;}
struct Holder{cell:Cell;}
fn identity(c:Cell)->Cell{return c;}
fn getter(h:Holder)->Cell{return h.cell;}
fn forwarded(h:Holder)->Cell{return identity(getter(h));}
fn read(c:Cell)->Int{return c.value+1;}
fn recursive(c:Cell,n:Int)->Cell{if n==0{return c;}return recursive(c,n-1);}
fn odd(c:Cell,n:Int)->Cell{if n==0{return c;}return even(c,n-1);}
fn even(c:Cell,n:Int)->Cell{if n==0{return c;}return odd(c,n-1);}
fn choose(a:Cell,b:Cell,flag:Bool)->Cell{if flag{return a;}return b;}
fn direct(noescape x:Int)->Int{
 let c:=Cell{value:0};let alias:=identity(c);alias.value:=x;return c.value+1;
}
fn nested(noescape x:Int)->Int{
 let h:=Holder{cell:Cell{value:0}};let alias:=forwarded(h);alias.value:=x;return h.cell.value+2;
}
fn reading(noescape x:Int)->Int{let c:=Cell{value:0};c.value:=x;return read(c);}
fn recursing(noescape x:Int)->Int{
 let c:=odd(recursive(Cell{value:0},3),2);c.value:=x;return c.value+1;
}
fn choosing(noescape x:Int,flag:Bool)->Int{
 let a:=Cell{value:0};let b:=Cell{value:1};let selected:=choose(a,b,flag);
 selected.value:=x;return a.value+b.value;
}
fn captured(noescape x:Int)->Int{
 let h:=Holder{cell:Cell{value:0}};let get:=()=>h.cell;let c:=get();
 c.value:=x;return h.cell.value+3;
}
wt direct(3),nested(3),reading(3),recursing(3),choosing(3,true),choosing(3,false),captured(3);
"#,
        "4 5 4 4 4 3 6\n",
    );
}

#[test]
fn forwarding_does_not_hide_publication_mutation_or_mixed_external_origins() {
    for body in [
        "fn f(noescape x:Int)->Cell{let c:=identity(Cell{value:0});c.value:=x;return c;}f(1);",
        "fn f(noescape x:Int)->Holder{let h:=Holder{cell:Cell{value:0}};let c:=getter(h);c.value:=x;return h;}f(1);",
        "fn f(noescape x:Int,external:Cell,flag:Bool){let c:=choose(Cell{value:0},external,flag);c.value:=x;}f(1,Cell{value:0},true);",
        "fn save(c:Cell,items:Vec<Cell>)->Cell{items.push_back(c);return c;}fn f(noescape x:Int,items:Vec<Cell>){let c:=save(Cell{value:0},items);c.value:=x;}let items:Vec<Cell>:=[];f(1,items);",
        "fn replace(h:Holder,c:Cell)->Cell{h.cell:=c;return h.cell;}fn f(noescape x:Int,external:Cell){let h:=Holder{cell:Cell{value:0}};let c:=replace(h,external);c.value:=x;}f(1,Cell{value:0});",
        "fn recursive(c:Cell,items:Vec<Cell>,n:Int)->Cell{if n==0{items.push_back(c);return c;}return recursive(c,items,n-1);}fn f(noescape x:Int,items:Vec<Cell>){let c:=recursive(Cell{value:0},items,3);c.value:=x;}let items:Vec<Cell>:=[];f(1,items);",
        "fn f(noescape x:Int,items:Vec<Cell>){let h:=Holder{cell:Cell{value:0}};let c:=getter(h);items.push_back(c);h.cell.value:=x;}let items:Vec<Cell>:=[];f(1,items);",
        "fn f(noescape x:Int,items:Vec<Cell>){let h:=Holder{cell:Cell{value:0}};h.cell.value:=x;items.push_back(getter(h));}let items:Vec<Cell>:=[];f(1,items);",
        "fn create()->Cell{return Cell{value:0};}fn f(noescape x:Int){let c:=create();c.value:=x;}f(1);",
        "fn maybe(c:Cell,flag:Bool)->Cell{if flag{return c;}return Cell{value:0};}fn f(noescape x:Int){let c:=maybe(Cell{value:0},true);c.value:=x;}f(1);",
        "fn wrap(c:Cell)->Holder{return Holder{cell:c};}fn f(noescape x:Int){let c:=Cell{value:0};let h:=wrap(c);h.cell.value:=x;}f(1);",
        r#"extern fn foreign(c:Cell)->Cell{cpp_name:"foreign",code:"auto foreign(auto c,Loc){return c;}",}fn f(noescape x:Int){let c:=foreign(Cell{value:0});c.value:=x;}f(1);"#,
    ] {
        rejects(
            &format!(
                "struct Cell{{value:Int;}}struct Holder{{cell:Cell;}}fn identity(c:Cell)->Cell{{return c;}}fn getter(h:Holder)->Cell{{return h.cell;}}fn choose(a:Cell,b:Cell,flag:Bool)->Cell{{if flag{{return a;}}return b;}}{body}"
            ),
            "cannot prove noescape for parameter 'x'",
        );
    }
}

#[test]
fn returned_aggregates_keep_original_private_aliases_without_publishing_them() {
    succeeds(
        r#"
struct Cell{value:Int;}
struct Holder{cell:Cell;}
enum Wrapped{Full(Cell),Empty}
fn wrap(c:Cell)->Holder{return Holder{cell:c};}
fn twice(c:Cell)->(Holder,Int){return (wrap(c),0);}
fn payload(c:Cell)->Wrapped{return Wrapped::Full(c);}
fn identity(h:Holder)->Holder{return h;}
fn maybe(c:Cell,flag:Bool)->Holder{if flag{return wrap(c);}return wrap(Cell{value:5});}
fn direct(noescape x:Int)->Int{
 let c:=Cell{value:0};let h:=identity(wrap(c));c.value:=x;return h.cell.value+1;
}
fn nested(noescape x:Int)->Int{
 let c:=Cell{value:0};let h:=twice(c);c.value:=x;return h[0].cell.value+2;
}
fn variant(noescape x:Int)->Int{
 let c:=Cell{value:0};let h:=payload(c);c.value:=x;
 return match h{Full(value)=>value.value+3,Empty=>0,};
}
fn option(noescape x:Int)->Int{
 let c:=Cell{value:0};let h:=Some(c);c.value:=x;return h?!.value+4;
}
fn branching(noescape x:Int,flag:Bool)->Int{
 let c:=Cell{value:0};let h:=maybe(c,flag);c.value:=x;return h.cell.value+1;
}
fn repeated(noescape x:Int)->Int{
 let total:=0;for i in 0..3{let c:=Cell{value:0};let h:=wrap(c);c.value:=x;total+=h.cell.value;}
 return total;
}
wt direct(3),nested(3),variant(3),option(3),branching(3,true),branching(3,false),repeated(3);
"#,
        "4 5 6 7 4 6 9\n",
    );
}

#[test]
fn publishing_returned_containers_reaches_original_children_and_earlier_instances() {
    for body in [
        "fn f(noescape x:Int)->Holder{let c:=Cell{value:0};let h:=wrap(c);c.value:=x;return h;}f(1);",
        "fn f(noescape x:Int,items:Vec<Holder>){let c:=Cell{value:0};let h:=wrap(c);items.push_back(h);c.value:=x;}let items:Vec<Holder>:=[];f(1,items);",
        "fn f(noescape x:Int,items:Vec<Holder>){let c:=Cell{value:0};let h:=wrap(c);c.value:=x;items.push_back(h);}let items:Vec<Holder>:=[];f(1,items);",
        "fn f(noescape x:Int,flag:Bool,items:Vec<Holder>){let c:=Cell{value:0};let h:=wrap(c);if flag{items.push_back(h);}c.value:=x;}let items:Vec<Holder>:=[];f(1,true,items);",
        "fn f(noescape x:Int,items:Vec<Cell>){let c:=Cell{value:0};let h:=wrap(c);items.push_back(h.cell);c.value:=x;}let items:Vec<Cell>:=[];f(1,items);",
        "fn f(noescape x:Int,items:Vec<Holder>){for i in 0..2{let c:=Cell{value:0};let h:=wrap(c);c.value:=x;items.push_back(h);}}let items:Vec<Holder>:=[];f(1,items);",
        "fn twice(c:Cell)->(Holder,Int){return (wrap(c),0);}fn f(noescape x:Int)->(Holder,Int){let c:=Cell{value:0};let h:=twice(c);c.value:=x;return h;}f(1);",
        "enum Wrapped{Full(Cell),Empty}fn payload(c:Cell)->Wrapped{return Wrapped::Full(c);}fn f(noescape x:Int)->Wrapped{let c:=Cell{value:0};let h:=payload(c);c.value:=x;return h;}f(1);",
        "fn f(noescape x:Int)->Option<Cell>{let c:=Cell{value:0};let h:=Some(c);c.value:=x;return h;}f(1);",
        "let f:=(noescape x:Int)=>{let c:=Cell{value:0};let h:=wrap(c);c.value:=x;()=>h.cell.value};let callback:=f(1);wt callback();",
        "fn f(noescape x:Int,external:Holder){let c:=Cell{value:0};let h:=wrap(c);c.value:=x;external.cell:=h.cell;}f(1,Holder{cell:Cell{value:0}});",
    ] {
        rejects(
            &format!(
                "struct Cell{{value:Int;}}struct Holder{{cell:Cell;}}fn wrap(c:Cell)->Holder{{return Holder{{cell:c}};}}{body}"
            ),
            "cannot prove noescape for parameter 'x'",
        );
    }
}

#[test]
fn return_path_candidates_do_not_prove_unique_wrappers_or_definite_children() {
    for body in [
        "fn f(noescape x:Int){let h:=wrap(Cell{value:0});h.cell.value:=x;}f(1);",
        "fn f(noescape x:Int,external:Cell){let h:=wrap(external);h.cell.value:=x;}f(1,Cell{value:0});",
        "fn maybe(c:Cell,flag:Bool)->Holder{if flag{return wrap(c);}return wrap(Cell{value:0});}fn f(noescape x:Int){let h:=maybe(Cell{value:0},true);let c:=h.cell;c.value:=x;}f(1);",
        "fn f(noescape x:Int){let h:=wrap(Cell{value:0});h.cell:=Cell{value:x};}f(1);",
    ] {
        rejects(
            &format!(
                "struct Cell{{value:Int;}}struct Holder{{cell:Cell;}}fn wrap(c:Cell)->Holder{{return Holder{{cell:c}};}}{body}"
            ),
            "cannot prove noescape for parameter 'x'",
        );
    }
}
