mod support;
use support::Case;
fn library(file: &str) -> String {
    format!(
        "include {:?};\n",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/beta04/library")
            .join(file)
    )
}
#[test]
fn heap_and_segment_tree_support_structs_and_noncommutative_operations() {
    let source = library("priority_queue.rusk")
        + &library("segment_tree.rusk")
        + r#"
struct Item{priority:Int;name:Str;}
let item:=Item{priority:2,name:"b"};let items:Vec<Item>:=[];
let queue:=PriorityQueue(items,(a:Item,b:Item)=>a.priority<b.priority);
queue.push(item);item.priority=0;queue.push(Item{priority:1,name:"a"});
let first:=queue.top();first.priority=99;wt queue.pop().name,queue.pop().name;
let words:=SegTree(["a","b","c","d"],"",(a:Str,b:Str)=>a+b);
wt words.prod(1,4),words.max_right(0,(s:Str)=>s.len()<=2),words.min_left(4,(s:Str)=>s.len()<=2);
assert(words.max_right(0,(s:Str)=>!s.contains("ab"))==1);
assert(words.min_left(4,(s:Str)=>!s.contains("cd"))==3);
let detached=words;words.set(1,"X");wt words.prod(0,4),detached.prod(0,4);
let empty:Vec<Int>:=[];let seg:=SegTree(empty,0,(a:Int,b:Int)=>a+b);
wt seg.prod(0,0),seg.max_right(0,(x:Int)=>true),seg.min_left(0,(x:Int)=>true);
"#;
    let case = Case::new(&source);
    let out = case.run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"a b\nbcd 2 2\naXcd abcd\n0 0 0\n");
    let plain = case.run_with_options("", &["--no-opt"]);
    assert_eq!(out.stdout, plain.stdout);
    assert_eq!(out.stderr, plain.stderr);
}
#[test]
fn segment_tree_boundaries_match_naive_scans_after_random_updates() {
    let source = library("segment_tree.rusk")
        + r#"
in count:Int;
let seg:=SegTree([0;23],0,(a:Int,b:Int)=>a+b);let limit:=0;let predicate:=(x:Int)=>x<=limit;
for step in 0..count {
    in position,value,bound,next_limit:Int;limit=next_limit;seg.set(position,value);
    wt seg.max_right(bound,predicate),seg.min_left(bound,predicate);
    if step%15==0 {wt seg.prod(0,23);}
}
"#;
    let mut input = String::from("180\n");
    let mut values = [0i64; 23];
    let mut seed = 314159265u64;
    let mut expected = String::new();
    for step in 0..180 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let position = (seed % 23) as usize;
        let value = ((seed >> 16) % 11) as i64;
        values[position] = value;

        let bound = ((seed >> 32) % 24) as usize;
        let limit = ((seed >> 40) % 70) as i64;
        let (mut right, mut sum) = (bound, 0);
        while right < 23 && sum + values[right] <= limit {
            sum += values[right];
            right += 1;
        }
        let (mut left, mut sum) = (bound, 0);
        while left > 0 && sum + values[left - 1] <= limit {
            left -= 1;
            sum += values[left];
        }
        input += &format!("{position} {value} {bound} {limit}\n");
        expected += &format!("{right} {left}\n");
        if step % 15 == 0 {
            expected += &format!("{}\n", values.iter().sum::<i64>());
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
fn heap_matches_a_sorted_multiset() {
    let mut source = library("priority_queue.rusk")
        + "let empty:Vec<Int>:=[];let q:=PriorityQueue(empty,(a:Int,b:Int)=>a<b);\n";
    let mut values = vec![];
    let mut expected = String::new();
    let mut seed = 7u64;
    for _ in 0..250 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        if values.is_empty() || !seed.is_multiple_of(3) {
            let x = ((seed >> 32) % 1000) as i64;
            values.push(x);
            source += &format!("q.push({x});\n");
        } else {
            values.sort();
            let x = values.remove(0);
            source += "wt q.top(),q.pop();\n";
            expected += &format!("{x} {x}\n");
        }
    }
    values.sort();
    for x in values {
        source += "wt q.pop();\n";
        expected += &format!("{x}\n");
    }
    let out = Case::new(&source).run("");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, expected.as_bytes());
}
#[test]
fn empty_pops_and_invalid_search_conditions_fail() {
    for source in [
        library("priority_queue.rusk")
            + "let a:Vec<Int>:=[];let q:=PriorityQueue(a,(a:Int,b:Int)=>a<b);q.pop();",
        library("priority_queue.rusk")
            + "let a:Vec<Int>:=[];let q:=PriorityQueue(a,(a:Int,b:Int)=>a<b);q.top();",
        library("segment_tree.rusk")
            + "let seg:=SegTree([1],0,(a:Int,b:Int)=>a+b);wt seg.max_right(0,(x:Int)=>false);",
        "let a:Vec<Int>:=[];a.pop_back();".into(),
    ] {
        let out = Case::new(&source).run("");
        assert!(!out.status.success());
        assert!(!out.stderr.is_empty());
    }
}

#[test]
fn new_judge_examples_match_samples_and_small_oracles() {
    for (path, input, expected) in [
        (
            "aoj/grl_1_a.test.rusk",
            "4 5 0\n0 1 1\n0 2 4\n1 2 2\n2 3 1\n1 3 5\n",
            "0\n1\n3\n4\n",
        ),
        ("aoj/dsl_3_a.test.rusk", "6 6\n1 2 1 2 3 2\n", "3\n"),
        (
            "aoj/dsl_3_a_min_left.test.rusk",
            "6 6\n1 2 1 2 3 2\n",
            "3\n",
        ),
        ("aoj/alds1_4_d.test.rusk", "5 3\n8\n1\n7\n3\n9\n", "10\n"),
        (
            "aoj/itp1_4_d_fold.test.rusk",
            "5\n10 1 5 4 17\n",
            "1 17 37\n",
        ),
        (
            "aoj/alds1_2_c.test.rusk",
            "5\nH4 C9 S4 D2 C3\n",
            "D2 C3 H4 S4 C9\nStable\nD2 C3 S4 H4 C9\nNot stable\n",
        ),
        (
            "library_checker/point_set_range_composite.test.rusk",
            "2 3\n2 1\n3 4\n1 0 2 5\n0 0 1 0\n1 0 2 5\n",
            "37\n19\n",
        ),
    ] {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(path);
        let out = Case::new(&format!("include {file:?};")).run(input);
        assert!(
            out.status.success(),
            "{path}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, expected.as_bytes(), "{path}");
    }
}
