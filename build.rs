fn main() {
    println!("cargo:rerun-if-changed=rusk.par");
    parol::build::Builder::with_explicit_output_dir(std::env::var("OUT_DIR").unwrap())
        .max_lookahead(3)
        .expect("valid lookahead")
        .grammar_file("rusk.par")
        .parser_output_file("rusk_parser.rs")
        .actions_output_file("rusk_grammar_trait.rs")
        .user_type_name("RuskGrammar")
        .user_trait_module_name("rusk_grammar")
        .trim_parse_tree()
        .generate_parser()
        .expect("Rusk grammar generation failed");
    // Generated module-level lint attributes cannot appear inside include!.
    for name in ["rusk_parser.rs", "rusk_grammar_trait.rs"] {
        let path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        let text = text
            .lines()
            .filter(|line| !line.starts_with("#!["))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, text).unwrap();
    }
}
