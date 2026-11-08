//! Forward ownership-state analysis. A release of a definitely empty local is
//! redundant; joins retain every local that may still own a reference.
use crate::{
    hir::ValueId,
    lir::{BlockId, Function, Operation, Terminator},
};
use std::collections::{BTreeSet, VecDeque};
fn defined(operation: &Operation) -> Option<ValueId> {
    match operation {
        Operation::Object { result, .. }
        | Operation::Literal { result, .. }
        | Operation::Declare {
            result,
            initial: Some(_),
        } => Some(result.id.clone()),
        Operation::Call(call) => Some(call.result.id.clone()),
        Operation::Rebind {
            target: crate::hir::Place::Local(id),
            ..
        } => Some(id.clone()),
        _ => None,
    }
}
fn transfer(operation: &Operation, live: &mut BTreeSet<ValueId>) {
    if let Some(id) = defined(operation) {
        live.insert(id);
    }
    if let Operation::Release(ids) = operation {
        for id in ids {
            live.remove(id);
        }
    }
}
pub(crate) fn simplify(function: &mut Function) {
    let mut incoming = vec![None::<BTreeSet<ValueId>>; function.blocks.len()];
    incoming[0] = Some(BTreeSet::new());
    let mut pending = VecDeque::from([BlockId(0)]);
    while let Some(id) = pending.pop_front() {
        let block = &function.blocks[id.0];
        let mut live = incoming[id.0].clone().unwrap();
        for op in &block.instructions {
            transfer(op, &mut live);
        }
        let successors = match block.terminator {
            Terminator::Jump(id) => vec![id],
            Terminator::Branch { yes, no, .. } => vec![yes, no],
            _ => vec![],
        };
        for next in successors {
            let before = incoming[next.0].clone();
            let state = incoming[next.0].get_or_insert_with(BTreeSet::new);
            state.extend(live.iter().cloned());
            if before.as_ref() != Some(state) {
                pending.push_back(next);
            }
        }
    }
    for (block, state) in function.blocks.iter_mut().zip(incoming) {
        let Some(mut live) = state else { continue };
        for op in &mut block.instructions {
            if let Operation::Release(ids) = op {
                ids.retain(|id| live.contains(id));
            }
            transfer(op, &mut live);
        }
        block
            .instructions
            .retain(|op| !matches!(op,Operation::Release(ids) if ids.is_empty()));
        if let Terminator::Return { cleanup, .. } = &mut block.terminator {
            cleanup.retain(|id| live.contains(id));
        }
    }
    // Disconnected code after a terminating statement cannot own live values.
    let mut reachable = BTreeSet::new();
    let mut work = vec![BlockId(0)];
    while let Some(id) = work.pop() {
        if !reachable.insert(id.0) {
            continue;
        }
        match function.blocks[id.0].terminator {
            Terminator::Jump(id) => work.push(id),
            Terminator::Branch { yes, no, .. } => {
                work.push(yes);
                work.push(no);
            }
            _ => {}
        }
    }
    let remap = reachable
        .iter()
        .enumerate()
        .map(|(new, old)| (*old, BlockId(new)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut index = 0;
    function.blocks.retain(|_| {
        let keep = reachable.contains(&index);
        index += 1;
        keep
    });
    for block in &mut function.blocks {
        match &mut block.terminator {
            Terminator::Jump(id) => *id = remap[&id.0],
            Terminator::Branch { yes, no, .. } => {
                *yes = remap[&yes.0];
                *no = remap[&no.0];
            }
            _ => {}
        }
    }
    let definitions = function
        .blocks
        .iter()
        .flat_map(|b| b.instructions.iter())
        .filter_map(|op| match op {
            Operation::Object { result, .. }
            | Operation::Literal { result, .. }
            | Operation::Declare { result, .. } => Some(result.id.clone()),
            Operation::Call(call) => Some(call.result.id.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    function.slots.retain(|slot| definitions.contains(&slot.id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hir::{Literal, Value},
        lir::Block,
    };
    #[test]
    fn conditional_ownership_is_retained_at_a_join_and_duplicate_releases_are_removed() {
        let id = ValueId::Local(0);
        let condition = Value {
            id: ValueId::Parameter(0),
            ty: "Bool".into(),
        };
        let mut function = Function {
            slots: vec![Value {
                id: id.clone(),
                ty: "Int".into(),
            }],
            blocks: vec![
                Block {
                    instructions: vec![],
                    terminator: Terminator::Branch {
                        condition,
                        yes: BlockId(1),
                        no: BlockId(2),
                    },
                },
                Block {
                    instructions: vec![Operation::Literal {
                        result: Value {
                            id: id.clone(),
                            ty: "Int".into(),
                        },
                        value: Literal::Int(1),
                    }],
                    terminator: Terminator::Jump(BlockId(2)),
                },
                Block {
                    instructions: vec![
                        Operation::Release(vec![id.clone()]),
                        Operation::Release(vec![id.clone()]),
                    ],
                    terminator: Terminator::Return {
                        span: None,
                        value: None,
                        cleanup: vec![id],
                    },
                },
            ],
        };
        simplify(&mut function);
        assert_eq!(function.blocks[2].instructions.len(), 1);
        let Terminator::Return { cleanup, .. } = &function.blocks[2].terminator else {
            unreachable!()
        };
        assert!(cleanup.is_empty());
    }
}
