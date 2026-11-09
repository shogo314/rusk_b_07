mod support;
use support::Case;

const BINDINGS: &str = include_str!("../docs/reference/bindings.md");
const ACCESS: &str = include_str!("../docs/reference/access.md");
const ESCAPE: &str = include_str!("../docs/reference/escape.md");

// Read the published examples, rather than maintaining a second copy of them.
// Counts below make added/removed examples require an explicit coverage choice.
fn examples(document: &str, count: usize) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = None;
    for line in document.lines() {
        if line == "```" {
            if let Some(source) = current.take() {
                blocks.push(source);
            }
        } else if matches!(line, "```text" | "```rusk") {
            assert!(current.is_none());
            current = Some(String::new());
        } else if let Some(source) = &mut current {
            source.push_str(line);
            source.push('\n');
        }
    }
    assert!(current.is_none(), "unterminated example");
    assert_eq!(blocks.len(), count, "update coverage for changed examples");
    blocks
}

fn runs(source: &str, suffix: &str, expected: &str) {
    let source = format!("{source}\n{suffix}");
    let output = Case::new(&source).run("");
    assert!(
        output.status.success(),
        "{source}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes(), "{source}");
    assert!(
        output.stderr.is_empty(),
        "{source}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rejects(source: &str, expected: &str) -> String {
    let case = Case::new(source);
    let mut previous = None;
    for stage in ["hir", "lir", "cpp"] {
        let output = case.compile_with_options(&["--emit", stage]);
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success(), "{source}");
        assert!(message.contains(expected), "{source}\n{message}");
        assert!(!case.0.join("main.cpp").exists());
        if let Some(previous) = &previous {
            assert_eq!(previous, &message);
        }
        previous = Some(message);
    }
    previous.unwrap()
}

fn replace(source: &str, from: &str, to: &str) -> String {
    assert_eq!(source.matches(from).count(), 1, "example changed: {from}");
    source.replacen(from, to, 1)
}

#[test]
fn fixed_examples_run_and_check_each_initialization_branch() {
    let blocks = examples(BINDINGS, 4);
    for (index, suffix, expected) in [
        (0, "", "5 9\n"),
        (1, "wt increment(left),left,right,next(3);", "2 2 2 4\n"),
        (2, "wt values,alias;", "3 2 4 9\n"),
        (3, "wt choose(true),choose(false);", "1 2\n"),
    ] {
        runs(&blocks[index], suffix, expected);
    }
}

#[test]
fn the_documented_fixed_rebinding_is_rejected_before_output() {
    let blocks = examples(BINDINGS, 4);
    rejects(
        &replace(&blocks[0], "// count := 9;", "count := 9;"),
        "cannot rebind fixed variable 'count'",
    );
    rejects(
        &(blocks[1].clone() + "\nright := 3;"),
        "cannot rebind fixed variable 'right'",
    );
}

#[test]
fn readonly_examples_keep_shared_updates_and_detach_copies() {
    let blocks = examples(ACCESS, 8);
    for (index, suffix, expected) in [
        (0, "wt original,detached;", "2\n2 3\n"),
        (1, "wt twice(left),next(right),working;", "2 3 3\n"),
        (2, "wt value;", "2\n"),
        (3, "", "2 1\n"),
        (4, "", "2\n"),
        (5, "wt result,detached;", "1 2\n"),
    ] {
        runs(&blocks[index], suffix, expected);
    }
}

#[test]
fn commented_readonly_violations_and_the_diagnostic_excerpt_match_the_compiler() {
    let blocks = examples(ACCESS, 8);
    for (index, comment) in [
        (0, "// view = 3;"),
        (0, "// let alias := view; alias += 1;"),
        (2, "// value = 3;"),
        (3, "// values[0] = 3;"),
        (3, "// values.push_back(3);"),
        (4, "// bump(x);"),
        (5, "// result = 2;"),
    ] {
        rejects(
            &replace(
                &blocks[index],
                comment,
                comment.strip_prefix("// ").unwrap(),
            ),
            "cannot modify readonly access",
        );
    }
    rejects(
        &(blocks[1].clone() + "\nright = 3;"),
        "cannot modify readonly access",
    );
    let message = rejects(&blocks[6], "cannot modify readonly access");
    let mut rest = message.split("readonly propagation path:").nth(1).unwrap();
    for line in blocks[7].lines() {
        let index = rest
            .find(line)
            .unwrap_or_else(|| panic!("missing diagnostic step {line:?}: {message}"));
        rest = &rest[index + line.len()..];
    }
}

fn box_source(block: &str) -> String {
    format!("struct Box{{value:Int;}}\n{block}")
}

#[test]
fn noescape_examples_are_instantiated_and_run_including_generic_callbacks() {
    let blocks = examples(ESCAPE, 10);
    for (index, suffix, expected) in [
        (
            0,
            "let value:=1;increment(value);let separate:=detached(value);let alias:=shared(value);value=7;wt value,separate,alias;",
            "7 2 7\n",
        ),
        (1, "wt twice(3),next(3);", "6 4\n"),
        (2, "wt copied_first([7]);", "7\n"),
        (
            3,
            "let values:Vec<Int>:=[];append(values,1);wt values;",
            "1\n",
        ),
        (
            4,
            "let value:=1;discard(value);let separate:=detached(value);value=7;wt separate,value;",
            "1 7\n",
        ),
        (5, "", "4\n"),
        (6, "wt local(3);", "4\n"),
        (7, "wt nested(3);", "4\n"),
        (8, "wt via_call(3);", "4\n"),
        (9, "wt via_container(3);", "4\n"),
    ] {
        let source = if index >= 7 {
            box_source(&blocks[index])
        } else {
            blocks[index].clone()
        };
        runs(&source, suffix, expected);
    }
}

#[test]
fn noescape_documented_rejections_are_checked_after_instantiation() {
    let blocks = examples(ESCAPE, 10);
    let source = replace(&blocks[0], "fn shared(escape", "fn shared(noescape");
    rejects(&source, "noescape parameter 'value' may escape");
    let source = replace(&blocks[2], "// fn first", "fn first") + "\nwt first([7]);";
    rejects(&source, "noescape parameter 'values' may escape");
    let source = replace(&blocks[3], "escape value:", "noescape value:")
        + "\nlet values:Vec<Int>:=[];append(values,1);";
    rejects(&source, "noescape parameter 'value' may escape");
    let source = replace(
        &blocks[5],
        "// wt invoke(() => value);",
        "wt invoke(() => value);",
    );
    rejects(&source, "noescape parameter 'callback' may escape");
    let source = box_source(&replace(
        &blocks[9],
        "box.value := value;",
        "held[0].value := value;",
    )) + "\nwt via_container(3);";
    rejects(&source, "cannot prove noescape for parameter 'value'");
    let source = box_source(&replace(
        &blocks[9],
        "return held[0].value + 1;",
        "return box.value;",
    )) + "\nwt via_container(3);";
    rejects(&source, "cannot prove noescape for parameter 'value'");
}
