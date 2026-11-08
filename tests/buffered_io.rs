mod support;
use support::Case;

#[test]
fn input_tokens_and_output_cross_buffer_boundaries() {
    let case = Case::new("in a:Int,s:Str,b:Int; wt a,s,b;");
    let text = "x".repeat(131_100);
    let input = format!(
        "{}+9223372036854775807 {} -9223372036854775808",
        " ".repeat(65530),
        text
    );
    let out = case.run(&input);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("9223372036854775807 {text} -9223372036854775808\n")
    );
}

#[test]
fn token_character_and_line_input_share_one_buffer() {
    let case = Case::new(
        "in n:Int; let rest:=read_line(); in c:Char; let tail:=read_line(); let last:=read_line(); wt n,rest,c,tail,last;",
    );
    let out = case.run("12 remaining\r\n  X tail\nlast");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"12  remaining X  tail last\n");
    let out = Case::new(r#"let s:="a\u0000b"; wt s; let c:='\0'; wt c;"#).run("");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"a\0b\n\0\n");
}

#[test]
fn extern_iostreams_interoperate_with_buffered_helpers() {
    let out=Case::new(r#"
extern fn external_read()->Int {
 cpp_name:"external_read",
 code:"Obj<std::int64_t> external_read(Loc){std::int64_t n; std::cin>>n; std::cout<<\"external \"; return make_obj<std::int64_t>(n);}",
}
in first:Int; wt first; let second:=external_read(); in third:Int; wt second,third;
"#).run("1 2 3");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"1\nexternal 2 3\n");
}

#[test]
fn invalid_tokens_at_buffer_edges_are_located() {
    let case = Case::new("in x:Int; wt x;");
    let out = case.run("0");
    assert!(out.status.success());
    for token in [
        "9223372036854775808",
        "-9223372036854775809",
        "+",
        "--1",
        "12x",
        "",
    ] {
        let out = case.execute(&format!("{}{token}", " ".repeat(65530)));
        assert!(!out.status.success(), "{token}");
        assert!(out.stdout.is_empty());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains("main.rusk:1:4:"), "{error}");
    }
}
