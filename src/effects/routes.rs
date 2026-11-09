//! Diagnostic-only provenance. Keep one short, deterministic witness per alias;
//! source bindings never participate in the safety proof or generated code.
use super::{Projection, ReturnAlias, ReturnOrigins, State, Summary};
use crate::{
    ast::{Name, Span},
    hir::{Place, Rvalue, Value, ValueId},
    lir::{self, Operation},
};
use std::collections::{BTreeMap, BTreeSet};

mod calls;
pub(super) use calls::Context;

const MAX_STEPS: usize = 16;
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Step {
    span: Option<Span>,
    text: String,
    callee: Option<(usize, calls::Goal)>,
}
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Trail {
    steps: Vec<Step>,
    truncated: bool,
}
impl Trail {
    fn push(&mut self, step: Step) {
        if self.steps.len() < MAX_STEPS {
            self.steps.push(step);
        } else {
            self.truncated = true;
        }
    }
    fn better_than(&self, other: &Self) -> bool {
        (self.steps.len(), self) < (other.steps.len(), other)
    }
    fn render(&self, context: &Context<'_>) -> String {
        calls::render(self, context)
    }
}
fn join_trail(target: &mut Option<Trail>, value: &Option<Trail>) {
    if let Some(value) = value
        && target.as_ref().is_none_or(|old| value.better_than(old))
    {
        *target = Some(value.clone());
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
struct Origins {
    known: BTreeMap<ReturnAlias, Trail>,
    uncertain: Option<Trail>,
}
impl Origins {
    fn insert(&mut self, alias: ReturnAlias, trail: Trail) {
        if self
            .known
            .get(&alias)
            .is_none_or(|old| trail.better_than(old))
        {
            self.known.insert(alias, trail);
        }
    }
    fn join(&mut self, other: &Self) {
        for (alias, trail) in &other.known {
            self.insert(alias.clone(), trail.clone());
        }
        join_trail(&mut self.uncertain, &other.uncertain);
    }
    fn step(&mut self, step: Step) {
        for trail in self.known.values_mut() {
            trail.push(step.clone());
        }
        if let Some(trail) = &mut self.uncertain {
            trail.push(step);
        }
    }
    fn project(&self, projection: Projection) -> Self {
        let mut result = Self {
            uncertain: self.uncertain.clone(),
            ..Self::default()
        };
        for (alias, trail) in &self.known {
            let origins = ReturnOrigins {
                aliases: [alias.clone()].into(),
                ..ReturnOrigins::default()
            }
            .project(projection.clone());
            for alias in origins.aliases {
                result.insert(alias, trail.clone());
            }
        }
        result
    }
    fn contain(&self, projection: Projection) -> Self {
        let mut result = Self {
            uncertain: self.uncertain.clone(),
            ..Self::default()
        };
        for (alias, trail) in &self.known {
            let origins = ReturnOrigins {
                aliases: [alias.clone()].into(),
                ..ReturnOrigins::default()
            }
            .contain(projection.clone());
            for alias in origins.aliases {
                result.insert(alias, trail.clone());
            }
        }
        result
    }
}
pub(crate) fn projection_name(ty: &str, projection: &Projection, program: &lir::Program) -> String {
    match projection {
        Projection::Field(index) => {
            let name = program
                .types
                .get(ty)
                .and_then(|kind| kind.fields())
                .and_then(|fields| fields.get(*index))
                .map(|(name, _)| name.clone());
            name.map_or_else(
                || format!("field {} of '{ty}'", index + 1),
                |name| format!("field '{name}' of '{ty}'"),
            )
        }
        Projection::VariantField { variant, field } => {
            let name = match program.types.get(ty) {
                Some(crate::hir::Type::Enum(variants)) => {
                    variants.get(*variant).map(|(name, _)| name.as_str())
                }
                _ => None,
            };
            format!(
                "payload {} of variant '{}'",
                field + 1,
                name.unwrap_or("unknown")
            )
        }
        Projection::Element => format!("element of '{ty}'"),
    }
}
fn path_name(ty: &str, path: &[Projection], program: &lir::Program) -> String {
    let mut ty = ty.to_string();
    let mut parts = vec![];
    for projection in path {
        parts.push(projection_name(&ty, projection, program));
        ty = super::project_type(&ty, std::slice::from_ref(projection), &program.types)
            .unwrap_or_default();
    }
    parts.join(" -> ")
}
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Routes {
    values: BTreeMap<ValueId, Origins>,
    heap: Option<Trail>,
    names: BTreeMap<ValueId, Name>,
}
impl Routes {
    pub fn new(body: &lir::Function, parameter: usize) -> Self {
        let mut result = Self::default();
        result.values.insert(
            ValueId::Parameter(parameter),
            Origins {
                known: [(
                    ReturnAlias {
                        parameter,
                        parameter_path: vec![],
                        result_path: vec![],
                    },
                    Trail::default(),
                )]
                .into(),
                uncertain: None,
            },
        );
        for block in &body.blocks {
            for op in &block.instructions {
                if let Operation::DescribeBinding { value, name, .. } = op {
                    result.names.insert(value.id.clone(), name.clone());
                }
            }
        }
        result
    }
    fn get(&self, value: &Value) -> Origins {
        self.values.get(&value.id).cloned().unwrap_or_default()
    }
    pub fn join(&mut self, other: &Self) -> bool {
        let old = self.clone();
        for (id, origins) in &other.values {
            self.values.entry(id.clone()).or_default().join(origins);
        }
        join_trail(&mut self.heap, &other.heap);
        *self != old
    }
    pub fn describe(
        &self,
        value: &Value,
        origins: &ReturnOrigins,
        parameter: usize,
        context: &Context<'_>,
    ) -> String {
        if origins.unknown {
            if let Some(trail) = self
                .values
                .get(&value.id)
                .and_then(|value| value.uncertain.as_ref())
                .or(self.heap.as_ref())
            {
                return format!("\nuncertain sharing path:\n{}", trail.render(context));
            }
        } else if let Some(alias) = origins
            .aliases
            .iter()
            .find(|alias| alias.parameter == parameter)
            && let Some(trail) = self
                .values
                .get(&value.id)
                .and_then(|value| value.known.get(alias))
            && !trail.steps.is_empty()
        {
            return format!("\nsharing path:\n{}", trail.render(context));
        }
        String::new()
    }
    pub fn retained(
        &self,
        value: &Value,
        origins: &ReturnOrigins,
        parameter: usize,
        function: usize,
        argument: usize,
        context: &Context<'_>,
    ) -> String {
        let mut trail = self.trail(value, origins, parameter).unwrap_or_default();
        trail.push(Step {
            span: None,
            text: format!(
                "retention through argument {} of '{}'",
                argument + 1,
                context.program.functions[function].name
            ),
            callee: Some((function, calls::Goal::Retained(argument))),
        });
        let label = if origins.unknown {
            "uncertain sharing path"
        } else {
            "sharing path"
        };
        format!("\n{label}:\n{}", trail.render(context))
    }
    fn trail(&self, value: &Value, origins: &ReturnOrigins, parameter: usize) -> Option<Trail> {
        if origins.unknown {
            self.values
                .get(&value.id)
                .and_then(|v| v.uncertain.clone())
                .or(self.heap.clone())
        } else {
            origins
                .aliases
                .iter()
                .filter(|a| a.parameter == parameter)
                .find_map(|a| self.values.get(&value.id)?.known.get(a).cloned())
        }
    }
    pub fn operation(
        &mut self,
        op: &Operation,
        before: &State,
        after: &State,
        callees: &[Summary],
        program: &lir::Program,
    ) {
        let step = |span: Option<Span>, text: String| Step {
            span,
            text,
            callee: None,
        };
        let mut produced = match op {
            Operation::DescribeBinding {
                value,
                name,
                captured,
            } => {
                let mut origins = self.get(value);
                origins.step(step(
                    Some(name.span.clone()),
                    if *captured {
                        format!("captured binding '{}' accessed here", name.text)
                    } else {
                        format!("binding '{}' declared here", name.text)
                    },
                ));
                Some((value, origins))
            }
            Operation::Declare { result, initial } => Some((
                result,
                initial
                    .as_ref()
                    .map(|value| self.get(value))
                    .unwrap_or_default(),
            )),
            Operation::Literal { result, .. } => Some((result, Origins::default())),
            Operation::Rebind {
                target: Place::Local(id),
                value,
                span,
            } => {
                let mut origins = self.get(value);
                if let Some(name) = self.names.get(id) {
                    origins.step(step(
                        span.clone(),
                        format!("binding '{}' assigned here", name.text),
                    ));
                } else if span.is_some() {
                    origins.step(step(span.clone(), "local alias assigned here".into()));
                }
                if let Some(actual) = after.values.get(id) {
                    origins
                        .known
                        .retain(|alias, _| actual.aliases.contains(alias));
                    if actual.unknown && origins.uncertain.is_none() {
                        let mut trail = self.heap.clone().unwrap_or_default();
                        trail.push(step(
                            span.clone(),
                            "sharing of the assigned value is unknown or exceeds analysis limits"
                                .into(),
                        ));
                        origins.uncertain = Some(trail);
                    }
                }
                self.values.insert(id.clone(), origins);
                None
            }
            Operation::Object { result, expression } => {
                let origins = match expression {
                    Rvalue::Share(value) => self.get(value),
                    Rvalue::Field { owner, index } | Rvalue::VariantField { owner, index, .. } => {
                        let projection = match expression {
                            Rvalue::VariantField { variant, .. } => Projection::VariantField {
                                variant: *variant,
                                field: *index,
                            },
                            _ => Projection::Field(*index),
                        };
                        let mut value = self.get(owner).project(projection.clone());
                        if after.get(result).unknown && value.uncertain.is_none() {
                            value.uncertain = Some(self.heap.clone().unwrap_or_default());
                        }
                        value.step(step(
                            None,
                            format!(
                                "shared {} retrieved",
                                projection_name(&owner.ty, &projection, program)
                            ),
                        ));
                        value
                    }
                    Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                        let mut value = Origins::default();
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
                            let mut actual = self.get(field).contain(projection.clone());
                            actual.step(step(
                                None,
                                format!(
                                    "shared value stored in {}",
                                    projection_name(&result.ty, &projection, program)
                                ),
                            ));
                            value.join(&actual);
                        }
                        value
                    }
                    Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. } => Origins::default(),
                };
                Some((result, origins))
            }
            Operation::Call(call) => {
                let callee = &callees[call.function];
                let mut value = Origins::default();
                for alias in &callee.returned.aliases {
                    let mut actual = self.get(&call.args[alias.parameter]);
                    for projection in &alias.parameter_path {
                        actual = actual.project(projection.clone());
                    }
                    for projection in alias.result_path.iter().rev() {
                        actual = actual.contain(projection.clone());
                    }
                    let mut text = format!(
                        "result of '{}' may share argument {}",
                        program.functions[call.function].name,
                        alias.parameter + 1
                    );
                    if !alias.parameter_path.is_empty() {
                        text += &format!(
                            " through {}",
                            path_name(
                                &call.args[alias.parameter].ty,
                                &alias.parameter_path,
                                program
                            )
                        );
                    }
                    if !alias.result_path.is_empty() {
                        text += &format!(
                            "; contained at {}",
                            path_name(&call.result.ty, &alias.result_path, program)
                        );
                    }
                    actual.step(Step {
                        span: Some(call.span.clone()),
                        text,
                        callee: Some((call.function, calls::Goal::Returned(alias.clone()))),
                    });
                    value.join(&actual);
                }
                if after.get(&call.result).unknown {
                    let mut trail = if before.heap_unknown {
                        self.heap.clone().unwrap_or_default()
                    } else {
                        Trail::default()
                    };
                    trail.push(step(
                        Some(call.span.clone()),
                        format!(
                            "return sharing of '{}' is unknown or exceeds analysis limits",
                            program.functions[call.function].name
                        ),
                    ));
                    value.uncertain = Some(trail);
                }
                Some((&call.result, value))
            }
            _ => None,
        };
        if let Some((value, origins)) = &mut produced {
            let actual = after.get(value);
            origins
                .known
                .retain(|alias, _| actual.aliases.contains(alias));
            if actual.unknown && origins.uncertain.is_none() {
                let mut trail = self.heap.clone().unwrap_or_default();
                trail.push(step(
                    None,
                    "sharing through this value is unknown or exceeds analysis limits".into(),
                ));
                origins.uncertain = Some(trail);
            }
            self.values.insert(value.id.clone(), origins.clone());
        }
        if !before.heap_unknown && after.heap_unknown {
            let invalidation = match op {
                Operation::Rebind { span, .. } => step(
                    span.clone(),
                    "heap sharing may be invalidated by this field store".into(),
                ),
                Operation::Call(call) => step(
                    Some(call.span.clone()),
                    format!(
                        "heap sharing may be invalidated by call to '{}'",
                        program.functions[call.function].name
                    ),
                ),
                _ => step(None, "heap sharing may be invalidated here".into()),
            };
            self.heap = Some(Trail {
                steps: vec![invalidation],
                truncated: false,
            });
        }
    }
}
