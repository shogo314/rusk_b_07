//! Closed fresh scalar aliases share inline storage; element aliases borrow one
//! owning Obj. Fresh groups stay in generated scalar operations, with mutation
//! and return promotion. Owned groups stay readonly and may also be passed to
//! ordinary functions with an inferred borrowed parameter. Foreign calls,
//! member storage and rebinding keep ordinary ownership. Stable borrowed scalar
//! parameters can also back local aliases across blocks for the whole call.
use super::{FunctionInfo, intrinsics};
use crate::{
    effects::Effect,
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, ProgramFunction, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy)]
enum Source<'a> {
    Fresh,
    Element,
    Parameter(&'a BTreeSet<ValueId>),
}
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
    groups(body, functions, summaries, Source::Fresh)
}
pub(super) fn owned_plan(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
) -> BTreeMap<ValueId, ValueId> {
    groups(body, functions, summaries, Source::Element)
}
pub(super) fn parameter_plan(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
    current: &ProgramFunction,
) -> BTreeMap<ValueId, ValueId> {
    let parameters: BTreeSet<_> = current
        .params
        .iter()
        .enumerate()
        .filter(|(index, ty)| {
            matches!(ty.as_str(), "Int" | "Bool" | "Char")
                && super::borrows_parameter(current, *index)
        })
        .map(|(index, _)| ValueId::Parameter(index))
        .collect();
    if parameters.is_empty() {
        return BTreeMap::new();
    }
    groups(body, functions, summaries, Source::Parameter(&parameters))
}
fn groups(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
    source: Source<'_>,
) -> BTreeMap<ValueId, ValueId> {
    let readonly = !matches!(source, Source::Fresh);
    let mut parents = BTreeMap::new();
    let mut origins = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    // Parameter origins are caller-owned values, never fresh allocations.
    if let Source::Parameter(parameters) = source {
        for parameter in parameters {
            parents.insert(parameter.clone(), parameter.clone());
            origins.insert(parameter.clone());
        }
    }
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
                } if !readonly && matches!(result.ty.as_str(), "Bool" | "Int" | "Char") => {
                    origins.insert(result.id.clone());
                }
                Operation::Call(call)
                    if matches!(call.result.ty.as_str(), "Bool" | "Int" | "Char")
                        && (match source {
                            Source::Element => {
                                intrinsics::lower_owned(call, &functions[call.function]).is_some()
                            }
                            Source::Fresh => {
                                intrinsics::lower(call, &functions[call.function]).is_some()
                            }
                            Source::Parameter(_) => false,
                        }) =>
                {
                    origins.insert(call.result.id.clone());
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
        let roots: Vec<_> = ids.iter().filter(|id| origins.contains(*id)).collect();
        if ids.len() > 1
            && roots.len() == 1
            && ids
                .iter()
                .all(|id| origins.contains(id) || aliases.contains(id))
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
                        if (!lowered
                            && !(readonly
                                && super::borrows_parameter(&summaries[call.function], index)))
                            || (effect.write != Effect::No
                                && (readonly
                                    || functions[call.function].name != "__assign__"
                                    || index != 0
                                    || effect.write != Effect::May))
                            || effect.structural != Effect::No
                            || effect.escape != Effect::No
                        {
                            block_value(&arg.id);
                        }
                    }
                }
                Operation::StoreBool { target, .. } if readonly => block_value(&target.id),
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
        if readonly
            && let Terminator::Return {
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
