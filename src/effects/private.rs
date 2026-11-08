//! Prove a narrow class of aggregate destinations cannot leave the function.
//! All references joined by sharing or local rebinding form one may-alias group.
//! Only directly constructed aggregates seed private groups. Every potential
//! publication blocks the entire group, including publications before a store
//! and aliases created on another branch or an earlier loop iteration.
use crate::{
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn destinations(body: &Function) -> BTreeSet<ValueId> {
    let mut edges = BTreeMap::<ValueId, BTreeSet<ValueId>>::new();
    let mut fresh = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut link = |a: &ValueId, b: &ValueId| {
        edges.entry(a.clone()).or_default().insert(b.clone());
        edges.entry(b.clone()).or_default().insert(a.clone());
    };
    for block in &body.blocks {
        for op in &block.instructions {
            match op {
                Operation::Declare { result, initial } => {
                    if let Some(initial) = initial {
                        link(&result.id, &initial.id);
                    }
                }
                Operation::Object { result, expression } => match expression {
                    Rvalue::Share(value) => link(&result.id, &value.id),
                    Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                        fresh.insert(result.id.clone());
                        // Nested destinations need reachability analysis; do not
                        // assume publication into another aggregate stays local.
                        blocked.extend(fields.iter().map(|value| value.id.clone()));
                    }
                    // Extracted members do not prove private destinations.
                    // Their origins still use the conservative heap analysis.
                    Rvalue::Field { .. }
                    | Rvalue::VariantField { .. }
                    | Rvalue::BoolCopy(_)
                    | Rvalue::VariantTest { .. } => {
                        blocked.insert(result.id.clone());
                    }
                },
                Operation::Rebind { target, value, .. } => match target {
                    Place::Local(id) => link(id, &value.id),
                    Place::Field { .. } => {
                        blocked.insert(value.id.clone());
                    }
                },
                Operation::Call(call) => {
                    blocked.insert(call.result.id.clone());
                    blocked.extend(call.args.iter().map(|value| value.id.clone()));
                }
                Operation::Literal { result, .. } => {
                    blocked.insert(result.id.clone());
                }
                Operation::StoreBool { target, value } => {
                    blocked.extend([target.id.clone(), value.id.clone()]);
                }
                Operation::Release(_)
                | Operation::AnnotateBinding { .. }
                | Operation::AnnotateAccess { .. }
                | Operation::AnnotateEscape { .. } => {}
            }
        }
        match &block.terminator {
            Terminator::Return {
                value: Some(value), ..
            }
            | Terminator::Branch {
                condition: value, ..
            } => {
                blocked.insert(value.id.clone());
            }
            _ => {}
        }
    }
    let mut visited = BTreeSet::new();
    let mut private = BTreeSet::new();
    for seed in fresh {
        if visited.contains(&seed) {
            continue;
        }
        let mut group = BTreeSet::new();
        let mut pending = vec![seed];
        while let Some(id) = pending.pop() {
            if !group.insert(id.clone()) {
                continue;
            }
            if let Some(neighbors) = edges.get(&id) {
                pending.extend(neighbors.iter().cloned());
            }
        }
        visited.extend(group.iter().cloned());
        if group
            .iter()
            .all(|id| matches!(id, ValueId::Local(_)) && !blocked.contains(id))
        {
            private.extend(group);
        }
    }
    private
}
