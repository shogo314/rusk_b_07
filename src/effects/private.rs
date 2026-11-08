//! Flow-insensitive allocation-site reachability for private aggregate stores.
//! Sharing and rebinding union possible destinations; member stores are weak
//! updates. Publication reaches every possible child, including older values
//! from another branch or loop iteration. Known nonretaining calls forward
//! aliases and track references contained in their results.
use super::{Effect, Projection, Summary, SummarySource};
use crate::{
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Object {
    site: ValueId,
    // A path locates a return-summary node. Even its root has unknown
    // identity; only fresh LIR constructions prove private allocations.
    path: Vec<Projection>,
}
#[derive(Clone, Default, PartialEq, Eq)]
struct Reference {
    objects: BTreeSet<Object>,
    unknown: bool,
}
impl Reference {
    fn fresh(site: ValueId) -> Self {
        Self {
            objects: [Object { site, path: vec![] }].into(),
            unknown: false,
        }
    }
    fn contained(site: ValueId, path: Vec<Projection>) -> Self {
        Self {
            objects: [Object { site, path }].into(),
            unknown: true,
        }
    }
    fn unknown() -> Self {
        Self {
            unknown: true,
            ..Self::default()
        }
    }
    fn join(&mut self, other: &Self) {
        self.objects.extend(other.objects.iter().cloned());
        self.unknown |= other.unknown;
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
struct Graph {
    values: BTreeMap<ValueId, Reference>,
    members: BTreeMap<Object, BTreeMap<Projection, Reference>>,
    exposed: BTreeSet<Object>,
}
impl Graph {
    fn get(&self, id: &ValueId) -> Reference {
        let mut value = self.values.get(id).cloned().unwrap_or_default();
        value.unknown |= matches!(id, ValueId::Parameter(_));
        value
    }
    fn bind(&mut self, id: &ValueId, value: &Reference) {
        self.values.entry(id.clone()).or_default().join(value);
    }
    fn expose(&mut self, value: &Reference) {
        let mut pending: Vec<_> = value.objects.iter().cloned().collect();
        let mut visited = BTreeSet::new();
        while let Some(site) = pending.pop() {
            if !visited.insert(site.clone()) {
                continue;
            }
            self.exposed.insert(site.clone());
            if let Some(fields) = self.members.get(&site) {
                for value in fields.values() {
                    pending.extend(value.objects.iter().cloned());
                }
            }
        }
    }
    fn store(&mut self, owner: &Reference, projection: Projection, value: &Reference) {
        for site in &owner.objects {
            self.members
                .entry(site.clone())
                .or_default()
                .entry(projection.clone())
                .or_default()
                .join(value);
        }
        if owner.unknown || owner.objects.iter().any(|site| self.exposed.contains(site)) {
            self.expose(value);
        }
    }
    /// Materialize may-reachability from a returned aggregate. These summary
    /// nodes always have unknown identity; they never prove private storage.
    /// Unlike a program store into an unknown destination, describing a result
    /// does not itself publish its contained references.
    fn contain(&mut self, site: &ValueId, path: &[Projection], value: &Reference) -> Reference {
        let root = Reference::contained(site.clone(), vec![]);
        let mut object = Object {
            site: site.clone(),
            path: vec![],
        };
        for (index, projection) in path.iter().enumerate() {
            let field = if index + 1 == path.len() {
                value.clone()
            } else {
                let mut prefix = object.path.clone();
                prefix.push(projection.clone());
                Reference::contained(site.clone(), prefix)
            };
            self.members
                .entry(object.clone())
                .or_default()
                .entry(projection.clone())
                .or_default()
                .join(&field);
            object.path.push(projection.clone());
        }
        root
    }
    fn project(&self, owner: &Reference, projection: Projection) -> Reference {
        let mut value = Reference {
            unknown: owner.unknown,
            ..Reference::default()
        };
        for site in &owner.objects {
            // A published object may have been changed by untracked code.
            value.unknown |= self.exposed.contains(site);
            if let Some(field) = self
                .members
                .get(site)
                .and_then(|fields| fields.get(&projection))
            {
                value.join(field);
            }
        }
        value
    }
    fn operation(&mut self, op: &Operation, callees: Option<&[Summary]>) {
        match op {
            Operation::Declare { result, initial } => {
                if let Some(initial) = initial {
                    self.bind(&result.id, &self.get(&initial.id));
                }
            }
            Operation::Object { result, expression } => {
                let value = match expression {
                    Rvalue::Share(value) => self.get(&value.id),
                    Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                        let owner = Reference::fresh(result.id.clone());
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
                            self.store(&owner, projection, &self.get(&field.id));
                        }
                        owner
                    }
                    Rvalue::Field { owner, index } => {
                        self.project(&self.get(&owner.id), Projection::Field(*index))
                    }
                    Rvalue::VariantField {
                        owner,
                        variant,
                        index,
                    } => self.project(
                        &self.get(&owner.id),
                        Projection::VariantField {
                            variant: *variant,
                            field: *index,
                        },
                    ),
                    Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. } => Reference::unknown(),
                };
                self.bind(&result.id, &value);
            }
            Operation::Rebind { target, value, .. } => {
                let value = self.get(&value.id);
                match target {
                    Place::Local(id) => self.bind(id, &value),
                    Place::Field { owner, index } => {
                        self.store(&self.get(&owner.id), Projection::Field(*index), &value)
                    }
                }
            }
            Operation::Call(call) => {
                let args: Vec<_> = call.args.iter().map(|arg| self.get(&arg.id)).collect();
                let known = callees
                    .map(|callees| &callees[call.function])
                    .filter(|callee| {
                        matches!(
                            callee.source,
                            SummarySource::Inferred | SummarySource::NativeContract
                        ) && callee.external_write == Effect::No
                            && !callee.returned.unknown
                    });
                let value = if let Some(callee) = known {
                    for (arg, effect) in args.iter().zip(&callee.parameters) {
                        if effect.write != Effect::No
                            || effect.structural != Effect::No
                            || effect.retained != Effect::No
                        {
                            self.expose(arg);
                        }
                    }
                    // Fresh is not uniqueness. Do not create private allocation
                    // sites from a summary of a callee's independent results.
                    let mut value = if callee.returned.fresh || callee.returned.aliases.is_empty() {
                        Reference::unknown()
                    } else {
                        Reference::default()
                    };
                    for alias in &callee.returned.aliases {
                        let mut actual = args[alias.parameter].clone();
                        for projection in &alias.parameter_path {
                            actual = self.project(&actual, projection.clone());
                        }
                        if alias.result_path.is_empty() {
                            value.join(&actual);
                        } else {
                            let contained =
                                self.contain(&call.result.id, &alias.result_path, &actual);
                            value.join(&contained);
                        }
                    }
                    value
                } else {
                    // Unknown results may hide aliases. Publish arguments
                    // rather than lose their reachability.
                    for arg in &args {
                        self.expose(arg);
                    }
                    Reference::unknown()
                };
                self.bind(&call.result.id, &value);
            }
            Operation::Literal { result, .. } => {
                self.bind(&result.id, &Reference::unknown());
            }
            Operation::StoreBool { .. }
            | Operation::Release(_)
            | Operation::DescribeBinding { .. }
            | Operation::AnnotateBinding { .. }
            | Operation::AnnotateAccess { .. }
            | Operation::AnnotateEscape { .. } => {}
        }
    }
}

pub(super) fn destinations(body: &Function, callees: Option<&[Summary]>) -> BTreeSet<ValueId> {
    let mut graph = Graph::default();
    // Sites come from finite LIR instructions; summary paths are bounded by
    // effect validation. Weak updates never expand those paths through cycles.
    loop {
        let before = graph.clone();
        for block in &body.blocks {
            for op in &block.instructions {
                graph.operation(op, callees);
            }
            if let Terminator::Return {
                value: Some(value), ..
            } = &block.terminator
            {
                graph.expose(&graph.get(&value.id));
            }
        }
        // Revisit exposed objects even if publication precedes a new edge.
        graph.expose(&Reference {
            objects: graph.exposed.clone(),
            unknown: false,
        });
        if graph == before {
            break;
        }
    }
    graph
        .values
        .iter()
        .filter_map(|(id, value)| {
            (matches!(id, ValueId::Local(_))
                && !value.unknown
                && !value.objects.is_empty()
                && value
                    .objects
                    .iter()
                    .all(|site| !graph.exposed.contains(site)))
            .then_some(id.clone())
        })
        .collect()
}
