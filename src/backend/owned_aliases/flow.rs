//! Exact logical ownership at CFG edges. Reject incompatible incoming states;
//! every accepted root redefinition starts with no surviving old aliases.
use crate::{
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, Terminator},
};
use std::collections::{BTreeSet, VecDeque};
#[cfg(test)]
mod tests;
type Site = (usize, usize, usize);

fn spend(budget: &mut usize, cost: usize) -> Option<()> {
    let remaining = budget.checked_sub(cost);
    *budget = remaining.unwrap_or(0);
    remaining.map(|_| ())
}

fn inputs(op: &Operation) -> Vec<&ValueId> {
    match op {
        Operation::Object { expression, .. } => match expression {
            Rvalue::Share(v) | Rvalue::BoolCopy(v) => vec![&v.id],
            Rvalue::Field { owner, .. }
            | Rvalue::VariantTest { owner, .. }
            | Rvalue::VariantField { owner, .. } => vec![&owner.id],
            Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                fields.iter().map(|v| &v.id).collect()
            }
        },
        Operation::Call(call) => call.args.iter().map(|v| &v.id).collect(),
        Operation::Declare { initial, .. } => initial.iter().map(|v| &v.id).collect(),
        Operation::Rebind { target, value, .. } => vec![
            match target {
                Place::Local(id) => id,
                Place::Field { owner, .. } => &owner.id,
            },
            &value.id,
        ],
        Operation::StoreBool { target, value } => vec![&target.id, &value.id],
        _ => vec![],
    }
}

fn release(
    ids: &[ValueId],
    location: (usize, usize),
    members: &BTreeSet<ValueId>,
    live: &mut BTreeSet<ValueId>,
    resets: &mut BTreeSet<Site>,
    budget: &mut usize,
) -> Option<()> {
    for (index, id) in ids.iter().enumerate() {
        spend(budget, 1)?;
        if members.contains(id) && live.remove(id) && live.is_empty() {
            resets.insert((location.0, location.1, index));
        }
    }
    Some(())
}

pub(super) fn releases(
    body: &Function,
    members: &BTreeSet<ValueId>,
    owner: &ValueId,
    budget: &mut usize,
) -> Option<BTreeSet<Site>> {
    spend(budget, body.blocks.len())?;
    let mut incoming = vec![None; body.blocks.len()];
    *incoming.first_mut()? = Some(BTreeSet::new());
    let mut work = VecDeque::from([0]);
    let mut resets = BTreeSet::new();
    while let Some(block_id) = work.pop_front() {
        spend(budget, 1)?;
        let state = incoming[block_id].as_ref()?;
        spend(budget, state.len())?;
        let mut live = state.clone();
        let block = &body.blocks[block_id];
        for (position, op) in block.instructions.iter().enumerate() {
            spend(budget, 1)?;
            let inputs = inputs(op);
            spend(budget, inputs.len())?;
            if inputs
                .iter()
                .any(|id| members.contains(*id) && !live.contains(*id))
            {
                return None;
            }
            let result = match op {
                Operation::Declare { result, .. }
                | Operation::Object { result, .. }
                | Operation::Literal { result, .. } => Some(&result.id),
                Operation::Call(call) => Some(&call.result.id),
                _ => None,
            };
            if let Some(id) = result.filter(|id| members.contains(*id)) {
                if (id == owner && !live.is_empty()) || live.contains(id) {
                    return None;
                }
                // The candidate analysis guarantees one element-producing root
                // and only Declare/Share definitions for its other members.
                live.insert(id.clone());
            }
            if let Operation::Release(ids) = op {
                release(
                    ids,
                    (block_id, position),
                    members,
                    &mut live,
                    &mut resets,
                    budget,
                )?;
            }
        }
        let successors: Vec<_> = match &block.terminator {
            Terminator::Jump(target) => vec![target.0],
            Terminator::Branch { condition, yes, no } => {
                if members.contains(&condition.id) && !live.contains(&condition.id) {
                    return None;
                }
                vec![yes.0, no.0]
            }
            Terminator::Return { value, cleanup, .. } => {
                if value.as_ref().is_some_and(|v| members.contains(&v.id)) {
                    return None;
                }
                release(
                    cleanup,
                    (block_id, block.instructions.len()),
                    members,
                    &mut live,
                    &mut resets,
                    budget,
                )?;
                if !live.is_empty() {
                    return None;
                }
                vec![]
            }
            Terminator::Unreachable => vec![],
        };
        for successor in successors {
            spend(budget, live.len())?;
            match &incoming[successor] {
                Some(previous) if *previous != live => return None,
                Some(_) => {}
                None => {
                    incoming[successor] = Some(live.clone());
                    work.push_back(successor);
                }
            }
        }
    }
    Some(resets)
}
