//! Borrow aliases of an owning element within one block. The owner is reset at
//! the final original release, preserving object and character-reference life.
use super::{FunctionInfo, scalar_aliases, storage};
use crate::{
    hir::ValueId,
    lir::{Function, Operation, ProgramFunction, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Plan {
    pub aliases: BTreeMap<ValueId, ValueId>,
    pub releases: BTreeMap<ValueId, Option<ValueId>>,
}
pub(super) fn plan(
    body: &Function,
    functions: &[FunctionInfo],
    summaries: &[ProgramFunction],
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
    for (owner, members) in groups {
        if members.iter().any(|id| {
            persistent.contains(id)
                || definitions.get(id) != Some(&1)
                || sites.get(id).is_none_or(|sites| sites.len() != 1)
        }) {
            continue;
        }
        let blocks: BTreeSet<_> = members.iter().map(|id| sites[id][0].0).collect();
        if blocks.len() != 1 {
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
    result
}
