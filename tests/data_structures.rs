mod support;
use support::Case;

fn with_library(source: &str) -> String {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/beta02/library/");
    format!(
        "include {:?};\ninclude {:?};\n{source}",
        format!("{directory}fenwick.rusk"),
        format!("{directory}sum_segment_tree.rusk")
    )
}

#[test]
fn generic_data_structures_work_with_int_and_user_types() {
    let source = with_library(
        r#"
struct Weight { value:Int; }
fn __add__(a:Weight,b:Weight)->Weight { return Weight { value:a.value+b.value }; }
fn __sub__(a:Weight,b:Weight)->Weight { return Weight { value:a.value-b.value }; }
let values:=[1,2,3];
let fw:=Fenwick(values,0); let seg:=SumSegTree(values,0);
fw.add(1,5); seg.set(1,7); wt fw.range_sum(0,3),seg.prod(0,3),values;
let zero:=Weight { value:0 };
let weights:=[Weight { value:2 },Weight { value:3 }];
let generic_fw:=Fenwick(weights,zero); let generic_seg:=SumSegTree(weights,zero);
generic_fw.add(0,Weight { value:4 }); generic_seg.set(0,Weight { value:6 });
wt generic_fw.range_sum(0,2).value,generic_seg.prod(0,2).value,zero.value,weights[0].value;
let detached = generic_fw; detached.add(0,Weight { value:1 });
wt generic_fw.prefix_sum(1).value,detached.prefix_sum(1).value;
let strings:=SumSegTree(["a","b","c","d","e"],"");
wt strings.prod(1,5); strings.set(2,"X"); wt strings.prod(0,5);
let empty:=Fenwick::<Int>(0,0); wt empty.range_sum(0,0);
let empty_seg:=SumSegTree::<Int>([],0); wt empty_seg.prod(0,0);
"#,
    );
    let out = Case::new(&source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.stdout,
        b"11 11 1 2 3\n9 9 0 2\n6 7\nbcde\nabXde\n0\n0\n"
    );
}

#[test]
fn data_structure_queries_match_a_naive_model() {
    let source = with_library(
        r#"
in n,q:Int;
in initial(n):Vec<Int>;
let fw:=Fenwick(initial,0); let seg:=SumSegTree(initial,0);
for query in 0..q {
    in kind,left,right:Int;
    if kind==0 { fw.add(left,right); seg.set(left,seg.prod(left,left+1)+right); }
    else { wt fw.range_sum(left,right),seg.prod(left,right); }
}
"#,
    );
    let mut values = [0i64; 17];
    let mut input = String::from("17 200\n0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n");
    let mut expected = String::new();
    let mut seed = 314u64;
    for query in 0..200 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = ((seed >> 32) % 17) as usize;
        if query % 3 == 0 {
            let delta = (seed % 21) as i64 - 10;
            values[a] += delta;
            input.push_str(&format!("0 {a} {delta}\n"));
        } else {
            let b = (seed % 18) as usize;
            let left = a.min(b);
            let right = a.max(b);
            let sum: i64 = values[left..right].iter().sum();
            input.push_str(&format!("1 {left} {right}\n"));
            expected.push_str(&format!("{sum} {sum}\n"));
        }
    }
    let out = Case::new(&source).run(&input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
fn invalid_data_structure_indices_fail_instead_of_looping() {
    for statement in [
        "let fw:=Fenwick(2,0); fw.add(-1,1);",
        "let fw:=Fenwick(2,0); fw.add(2,1);",
        "let fw:=Fenwick(2,0); fw.prefix_sum(3);",
        "let fw:=Fenwick(2,0); fw.range_sum(2,1);",
        "let seg:=SumSegTree([1,2],0); seg.set(2,0);",
        "let seg:=SumSegTree([1,2],0); seg.prod(0,3);",
    ] {
        let out = Case::new(&with_library(statement)).run("");
        assert_eq!(out.status.code(), Some(1), "{statement}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("assertion failed"));
    }
}

#[test]
fn bit_operations_keep_integer_values_independent() {
    let out = Case::new("let a:=6; let b:=3; wt a&b,a|b,a^b,(-1)&8,1|2^3&6,a,b; assert((a&b)==2);")
        .run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"2 7 5 8 1 6 3\n");
}
