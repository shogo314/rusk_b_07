mod support;
use support::Case;
fn output(source: &str, expected: &str) {
    let out = Case::new(source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}
#[test]
fn precedence_associativity_comparison_chains_and_boundaries() {
    output(
        r#"
wt 10<?3+4,1+2<?4*2,8<?4>?6,8>?4<?6;
wt 1<?2==1,0<=8<?3+1<5,1|8<?4;
wt Some(8)??9<?4,None::<Int>()??9<?4,1+(8<?4)*2;
wt -9223372036854775808<?9223372036854775807,-9223372036854775808>?9223372036854775807;
let a:=10;let b:=2;let c:=5;a<?=b>?=c;wt a,b,c;
wt min(8,4),max(8,4);
"#,
        "7 3 6 6\nYes Yes 5\n8 4 9\n-9223372036854775808 9223372036854775807\n5 5 5\n4 8\n",
    );
}
#[test]
fn update_preserves_outer_identity_copies_rhs_and_returns_target() {
    output(
        r#"
let a:=10;let alias:=a;let result:=(a<?=3);result=4;wt a,alias;
let source:=2;a<?=source;source=1;wt a,source;
a>?=8;wt alias;
let n:=7;n+=5;n-=2;n*=3;n/=4;n%=4;wt n;
"#,
        "4 4\n2 1\n8\n3\n",
    );
}
#[test]
fn operands_and_compound_places_are_evaluated_once_in_order() {
    output(
        r#"
let log:Vec<Int>:=[];let rows:=[[8]];
fn row(rows:Vec<Vec<Int>>,log:Vec<Int>)->Vec<Int>{log.push_back(1);return rows[0];}
fn index(log:Vec<Int>)->Int{log.push_back(2);return 0;}
fn rhs(log:Vec<Int>,target:Int)->Int{log.push_back(3);target=9;return 4;}
row(rows,log)[index(log)]<?=rhs(log,rows[0][0]);wt rows,log;
log=[];let held:=rows[0][0];
fn replace(rows:Vec<Vec<Int>>,log:Vec<Int>)->Int{log.push_back(3);rows[0]:=[99];return 1;}
row(rows,log)[index(log)]<?=replace(rows,log);wt held,rows,log;
fn mark(log:Vec<Int>,n:Int)->Int{log.push_back(n);return n;}
log=[];wt mark(log,1)<?mark(log,2),mark(log,3)>?mark(log,4);wt log;
let a:=5;fn change(a:Int)->Int{a=0;return 1;}wt a<?change(a),a;
"#,
        "4 1 2 3\n1 99 1 2 3\n1 4\n1 2 3 4\n0 0\n",
    );
}
#[test]
fn ordered_strings_characters_tuples_and_vectors_return_independent_copies() {
    output(
        r#"
let a:="z";let b:="a";let chosen:=a<?b;chosen.push_back('!');wt a,b,chosen;
let s:="abc";let c:=s[0]<?'z';s.push_back('!');c='Z';wt s,c;
s[0]<?='A';s.push_back('?');wt s;
let x:=(1,"b");let y:=(1,"a");let p:=x<?y;p[1]="x";wt x,y,p;
let v:=[2,3];let w:=[1,4];let selected:=v>?w;selected[0]=9;wt v,w,selected;
"#,
        "z a a!\nabc! Z\nAbc!?\n1 b 1 a 1 x\n2 3 1 4 9 3\n",
    );
}
#[test]
fn unchanged_and_equal_candidates_preserve_payload_aliases() {
    output(
        r#"
struct Item{rank:Int;data:Vec<Int>;}
fn __lt__(a:Item,b:Item)->Bool{return a.rank<b.rank;}
let item:=Item{rank:1,data:[10]};let outer:=item;let held:=item.data;
let candidate:=Item{rank:2,data:[20]};item<?=candidate;held[0]=11;wt item.data;
item>?=candidate;candidate.data[0]=99;wt outer.rank,item.data,held;
let tie:=Item{rank:2,data:[30]};let independent:=item<?tie;
wt independent.data,(item>?tie).data;
let current:=item.data;item<?=tie;current[0]=21;independent.data[0]=77;
wt item.data,tie.data,independent.data;
"#,
        "11\n2 20 11\n20 20\n21 30 77\n",
    );
}
#[test]
fn operator_and_update_hooks_can_be_overloaded_independently() {
    output(
        r#"
struct Token{key:Int;}
fn __min__(a:Token,b:Token)->Token{return Token{key:a.key+b.key};}
fn __max__(a:Token,b:Token)->Token{return Token{key:a.key*b.key};}
fn __imin__(a:Token,b:Token){a.key+=b.key;}
fn __imax__(a:Token,b:Token){a.key*=b.key;}
let a:=Token{key:2};let b:=Token{key:3};wt (a<?b).key,(a>?b).key;
a<?=b;a>?=b;wt a.key,b.key;
"#,
        "5 6\n15 3\n",
    );
}
#[test]
fn invalid_operands_targets_and_comparison_hooks_are_source_errors() {
    for (source, message) in [
        ("wt 1<?\"a\";", "no matching function"),
        ("let x:Int;x<?=1;", "before initialization"),
        ("let a:=1;let b:=2;(a,b)<?=(2,3);", "tuple assignment"),
        ("struct X{n:Int;}let x:=X{n:1};let chosen:=x<?x;", "__lt__"),
        (
            "struct X{n:Int;}fn __lt__(a:X,b:X)->Int{return 1;}let x:=X{n:1};let chosen:=x<?x;",
            "type mismatch",
        ),
    ] {
        let out = Case::new(source).compile();
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && error.contains(message) && error.contains("main.rusk"),
            "{source}\n{error}"
        );
    }
}
#[test]
fn integer_results_match_an_independent_oracle() {
    let source = "while let (Some(a),Some(b))=(try_read::<Int>(),try_read::<Int>()){let low=a;let high=b;low<?=b;high>?=a;wt a<?b,a>?b,low,high;}";
    let mut input = String::new();
    let mut expected = String::new();
    for a in -12i64..=12 {
        for b in -12i64..=12 {
            input += &format!("{a} {b}\n");
            expected += &format!("{} {} {} {}\n", a.min(b), a.max(b), a.min(b), a.max(b));
        }
    }
    let out = Case::new(source).run(&input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}
#[test]
fn exits_from_operands_keep_existing_control_flow_and_borrow_lifetimes() {
    output(
        r#"
let s:="abc";let target:=4;
outer:loop{target<?={let c:=s[0];break outer;0};}
s.push_back('!');wt target,s;
let n:=0;outer:while n<3{n+=1;let value:=n<?{continue outer;0};wt value;}
wt n;
"#,
        "4 abc!\n3\n",
    );
}

#[test]
fn runtime_failures_report_the_actual_operator_call_site() {
    for source in [
        "let s:=\"abc\";\nlet held:=s[0];\ns<?=\"z\";\ns<?=\"a\";\n",
        "let s:=\"abc\";\nlet held:=s[0];\ns>?=\"a\";\ns>?=\"zzzz\";\n",
    ] {
        let out = Case::new(source).run("");
        assert_eq!(out.status.code(), Some(1));
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("main.rusk:4:2:")
                && err.contains("active character references")
                && !err.contains("<stdlib/"),
            "{err}"
        );
    }
}
