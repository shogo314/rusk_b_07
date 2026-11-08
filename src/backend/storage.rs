//! Keep values local to a C++ basic-block scope when their entire observable
//! lifetime (including explicit releases) is contained in that block.
use crate::{
    hir::{Place, Rvalue, ValueId},
    lir::{Function, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn persistent(function: &Function) -> BTreeSet<ValueId> {
    let mut uses = BTreeMap::<ValueId, BTreeSet<usize>>::new();
    for (block_id, block) in function.blocks.iter().enumerate() {
        let mut add = |id: &ValueId| {
            uses.entry(id.clone()).or_default().insert(block_id);
        };
        for op in &block.instructions {
            match op {
                Operation::Object { result, expression } => {
                    add(&result.id);
                    match expression {
                        Rvalue::Share(v)
                        | Rvalue::BoolCopy(v)
                        | Rvalue::VariantTest { owner: v, .. }
                        | Rvalue::VariantField { owner: v, .. } => add(&v.id),
                        Rvalue::Field { owner, .. } => add(&owner.id),
                        Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                            for v in fields {
                                add(&v.id);
                            }
                        }
                    }
                }
                Operation::Literal { result, .. } => add(&result.id),
                Operation::Call(call) => {
                    add(&call.result.id);
                    for v in &call.args {
                        add(&v.id);
                    }
                }
                Operation::Declare { result, initial } => {
                    add(&result.id);
                    if let Some(v) = initial {
                        add(&v.id);
                    }
                }
                Operation::Rebind { target, value, .. } => {
                    match target {
                        Place::Local(id) => add(id),
                        Place::Field { owner, .. } => add(&owner.id),
                    }
                    add(&value.id);
                }
                Operation::StoreBool { target, value } => {
                    add(&target.id);
                    add(&value.id);
                }
                Operation::DescribeBinding { .. }
                | Operation::AnnotateBinding { .. }
                | Operation::AnnotateAccess { .. }
                | Operation::AnnotateEscape { .. } => {}
                Operation::Release(ids) => {
                    for id in ids {
                        add(id);
                    }
                }
            }
        }
        match &block.terminator {
            Terminator::Branch { condition, .. } => add(&condition.id),
            Terminator::Return { value, cleanup, .. } => {
                if let Some(v) = value {
                    add(&v.id);
                }
                for id in cleanup {
                    add(id);
                }
            }
            _ => {}
        }
    }
    uses.into_iter()
        .filter_map(|(id, blocks)| (blocks.len() > 1).then_some(id))
        .collect()
}
