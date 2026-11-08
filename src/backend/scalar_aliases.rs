//! Closed groups of fresh scalar references can share inline storage. Every use
//! must stay in generated scalar operations; foreign calls, returns, member
//! storage and rebinding keep ordinary ownership and promotion semantics.
use super::{FunctionInfo, intrinsics};
use crate::{
    effects::Effect,
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, ProgramFunction, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};
fn root(parents: &BTreeMap<ValueId, ValueId>, id: &ValueId) -> ValueId {
    let mut id = id.clone();
    while let Some(parent) = parents.get(&id) {
        if *parent == id {
            break;
        }
        id = parent.clone();
    }
    id
}
pub(super) fn plan(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
) -> BTreeMap<ValueId, ValueId> {
    groups(body, functions, summaries, false)
}
pub(super) fn owned_plan(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
) -> BTreeMap<ValueId, ValueId> {
    groups(body, functions, summaries, true)
}
fn groups(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
    owned: bool,
) -> BTreeMap<ValueId, ValueId> {
    let mut parents = BTreeMap::new();
    let mut fresh = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for slot in &body.slots {
        if matches!(slot.ty.as_str(), "Bool" | "Int" | "Char") {
            parents.insert(slot.id.clone(), slot.id.clone());
        }
    }
    for block in &body.blocks {
        for op in &block.instructions {
            let pair = match op {
                Operation::Declare {
                    result,
                    initial: Some(value),
                } if matches!(result.ty.as_str(), "Bool" | "Int" | "Char") => Some((result, value)),
                Operation::Object {
                    result,
                    expression: Rvalue::Share(value),
                } if matches!(result.ty.as_str(), "Bool" | "Int" | "Char") => Some((result, value)),
                _ => None,
            };
            if let Some((result, value)) = pair {
                let a = root(&parents, &result.id);
                let b = root(&parents, &value.id);
                parents.entry(a.clone()).or_insert(a.clone());
                parents.entry(b.clone()).or_insert(b.clone());
                if a != b {
                    let (first, last) = if a < b { (a, b) } else { (b, a) };
                    parents.insert(last, first);
                }
                aliases.insert(result.id.clone());
            }
            match op {
                Operation::Literal { result, .. }
                | Operation::Object {
                    result,
                    expression: Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. },
                } if !owned && matches!(result.ty.as_str(), "Bool" | "Int" | "Char") => {
                    fresh.insert(result.id.clone());
                }
                Operation::Call(call)
                    if matches!(call.result.ty.as_str(), "Bool" | "Int" | "Char")
                        && (if owned {
                            intrinsics::lower_owned(call, &functions[call.function]).is_some()
                        } else {
                            intrinsics::lower(call, &functions[call.function]).is_some()
                        }) =>
                {
                    fresh.insert(call.result.id.clone());
                }
                _ => {}
            }
        }
    }
    let mut members = BTreeMap::<ValueId, Vec<ValueId>>::new();
    for id in parents.keys() {
        members
            .entry(root(&parents, id))
            .or_default()
            .push(id.clone());
    }
    let mut owners = BTreeMap::new();
    for (group, ids) in &members {
        let roots: Vec<_> = ids.iter().filter(|id| fresh.contains(*id)).collect();
        if ids.len() > 1
            && roots.len() == 1
            && ids
                .iter()
                .all(|id| fresh.contains(id) || aliases.contains(id))
        {
            owners.insert(group.clone(), roots[0].clone());
        }
    }
    let mut blocked = BTreeSet::new();
    let mut block_value = |id: &ValueId| {
        if parents.contains_key(id) {
            blocked.insert(root(&parents, id));
        }
    };
    for block in &body.blocks {
        for op in &block.instructions {
            match op {
                Operation::Call(call) => {
                    let lowered = intrinsics::lower(call, &functions[call.function]).is_some();
                    for (index, arg) in call.args.iter().enumerate() {
                        let effect = &summaries[call.function].effects.parameters[index];
                        if !lowered
                            || effect.write != Effect::No
                            || effect.structural != Effect::No
                            || effect.escape != Effect::No
                        {
                            block_value(&arg.id);
                        }
                    }
                }
                Operation::StoreBool { target, .. } if owned => block_value(&target.id),
                Operation::Rebind { target, value, .. } => {
                    block_value(&value.id);
                    if let Place::Local(id) = target {
                        block_value(id);
                    }
                }
                Operation::Object {
                    expression: Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. },
                    ..
                } => {
                    for field in fields {
                        block_value(&field.id);
                    }
                }
                _ => {}
            }
        }
        if let Terminator::Return {
            value: Some(value), ..
        } = &block.terminator
        {
            block_value(&value.id);
        }
    }
    parents
        .keys()
        .filter_map(|id| {
            let group = root(&parents, id);
            if blocked.contains(&group) {
                None
            } else {
                owners.get(&group).map(|owner| (id.clone(), owner.clone()))
            }
        })
        .collect()
}
