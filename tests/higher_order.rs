mod support;
use support::Case;
fn succeeds(source: &str, expected: &str) {
    let case = Case::new(source);
    let out = case.run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
    let plain = case.run_with_options("", &["--no-opt"]);
    assert_eq!(out.stdout, plain.stdout);
    assert_eq!(out.stderr, plain.stderr);
    assert_eq!(out.status.code(), plain.status.code());
}
#[test]
fn lazy_map_filter_share_positions_and_stop_at_short_circuit() {
    succeeds(
        r#"
let seen:=0;let mapped:=0;let values:=[1,2,3,4,5,6];
let it:=values.filter((x:Int)=>((seen+=1)>0 && x%2==0)).map((x:Int)=>((mapped+=1)*0+x*10));
wt seen,mapped;
for x in it{wt x;break;}
wt seen,mapped;wt it.to_vec();wt seen,mapped;
let aliases:=values.filter((x:Int)=>x%2==1).to_vec();aliases[0]=9;wt values;
let empty:Vec<Int>:=[];wt all(empty,(x:Int)=>false),any(empty,(x:Int)=>true);
let next:=values.map((x:Int)=>x);wt next.any((x:Int)=>x==2);wt next.to_vec();
"#,
        "0 0\n20\n2 1\n40 60\n6 3\n9 2 3 4 5 6\nYes No\nYes\n3 4 5 6\n",
    );
}
#[test]
fn copying_lazy_iterators_separates_state_and_captured_environment() {
    succeeds(
        r#"
let n:=0;let it:=(0..4).map((x:Int)=>x+(n+=1));
let detached=it;wt it.to_vec(),n;wt detached.to_vec(),n;
let a:=[1,2,3];let f:=a.filter((x:Int)=>x>1);let c=()=>f.to_vec();
wt c(),f.to_vec();
"#,
        "1 3 5 7 4\n1 3 5 7 4\n2 3 2 3\n",
    );
}
#[test]
fn folding_scanning_and_search_handle_empty_ranges_and_boundaries() {
    succeeds(
        r#"
let seed:=10;wt fold([1,2,3],seed,(a:Int,b:Int)=>a+b),seed;
wt scan([1,2,3],0,(a:Int,b:Int)=>a+b);
let snapshots:=scan([1,2,3],0,(a:Int,b:Int)=>(a+=b));wt snapshots;
let empty:Vec<Int>:=[];wt fold(empty,7,(a:Int,b:Int)=>a+b),scan(empty,7,(a:Int,b:Int)=>a+b);
wt first_true(0,100,(x:Int)=>x*x>=40),first_true(0,10,(x:Int)=>false),first_true(3,3,(x:Int)=>true);
wt first_true(-9223372036854775808,9223372036854775807,(x:Int)=>x>=0);
"#,
        "16 10\n0 1 3 6\n0 1 3 6\n7 7\n7 10 3\n0\n",
    );
}
#[test]
fn sorting_is_stable_and_keys_are_snapshotted_once() {
    succeeds(
        r#"
struct Point{x:Int;id:Int;}
let a:=[Point{x:2,id:0},Point{x:1,id:1},Point{x:2,id:2},Point{x:1,id:3}];
let calls:=0;sort_by_key(a,(p:Point)=>p.x+(calls+=1)*0);
for p in a{wt p.id;} wt calls;
sort_by_comp(a,(a:Point,b:Point)=>a.x>b.x);for p in a{wt p.id;}
let key:=0;let ints:=[3,2,1];sort_by_key(ints,(x:Int)=>(key=x));wt ints,key;
let empty:Vec<Int>:=[];sort_by_key(empty,(x:Int)=>(calls+=1));wt calls;
"#,
        "1\n3\n0\n2\n4\n0\n2\n1\n3\n1 2 3 1\n4\n",
    );
}
#[test]
fn mutation_and_invalid_callback_signatures_fail_safely() {
    for source in [
        "let a:=[1,2];let it:=a.map((x:Int)=>a.len()+((a=[]).len()));wt it.to_vec();",
        "let a:=[1,2];sort_by_key(a,(x:Int)=>a.len()+((a=[]).len()));",
        "let a:=[1,2];sort_by_comp(a,(x:Int,y:Int)=>(a=[]).len()==0);",
    ] {
        let case = Case::new(source);
        let out = case.run("");
        assert!(!out.status.success(), "{source}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("container changed"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    for source in [
        "wt map([1],(x:Str)=>x);",
        "wt any([1],(x:Int)=>x);",
        "wt first_true(0,1,(x:Int)=>x);",
        "wt fold([1],0,(a:Int,b:Int)=>true);",
    ] {
        let out = Case::new(source).compile();
        assert!(!out.status.success(), "{source}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("main.rusk:"));
    }
}
