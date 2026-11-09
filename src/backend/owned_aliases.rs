//! Keep one element owner until the final logical alias release. Exact CFG
//! states allow aliases across blocks; stable slot borrowing stays block local.
mod flow;
use super::{FunctionInfo, intrinsics, scalar_aliases, storage};
use crate::{
    hir::{Rvalue, ValueId},
    lir::{Function, Operation, ProgramFunction, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Plan {
    pub aliases: BTreeMap<ValueId, ValueId>,
    pub releases: BTreeMap<ValueId, Option<ValueId>>,
    pub elements: BTreeSet<ValueId>,
    pub across_blocks: BTreeSet<ValueId>,
    pub release_sites: BTreeMap<(usize, usize, usize), ValueId>,
}
pub(super) fn plan(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
    inline_aliases: &BTreeMap<ValueId, ValueId>,
) -> Plan {
    let candidates = scalar_aliases::owned_plan(body, functions, summaries);
    let persistent = storage::persistent(body);
    let mut definitions = BTreeMap::<ValueId, usize>::new();
    let mut sites = BTreeMap::<ValueId, Vec<(usize, usize)>>::new();
    for (block_id, block) in body.blocks.iter().enumerate() {
        let mut position = 0;
        for op in &block.instructions {
            let value = match op {
                Operation::Declare { result, .. }
                | Operation::Object { result, .. }
                | Operation::Literal { result, .. } => Some(result),
                Operation::Call(call) => Some(&call.result),
                _ => None,
            };
            if let Some(value) = value {
                *definitions.entry(value.id.clone()).or_default() += 1;
            }
            if let Operation::Release(ids) = op {
                for id in ids {
                    sites
                        .entry(id.clone())
                        .or_default()
                        .push((block_id, position));
                    position += 1;
                }
            }
        }
        if let Terminator::Return { cleanup, .. } = &block.terminator {
            for id in cleanup {
                sites
                    .entry(id.clone())
                    .or_default()
                    .push((block_id, position));
                position += 1;
            }
        }
    }
    let mut groups = BTreeMap::<ValueId, BTreeSet<ValueId>>::new();
    for (id, owner) in candidates {
        groups.entry(owner).or_default().insert(id);
    }
    let mut result = Plan::default();
    // Exhaustion only disables further CFG borrowing, never an ownership check.
    let mut budget = 1_000_000;
    for (owner, members) in groups {
        if members.iter().any(|id| definitions.get(id) != Some(&1)) {
            continue;
        }
        let local = members.iter().all(|id| {
            !persistent.contains(id) && sites.get(id).is_some_and(|sites| sites.len() == 1)
        });
        let blocks: BTreeSet<_> = members
            .iter()
            .flat_map(|id| sites.get(id).into_iter().flatten().map(|site| site.0))
            .collect();
        if !local || blocks.len() != 1 {
            if let Some(resets) = flow::releases(body, &members, &owner, &mut budget) {
                result.across_blocks.insert(owner.clone());
                for site in resets {
                    result.release_sites.insert(site, owner.clone());
                }
                for id in members {
                    result.aliases.insert(id, owner.clone());
                }
            }
            continue;
        }
        let last = members
            .iter()
            .max_by_key(|id| sites[*id][0].1)
            .unwrap()
            .clone();
        for id in members {
            result.releases.insert(
                id.clone(),
                if id == last {
                    Some(owner.clone())
                } else {
                    None
                },
            );
            result.aliases.insert(id, owner.clone());
        }
    }
    result.elements = borrow_elements(body, functions, inline_aliases, &result);
    result
}

/// A slot reference needs both its container and the slot itself to survive.
/// The closed alias plan already confines all uses and releases to one block.
/// Scan each interval once, rejecting source release, rebinding and every call
/// without a trusted direct lowering. Unknown ownership operations also end
/// borrowing: even a scalar Obj from foreign code may have a custom deleter.
/// Scalar writes only change the pointee and remain observable.
fn borrow_elements(
    body: &Function,
    functions: &[FunctionInfo],
    inline_aliases: &BTreeMap<ValueId, ValueId>,
    plan: &Plan,
) -> BTreeSet<ValueId> {
    let mut safe_values: BTreeSet<_> = inline_aliases.keys().cloned().collect();
    for block in &body.blocks {
        for op in &block.instructions {
            let fresh = match op {
                Operation::Literal { result, .. }
                | Operation::Object {
                    result,
                    expression: Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. },
                } => Some(result),
                Operation::Call(call)
                    if call.result.ty == "Unit"
                        || intrinsics::lower(call, &functions[call.function]).is_some() =>
                {
                    Some(&call.result)
                }
                _ => None,
            };
            if let Some(value) = fresh {
                safe_values.insert(value.id.clone());
            }
        }
    }
    let mut elements = BTreeSet::new();
    for block in &body.blocks {
        let mut active = BTreeMap::<ValueId, ValueId>::new();
        let mut release = |ids: &[ValueId], active: &mut BTreeMap<ValueId, ValueId>| {
            for id in ids {
                active.retain(|_, source| source != id);
                let borrowed = plan
                    .aliases
                    .get(id)
                    .is_some_and(|owner| active.contains_key(owner));
                if !safe_values.contains(id) && !borrowed {
                    active.clear();
                }
                if let Some(Some(owner)) = plan.releases.get(id)
                    && active.remove(owner).is_some()
                {
                    elements.insert(owner.clone());
                }
            }
        };
        for op in &block.instructions {
            match op {
                Operation::Call(call) => {
                    let info = &functions[call.function];
                    let element = intrinsics::lower_owned(call, info).is_some();
                    let candidate =
                        element && plan.aliases.get(&call.result.id) == Some(&call.result.id);
                    if !(candidate || safe_values.contains(&call.result.id))
                        || (!element && intrinsics::lower(call, info).is_none())
                    {
                        active.clear();
                    }
                    if candidate {
                        active.insert(call.result.id.clone(), call.args[0].id.clone());
                    }
                }
                Operation::Object { result, .. } | Operation::Declare { result, .. }
                    if !safe_values.contains(&result.id)
                        && !plan
                            .aliases
                            .get(&result.id)
                            .is_some_and(|owner| active.contains_key(owner)) =>
                {
                    active.clear();
                }
                Operation::Rebind { .. } => active.clear(),
                Operation::Release(ids) => release(ids, &mut active),
                _ => {}
            }
        }
        if let Terminator::Return { cleanup, .. } = &block.terminator {
            release(cleanup, &mut active);
        }
    }
    elements
}
