use super::*;
use crate::{ast::AccessMode, readonly::operation};
use std::collections::VecDeque;

impl Context<'_> {
    pub(super) fn run(
        &mut self,
        body: &lir::Function,
        initial: State,
        routes: Routes,
        goal: Goal,
    ) -> Option<Found> {
        if !self.spend(body.blocks.len()) {
            return None;
        }
        let declarations = body
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter_map(|op| match op {
                Operation::AnnotateAccess { value, info }
                    if info.annotation.mode == AccessMode::Readonly =>
                {
                    Some((value.id.clone(), info.clone()))
                }
                _ => None,
            })
            .collect();
        self.declarations.push(declarations);
        let result = self.run_inner(body, initial, routes, goal);
        self.declarations.pop();
        result
    }
    fn run_inner(
        &mut self,
        body: &lir::Function,
        initial: State,
        routes: Routes,
        goal: Goal,
    ) -> Option<Found> {
        let mut entries = vec![None; body.blocks.len()];
        *entries.first_mut()? = Some((initial, routes));
        let mut pending = VecDeque::from([0]);
        let mut returned = Paths::default();
        let mut retained = Paths::default();
        while let Some(index) = pending.pop_front() {
            if !self.spend(1) {
                return None;
            }
            let entry = entries[index].as_ref()?;
            if !self.spend(entry.0.diagnostic_cost() + entry.1.cost()) {
                return None;
            }
            let (mut state, mut routes) = entry.clone();
            let block = &body.blocks[index];
            for (position, op) in block.instructions.iter().enumerate() {
                if !self.spend(1) {
                    return None;
                }
                let target = match &goal {
                    Goal::Access(point, value) if *point == (index,position) && has(&state.get(value), self.source) => Some(value.clone()),
                    Goal::Write => write_target(op, &state, self.source, self.program).filter(|_| !matches!(op, Operation::Call(call) if self.active.contains(&call.function))),
                    _ => None,
                };
                if let Some(value) = target {
                    return Some(Found {
                        paths: routes.get(&value, &state, self.source),
                        heap: routes.heap,
                        site: Some((index, position)),
                        value: Some(value),
                    });
                }
                if !self.spend(state.diagnostic_cost()) {
                    return None;
                }
                let before = state.clone();
                let mut summary = Summary::default();
                operation(
                    op,
                    &mut state,
                    &mut summary,
                    self.program,
                    self.summaries,
                    self.declarations.last()?,
                    false,
                )
                .ok()?;
                routes.operation(op, &before, &state, self);
            }
            retained.join(&routes.heap);
            let next = match &block.terminator {
                Terminator::Jump(target) => vec![target.0],
                Terminator::Branch { yes, no, .. } => vec![yes.0, no.0],
                Terminator::Return {
                    value: Some(value),
                    span,
                    ..
                } => {
                    if matches!(goal, Goal::Returns) {
                        let mut paths = routes.get(value, &state, self.source);
                        paths.step(span.clone(), "readonly access returned here".into());
                        returned.join(&paths);
                    }
                    vec![]
                }
                _ => vec![],
            };
            for next in next {
                if !self.spend(state.diagnostic_cost() + routes.cost()) {
                    return None;
                }
                let changed = match &mut entries[next] {
                    Some((old, trail)) => {
                        let a = old.join(&state);
                        let b = trail.join(&routes);
                        a || b
                    }
                    slot @ None => {
                        *slot = Some((state.clone(), routes.clone()));
                        true
                    }
                };
                if changed {
                    pending.push_back(next);
                }
            }
        }
        if matches!(goal, Goal::Returns) {
            Some(Found {
                paths: returned,
                heap: retained,
                site: None,
                value: None,
            })
        } else {
            None
        }
    }
}
