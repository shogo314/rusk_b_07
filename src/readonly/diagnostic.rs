//! Explain an independently proved access violation. Provenance is replayed
//! only after failure and never participates in access or effect inference.
mod routes;
mod trail;
use crate::{
    hir::{Rvalue, Value},
    lir::{self, Operation},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Failure {
    pub message: String,
    pub value: Value,
    source: Box<crate::hir::AccessInfo>,
    path: Vec<crate::effects::Projection>,
    uncertain: bool,
}
impl Failure {
    pub fn new(message: String, value: &Value, access: &super::AccessPaths, unknown: bool) -> Self {
        let (path, sources) = access.0.first_key_value().unwrap();
        Self {
            message,
            value: value.clone(),
            source: Box::new(sources.first().unwrap().clone()),
            path: path.clone(),
            uncertain: unknown || access.1,
        }
    }
    pub fn explain(
        self,
        body: &lir::Function,
        program: &lir::Program,
        summaries: &[super::Summary],
        point: (usize, usize),
    ) -> String {
        let route = routes::explain(&self, body, program, summaries, point);
        let mut names = BTreeMap::new();
        let mut definitions = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for block in &body.blocks {
            for op in &block.instructions {
                if let Operation::DescribeBinding {
                    value,
                    name,
                    captured,
                } = op
                {
                    names.insert(value.id.clone(), (name, *captured));
                }
                let result = match op {
                    Operation::Object { result, .. } => Some(result),
                    Operation::Call(call) => Some(&call.result),
                    _ => None,
                };
                if let Some(result) = result
                    && definitions.insert(result.id.clone(), op).is_some()
                {
                    ambiguous.insert(result.id.clone());
                }
            }
        }
        let mut value = &self.value;
        let mut visited = BTreeSet::new();
        let mut steps = vec![];
        for _ in 0..16 {
            if !visited.insert(value.id.clone()) {
                break;
            }
            if let Some((name, captured)) = names.get(&value.id) {
                steps.push(name.span.error(format!(
                    "access uses {}binding '{}'",
                    if *captured { "captured " } else { "" },
                    name.text
                )));
                break;
            }
            if ambiguous.contains(&value.id) {
                break;
            }
            match definitions.get(&value.id) {
                Some(Operation::Object {
                    expression: Rvalue::Share(source),
                    ..
                }) => value = source,
                Some(Operation::Object {
                    expression: Rvalue::Field { owner, index },
                    ..
                }) => {
                    let name = program
                        .types
                        .get(&owner.ty)
                        .and_then(|ty| ty.fields())
                        .and_then(|fields| fields.get(*index))
                        .map(|(name, _)| name.as_str());
                    steps.push(format!(
                        "access selects field '{}' of '{}'",
                        name.unwrap_or("unknown"),
                        owner.ty
                    ));
                    value = owner;
                }
                Some(Operation::Object {
                    expression:
                        Rvalue::VariantField {
                            owner,
                            variant,
                            index,
                        },
                    ..
                }) => {
                    let name = match program.types.get(&owner.ty) {
                        Some(crate::hir::Type::Enum(variants)) => {
                            variants.get(*variant).map(|(name, _)| name.as_str())
                        }
                        _ => None,
                    };
                    steps.push(format!(
                        "access selects payload {} of variant '{}'",
                        index + 1,
                        name.unwrap_or("unknown")
                    ));
                    value = owner;
                }
                Some(Operation::Call(call)) => {
                    steps.push(call.span.error(format!(
                        "access uses result of '{}'",
                        program.functions[call.function].name
                    )));
                    break;
                }
                _ => break,
            }
        }
        let message = if steps.is_empty() {
            self.message
        } else {
            format!(
                "{}\nchecked access:\n  {}",
                self.message,
                steps.join("\n  ")
            )
        };
        format!("{message}{route}")
    }
}
