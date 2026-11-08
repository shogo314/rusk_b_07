use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub(crate) struct Case(pub(crate) PathBuf);
impl Case {
    pub(crate) fn new(source: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rusk-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rusk"), source).unwrap();
        Self(dir)
    }
    pub(crate) fn compile(&self) -> Output {
        self.compile_with_options(&[])
    }
    pub(crate) fn compile_with_options(&self, options: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rusk"))
            .arg("compile")
            .arg(self.0.join("main.rusk"))
            .args(options)
            .args(std::env::var_os("RUSK_TEST_NO_OPT").map(|_| "--no-opt"))
            .arg("-o")
            .arg(self.0.join("main.cpp"))
            .output()
            .unwrap()
    }
    pub(crate) fn run(&self, input: &str) -> Output {
        self.run_with_options(input, &[])
    }
    pub(crate) fn run_with_options(&self, input: &str, options: &[&str]) -> Output {
        let compiled = if options.is_empty() {
            self.compile()
        } else {
            self.compile_with_options(options)
        };
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let native = Command::new("g++")
            .args(["-std=c++20", "-O2", "-Wall", "-Wextra"])
            .arg(self.0.join("main.cpp"))
            .arg("-o")
            .arg(self.0.join("main"))
            .output()
            .unwrap();
        assert!(
            native.status.success(),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        self.execute(input)
    }
    pub(crate) fn execute(&self, input: &str) -> Output {
        let mut child = Command::new(self.0.join("main"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let written = child.stdin.take().unwrap().write_all(input.as_bytes());
        // Programs may deliberately fail before consuming all supplied input.
        // Preserve their exit status and diagnostics even if stdin closes early.
        let output = child.wait_with_output().unwrap();
        if let Err(error) = written {
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe, "{error}");
        }
        output
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
