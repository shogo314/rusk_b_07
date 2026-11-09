//! Failure-only replay. Safety states and summaries remain authoritative;
//! missing witnesses are explicitly uncertain rather than invented histories.
mod calls;
mod flow;
use super::Failure;
use super::trail::{Paths, Trail};
use crate::{
    ast::Name,
    effects::{Effect, Projection, projection_name},
    hir::{AccessInfo, Place, Rvalue, Value, ValueId},
    lir::{self, Operation, Terminator},
    readonly::{AccessPaths, State, Summary},
};
use std::collections::{BTreeMap, BTreeSet};

fn has(access: &AccessPaths, source: &AccessInfo) -> bool {
    access.0.values().any(|sources| sources.contains(source))
}
#[derive(Clone, Default, PartialEq, Eq)]
struct Routes {
    values: BTreeMap<ValueId, Paths>,
    heap: Paths,
    names: BTreeMap<ValueId, (Name, bool)>,
    types: BTreeMap<ValueId, String>,
}
impl Routes {
    fn cost(&self) -> usize {
        self.names.len()
            + self.types.len()
            + self.heap.cost()
            + self.values.values().map(Paths::cost).sum::<usize>()
    }
    fn new(body: &lir::Function) -> Self {
        let mut result = Self::default();
        for value in &body.slots {
            result.types.insert(value.id.clone(), value.ty.clone());
        }
        for block in &body.blocks {
            for op in &block.instructions {
                if let Operation::DescribeBinding {
                    value,
                    name,
                    captured,
                } = op
                {
                    result
                        .names
                        .insert(value.id.clone(), (name.clone(), *captured));
                    result.types.insert(value.id.clone(), value.ty.clone());
                }
                if let Operation::Declare {
                    initial: Some(value),
                    ..
                } = op
                {
                    result.types.insert(value.id.clone(), value.ty.clone());
                }
            }
        }
        result
    }
    fn join(&mut self, other: &Self) -> bool {
        let old_values = self.values.clone();
        let old_heap = self.heap.clone();
        for (id, paths) in &other.values {
            self.values.entry(id.clone()).or_default().join(paths);
        }
        self.heap.join(&other.heap);
        self.values != old_values || self.heap != old_heap
    }
    fn get(&self, value: &Value, state: &State, source: &AccessInfo) -> Paths {
        self.values.get(&value.id).cloned().unwrap_or_default().fit(
            &state.get(value),
            source,
            self.heap.best().as_ref(),
        )
    }
    fn store(
        &mut self,
        owner: &Value,
        projection: Projection,
        paths: &Paths,
        after: &State,
        context: &mut Context<'_>,
    ) {
        let mut paths = paths.clone();
        paths.step(
            None,
            format!(
                "readonly access stored in {}",
                projection_name(&owner.ty, &projection, context.program)
            ),
        );
        let paths = paths.contain(&[projection]);
        let target = after.reference(owner);
        for (id, reference) in &after.values {
            if !context.spend(1) {
                break;
            }
            let Some(ty) = self.types.get(id) else {
                continue;
            };
            let value = Value {
                id: id.clone(),
                ty: ty.clone(),
            };
            let actual = after.get(&value);
            if !has(&actual, context.source) {
                continue;
            }
            for prefix in after
                .objects
                .paths_to(reference, &target, &mut context.budget)
            {
                let routes = paths.contain(&prefix);
                // Keep only paths supported by the independent access proof.
                for (path, trail) in routes.0 {
                    if actual.0.iter().any(|(proof, sources)| {
                        sources.contains(context.source)
                            && (path.starts_with(proof) || proof.starts_with(&path))
                    }) {
                        self.values
                            .entry(id.clone())
                            .or_default()
                            .insert(path, trail);
                    }
                }
            }
        }
    }
    fn operation(
        &mut self,
        op: &Operation,
        before: &State,
        after: &State,
        context: &mut Context<'_>,
    ) {
        let source = context.source;
        let mut produced = match op {
            Operation::Literal { result, .. } => Some((result, Paths::default())),
            Operation::Declare { result, initial } => {
                let paths = if context.declaration(op, source) {
                    Paths::origin(source)
                } else {
                    initial
                        .as_ref()
                        .map(|v| self.get(v, before, source))
                        .unwrap_or_default()
                };
                Some((result, paths))
            }
            Operation::AnnotateAccess { value, info } if info == source => {
                let existing = self.get(value, before, source);
                Some((
                    value,
                    if existing.0.is_empty() {
                        Paths::origin(source)
                    } else {
                        existing
                    },
                ))
            }
            Operation::DescribeBinding {
                value,
                name,
                captured,
            } => {
                let mut paths = self.get(value, before, source);
                if *captured || name.span != source.declaration || name.text != source.name {
                    paths.step(
                        Some(name.span.clone()),
                        format!(
                            "{}binding '{}' {} here",
                            if *captured { "captured " } else { "" },
                            name.text,
                            if *captured { "accessed" } else { "declared" }
                        ),
                    );
                }
                Some((value, paths))
            }
            Operation::Object { result, expression } => {
                let paths = match expression {
                    Rvalue::Share(value) => self.get(value, before, source),
                    Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. } => Paths::default(),
                    Rvalue::Field { owner, index } | Rvalue::VariantField { owner, index, .. } => {
                        let projection = match expression {
                            Rvalue::VariantField { variant, .. } => Projection::VariantField {
                                variant: *variant,
                                field: *index,
                            },
                            _ => Projection::Field(*index),
                        };
                        let mut paths = self.get(owner, before, source).project(&projection);
                        paths.step(
                            None,
                            format!(
                                "readonly {} retrieved",
                                projection_name(&owner.ty, &projection, context.program)
                            ),
                        );
                        paths
                    }
                    Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                        for (index, field) in fields.iter().enumerate() {
                            let projection = match expression {
                                Rvalue::Variant { index: variant, .. } => {
                                    Projection::VariantField {
                                        variant: *variant,
                                        field: index,
                                    }
                                }
                                _ => Projection::Field(index),
                            };
                            let paths = self.get(field, before, source);
                            self.store(result, projection, &paths, after, context);
                        }
                        self.values.get(&result.id).cloned().unwrap_or_default()
                    }
                };
                Some((result, paths))
            }
            Operation::Rebind {
                target,
                value,
                span,
            } => {
                let mut paths = self.get(value, before, source);
                match target {
                    Place::Local(id) => {
                        if context
                            .declarations
                            .last()
                            .is_some_and(|map| map.get(id) == Some(source))
                        {
                            paths = Paths::origin(source);
                        }
                        if let Some((name, _)) = self.names.get(id) {
                            paths.step(
                                span.clone(),
                                format!("binding '{}' assigned here", name.text),
                            );
                        }
                        self.values.insert(
                            id.clone(),
                            paths.fit(
                                &after.get(&Value {
                                    id: id.clone(),
                                    ty: value.ty.clone(),
                                }),
                                source,
                                self.heap.best().as_ref(),
                            ),
                        );
                    }
                    Place::Field { owner, index } => {
                        self.store(owner, Projection::Field(*index), &paths, after, context)
                    }
                }
                None
            }
            Operation::Call(call) => {
                let program = context.program;
                let callee = &program.functions[call.function];
                let arguments: Vec<_> = call
                    .args
                    .iter()
                    .map(|v| self.get(v, before, source))
                    .collect();
                let mut paths = Paths::default();
                for alias in &callee.effects.returned.aliases {
                    let mut actual = arguments[alias.parameter].clone();
                    let mut ty = call.args[alias.parameter].ty.clone();
                    for projection in &alias.parameter_path {
                        actual = actual.project(projection);
                        actual.step(
                            None,
                            format!(
                                "readonly {} retrieved",
                                projection_name(&ty, projection, context.program)
                            ),
                        );
                        ty = crate::effects::project_type(
                            &ty,
                            std::slice::from_ref(projection),
                            &context.program.types,
                        )
                        .unwrap_or_default();
                    }
                    paths.join(&actual.contain(&alias.result_path));
                }
                for (path, sources) in &context.summaries[call.function].returned.0 {
                    if sources.contains(source) {
                        paths.insert(path.clone(), Trail::origin(source));
                    }
                }
                let mut retained_inside = false;
                if let Some(returned) = context.returns(call.function, &arguments) {
                    for (path, trail) in returned.paths.0 {
                        if !trail.uncertain || !paths.0.contains_key(&path) {
                            paths.0.insert(path, trail);
                        }
                    }
                    let mut retained = returned.heap;
                    retained_inside = !retained.0.is_empty();
                    retained.step(
                        Some(call.span.clone()),
                        format!("readonly access retained by '{}'", callee.name),
                    );
                    self.heap.join(&retained);
                } else if callee.body.is_some() {
                    paths.truncate();
                }
                if callee.effects.returned.unknown {
                    for actual in &arguments {
                        if let Some(trail) = actual.best() {
                            paths.insert(vec![], trail);
                        }
                    }
                    paths = paths.fit(&after.get(&call.result), source, self.heap.best().as_ref());
                    paths.uncertain();
                    paths.step(Some(call.span.clone()), format!("return access of '{}' is unknown; readonly restriction propagated conservatively", callee.name));
                } else {
                    paths.step(
                        Some(call.span.clone()),
                        format!("result of '{}' propagates readonly access", callee.name),
                    );
                }
                for (index, effect) in callee.effects.parameters.iter().enumerate() {
                    if effect.retained != Effect::No && !retained_inside {
                        let mut retained = arguments[index].clone();
                        retained.step(
                            Some(call.span.clone()),
                            format!(
                                "call to '{}' may retain readonly argument {} in untracked storage",
                                callee.name,
                                index + 1
                            ),
                        );
                        retained.uncertain();
                        self.heap.join(&retained);
                    }
                }
                Some((&call.result, paths))
            }
            _ => None,
        };
        if let Some((value, paths)) = &mut produced {
            *paths = paths.fit(&after.get(value), source, self.heap.best().as_ref());
            self.values.insert(value.id.clone(), paths.clone());
        }
        if has(&after.heap, source) && !has(&before.heap, source) && self.heap.0.is_empty() {
            let mut paths = match op {
                Operation::Rebind { value, .. } => self.get(value, before, source),
                _ => Paths::origin(source),
            };
            paths.uncertain();
            paths.step(
                None,
                "readonly access may propagate through untracked heap storage".into(),
            );
            self.heap.join(&paths);
        }
    }
}

enum Goal {
    Access((usize, usize), Value),
    Returns,
    Write,
}
struct Found {
    paths: Paths,
    heap: Paths,
    site: Option<(usize, usize)>,
    value: Option<Value>,
}
struct Context<'a> {
    program: &'a lir::Program,
    summaries: &'a [Summary],
    source: &'a AccessInfo,
    budget: usize,
    active: BTreeSet<usize>,
    declarations: Vec<BTreeMap<ValueId, AccessInfo>>,
}
impl Context<'_> {
    fn spend(&mut self, cost: usize) -> bool {
        let left = self.budget.checked_sub(cost);
        self.budget = left.unwrap_or(0);
        left.is_some()
    }
    fn declaration(&self, op: &Operation, source: &AccessInfo) -> bool {
        let Operation::Declare { result, .. } = op else {
            return false;
        };
        self.declarations
            .last()
            .is_some_and(|map| map.get(&result.id) == Some(source))
    }
}
fn write_target(
    op: &Operation,
    state: &State,
    source: &AccessInfo,
    program: &lir::Program,
) -> Option<Value> {
    match op {
        Operation::Call(call) => call
            .args
            .iter()
            .zip(&program.functions[call.function].effects.parameters)
            .find(|(v, e)| {
                (e.write != Effect::No || e.structural != Effect::No) && has(&state.get(v), source)
            })
            .map(|(v, _)| v.clone()),
        Operation::Rebind {
            target: Place::Field { owner, .. },
            ..
        } if has(&state.get(owner).project_root(), source) => Some(owner.clone()),
        Operation::StoreBool { target, .. } if has(&state.get(target), source) => {
            Some(target.clone())
        }
        _ => None,
    }
}
pub(super) fn explain(
    failure: &Failure,
    body: &lir::Function,
    program: &lir::Program,
    summaries: &[Summary],
    point: (usize, usize),
) -> String {
    let mut context = Context {
        program,
        summaries,
        source: &failure.source,
        budget: 1_000_000,
        active: BTreeSet::new(),
        declarations: vec![],
    };
    let Some(found) = context.run(
        body,
        State::default(),
        Routes::new(body),
        Goal::Access(point, failure.value.clone()),
    ) else {
        return "\nreadonly propagation path: additional tracking unavailable or exceeds diagnostic limits".into();
    };
    let mut trail = found
        .paths
        .at(&failure.path)
        .or_else(|| found.paths.best())
        .unwrap_or_else(|| Trail::origin(&failure.source));
    if let Operation::Call(call) = &body.blocks[point.0].instructions[point.1]
        && let Some(index) = call
            .args
            .iter()
            .enumerate()
            .find(|(i, v)| {
                let e = &program.functions[call.function].effects.parameters[*i];
                v.id == failure.value.id && (e.write != Effect::No || e.structural != Effect::No)
            })
            .map(|(i, _)| i)
    {
        let selected = Paths([(failure.path.clone(), trail.clone())].into());
        trail = context.mutation(call, index, selected);
    }
    let heading = if failure.uncertain || trail.uncertain {
        "uncertain readonly propagation path"
    } else {
        "readonly propagation path"
    };
    format!("\n{heading}:\n{}", trail.render())
}
