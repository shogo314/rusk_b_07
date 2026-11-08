//! Flow-insensitive allocation-site reachability for private aggregate stores.
//! Sharing and rebinding union possible destinations; member stores are weak
//! updates. Publication reaches every possible child, including older values
//! from another branch or loop iteration. Calls remain a conservative boundary.
use super::Projection;
use crate::{
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default, PartialEq, Eq)]
struct Reference {
    objects: BTreeSet<ValueId>,
    unknown: bool,
}
impl Reference {
    fn fresh(site: ValueId) -> Self {
        Self {
            objects: [site].into(),
            unknown: false,
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
    members: BTreeMap<ValueId, BTreeMap<Projection, Reference>>,
    exposed: BTreeSet<ValueId>,
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
    fn operation(&mut self, op: &Operation) {
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
                for arg in &call.args {
                    self.expose(&self.get(&arg.id));
                }
                self.bind(&call.result.id, &Reference::unknown());
            }
            Operation::Literal { result, .. } => {
                self.bind(&result.id, &Reference::unknown());
            }
            Operation::StoreBool { .. }
            | Operation::Release(_)
            | Operation::AnnotateBinding { .. }
            | Operation::AnnotateAccess { .. }
            | Operation::AnnotateEscape { .. } => {}
        }
    }
}

pub(super) fn destinations(body: &Function) -> BTreeSet<ValueId> {
    let mut graph = Graph::default();
    // Sites and member keys come only from finite LIR instructions. Weak
    // updates grow sets monotonically, so cycles need no unbounded paths.
    loop {
        let before = graph.clone();
        for block in &body.blocks {
            for op in &block.instructions {
                graph.operation(op);
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
