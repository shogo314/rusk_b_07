use crate::ast::{Span, Stmt};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub struct Loaded {
    pub statements: Vec<Stmt>,
    pub dependencies: Vec<PathBuf>,
}

#[derive(Default)]
struct Loader {
    loaded: BTreeSet<PathBuf>,
    active: Vec<PathBuf>,
}

impl Loader {
    fn expand(
        &mut self,
        path: &Path,
        from: Option<&Span>,
        output: &mut Vec<Stmt>,
    ) -> Result<(), String> {
        let error = |message: String| {
            from.map_or_else(
                || format!("{}: {message}", path.display()),
                |span| span.error(&message),
            )
        };
        let canonical = path
            .canonicalize()
            .map_err(|e| error(format!("cannot load '{}': {e}", path.display())))?;
        if self.active.contains(&canonical) {
            let mut cycle = self
                .active
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>();
            cycle.push(canonical.display().to_string());
            return Err(error(format!("include cycle: {}", cycle.join(" -> "))));
        }
        if self.loaded.contains(&canonical) {
            return Ok(());
        }
        let text = std::fs::read_to_string(&canonical)
            .map_err(|e| error(format!("cannot read '{}': {e}", canonical.display())))?;
        let statements = crate::parse(&text, &canonical.display().to_string())?;
        self.active.push(canonical.clone());
        for statement in statements {
            if let Stmt::Include(name, span) = statement {
                self.expand(&canonical.parent().unwrap().join(name), Some(&span), output)?;
            } else {
                output.push(statement);
            }
        }
        self.active.pop();
        self.loaded.insert(canonical);
        Ok(())
    }
}

pub fn load(path: &Path) -> Result<Loaded, String> {
    let mut loader = Loader::default();
    let mut statements = vec![];
    loader.expand(path, None, &mut statements)?;
    Ok(Loaded {
        statements,
        dependencies: loader.loaded.into_iter().collect(),
    })
}
