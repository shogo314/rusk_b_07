//! Remove instructions which follow an unconditional exit, before validating
//! values. A diverging source expression has no result slot in the HIR.
use crate::hir::{Body, Instruction, LoopId, Program};
fn breaks(body: &Body, target: LoopId) -> bool {
    body.instructions.iter().any(|op| match op {
        Instruction::Break(id) => *id == target,
        Instruction::Scope(body) | Instruction::Loop { body, .. } => breaks(body, target),
        Instruction::If { yes, no, .. } => breaks(yes, target) || breaks(no, target),
        _ => false,
    })
}
fn prune(body: &mut Body) -> bool {
    let mut exits = false;
    body.instructions.retain_mut(|op| {
        if exits {
            return false;
        }
        exits = match op {
            Instruction::Return { .. }
            | Instruction::Break(_)
            | Instruction::Continue(_)
            | Instruction::Unreachable => true,
            Instruction::Scope(body) => prune(body),
            Instruction::If { yes, no, .. } => {
                let a = prune(yes);
                let b = prune(no);
                a && b
            }
            Instruction::Loop { id, body } => {
                prune(body);
                !breaks(body, *id)
            }
            _ => false,
        };
        true
    });
    exits
}
impl Program {
    pub fn prune_unreachable(&mut self) {
        for f in &mut self.functions {
            if let Some(body) = &mut f.body {
                prune(body);
            }
        }
        prune(&mut self.main);
    }
}
