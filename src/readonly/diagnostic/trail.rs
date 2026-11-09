use crate::readonly::AccessPaths;
use crate::{ast::Span, effects::Projection, hir::AccessInfo};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Step {
    span: Option<Span>,
    text: String,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Trail {
    steps: Vec<Step>,
    pub uncertain: bool,
    pub truncated: bool,
}
impl Trail {
    pub fn origin(source: &AccessInfo) -> Self {
        let mut trail = Self::default();
        trail.push(
            Some(source.declaration.clone()),
            format!("readonly binding '{}' introduced here", source.name),
        );
        trail
    }
    pub fn push(&mut self, span: Option<Span>, text: String) {
        if self.steps.len() < 16 {
            self.steps.push(Step { span, text });
        } else {
            self.truncated = true;
        }
    }
    fn better(&self, other: &Self) -> bool {
        (
            self.uncertain,
            self.truncated,
            self.steps.len(),
            &self.steps,
        ) < (
            other.uncertain,
            other.truncated,
            other.steps.len(),
            &other.steps,
        )
    }
    pub fn render(&self) -> String {
        let mut lines: Vec<_> = self
            .steps
            .iter()
            .map(|step| {
                format!(
                    "  {}",
                    step.span
                        .as_ref()
                        .map_or_else(|| step.text.clone(), |span| span.error(&step.text))
                )
            })
            .collect();
        if self.truncated {
            lines.push("  ... additional readonly propagation steps omitted".into());
        }
        lines.join("\n")
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Paths(pub BTreeMap<Vec<Projection>, Trail>);
impl Paths {
    pub fn cost(&self) -> usize {
        1 + self
            .0
            .iter()
            .map(|(path, trail)| 1 + path.len() + trail.steps.len())
            .sum::<usize>()
    }
    pub fn origin(source: &AccessInfo) -> Self {
        Self([(vec![], Trail::origin(source))].into())
    }
    pub fn insert(&mut self, path: Vec<Projection>, trail: Trail) {
        if self.0.get(&path).is_none_or(|old| trail.better(old)) {
            self.0.insert(path, trail);
        }
        if (self.0.len() > 64 || self.0.keys().any(|p| p.len() > 16))
            && let Some(mut trail) = self.best()
        {
            trail.truncated = true;
            trail.uncertain = true;
            self.0 = [(vec![], trail)].into();
        }
    }
    pub fn join(&mut self, other: &Self) {
        for (path, trail) in &other.0 {
            self.insert(path.clone(), trail.clone());
        }
    }
    pub fn best(&self) -> Option<Trail> {
        self.0
            .values()
            .cloned()
            .reduce(|a, b| if b.better(&a) { b } else { a })
    }
    pub fn at(&self, path: &[Projection]) -> Option<Trail> {
        self.0
            .iter()
            .filter(|(prefix, _)| path.starts_with(prefix))
            .map(|(_, trail)| trail.clone())
            .reduce(|a, b| if b.better(&a) { b } else { a })
    }
    pub fn step(&mut self, span: Option<Span>, text: String) {
        for trail in self.0.values_mut() {
            trail.push(span.clone(), text.clone());
        }
    }
    pub fn uncertain(&mut self) {
        for trail in self.0.values_mut() {
            trail.uncertain = true;
        }
    }
    pub fn truncate(&mut self) {
        for trail in self.0.values_mut() {
            trail.truncated = true;
        }
    }
    pub fn project(&self, projection: &Projection) -> Self {
        let mut result = Self::default();
        for (path, trail) in &self.0 {
            if path.is_empty() {
                result.insert(vec![], trail.clone());
            } else if path.first() == Some(projection) {
                result.insert(path[1..].to_vec(), trail.clone());
            }
        }
        result
    }
    pub fn contain(&self, prefix: &[Projection]) -> Self {
        let mut result = Self::default();
        for (path, trail) in &self.0 {
            let mut path = path.clone();
            path.splice(..0, prefix.iter().cloned());
            result.insert(path, trail.clone());
        }
        result
    }
    pub fn fit(&self, actual: &AccessPaths, source: &AccessInfo, heap: Option<&Trail>) -> Self {
        let mut result = Self::default();
        for (path, sources) in &actual.0 {
            if !sources.contains(source) {
                continue;
            }
            let mut trail = self.at(path).unwrap_or_else(|| {
                let mut trail = self.best().or_else(|| heap.cloned()).unwrap_or_else(|| Trail::origin(source));
                trail.uncertain = true;
                trail.push(None, "readonly propagation through contained or untracked references is not fully reconstructed".into());
                trail
            });
            trail.uncertain |= actual.1;
            result.insert(path.clone(), trail);
        }
        result
    }
    pub fn access(&self, source: &AccessInfo) -> AccessPaths {
        AccessPaths(
            self.0
                .keys()
                .map(|path| (path.clone(), [source.clone()].into()))
                .collect(),
            self.0.values().any(|trail| trail.uncertain),
        )
    }
}
