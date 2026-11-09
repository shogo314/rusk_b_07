//! Expand call witnesses only while rendering a failed contract. The effect
//! transfer and the frozen private-store proof remain the source of truth.
use super::*;
use crate::effects::{Effect, operation};
use crate::lir::Terminator;
use std::collections::VecDeque;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Goal {
    Returned(ReturnAlias),
    Retained(usize),
}
impl Goal {
    fn parameter(&self) -> usize {
        match self {
            Self::Returned(alias) => alias.parameter,
            Self::Retained(parameter) => *parameter,
        }
    }
}
pub(in crate::effects) struct Context<'a> {
    pub program: &'a lir::Program,
    pub callees: &'a [Summary],
    pub private: &'a [BTreeSet<ValueId>],
}
struct Budget(usize);
impl Budget {
    fn spend(&mut self, cost: usize) -> Option<()> {
        self.0 = self.0.checked_sub(cost)?;
        Some(())
    }
}
fn alias_cost(alias: &ReturnAlias) -> usize {
    1 + alias.parameter_path.len() + alias.result_path.len()
}
fn trail_cost(trail: &Trail) -> usize {
    1 + trail
        .steps
        .iter()
        .map(|step| {
            1 + step.text.len()
                + step.span.as_ref().map_or(0, |s| s.file.len())
                + match &step.callee {
                    Some((_, Goal::Returned(alias))) => alias_cost(alias),
                    _ => 0,
                }
        })
        .sum::<usize>()
}
fn state_cost(state: &State, routes: &Routes) -> usize {
    1 + state
        .values
        .values()
        .map(|v| 1 + v.aliases.iter().map(alias_cost).sum::<usize>())
        .sum::<usize>()
        + routes
            .values
            .values()
            .map(|v| {
                1 + v
                    .known
                    .iter()
                    .map(|(a, t)| alias_cost(a) + trail_cost(t))
                    .sum::<usize>()
                    + v.uncertain.as_ref().map_or(0, trail_cost)
            })
            .sum::<usize>()
        + routes.heap.as_ref().map_or(0, trail_cost)
        + routes
            .names
            .values()
            .map(|n| 1 + n.text.len() + n.span.file.len())
            .sum::<usize>()
}

fn trace(
    function: usize,
    goal: &Goal,
    context: &Context<'_>,
    budget: &mut Budget,
) -> Option<Trail> {
    let function = &context.program.functions[function];
    let Some(body) = &function.body else {
        let mut trail = Trail::default();
        trail.push(Step {
            span: None,
            text: format!(
                "body of '{}' is unavailable; internal sharing path cannot be reconstructed",
                function.name
            ),
            callee: None,
        });
        return Some(trail);
    };
    // Charge the initial CFG allocation and the binding-name scan before doing
    // either. Clones and joins below are charged by their current state size.
    let mut initial_cost = 1 + body.blocks.len() + function.params.len();
    for block in &body.blocks {
        budget.spend(1 + block.instructions.len())?;
        for op in &block.instructions {
            if let Operation::DescribeBinding { name, .. } = op {
                initial_cost = initial_cost.saturating_add(name.text.len() + name.span.file.len());
            }
        }
    }
    budget.spend(initial_cost)?;
    let parameter = goal.parameter();
    let initial = State {
        values: (0..function.params.len())
            .map(|i| (ValueId::Parameter(i), ReturnOrigins::parameter(i)))
            .collect(),
        heap_unknown: false,
    };
    let mut entries = vec![None; body.blocks.len()];
    entries[0] = Some((initial, Routes::new(body, parameter)));
    let mut pending = VecDeque::from([0]);
    while let Some(index) = pending.pop_front() {
        let (entry, paths) = entries[index].as_ref()?;
        budget.spend(state_cost(entry, paths))?;
        let (mut state, mut routes) = (entry.clone(), paths.clone());
        let block = &body.blocks[index];
        for op in &block.instructions {
            let cost = state_cost(&state, &routes);
            let expansion = match op {
                Operation::Call(call) => {
                    1 + call.args.len()
                        + context.callees[call.function]
                            .returned
                            .aliases
                            .iter()
                            .map(alias_cost)
                            .sum::<usize>()
                }
                Operation::Object {
                    expression: Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. },
                    ..
                } => 1 + fields.len(),
                _ => 1,
            };
            budget.spend(
                cost.saturating_mul(expansion)
                    .saturating_add(function.params.len()),
            )?;
            let before = state.clone();
            let mut step = Summary::inferred(function.params.len());
            operation(
                op,
                &mut state,
                &mut step,
                context.callees,
                &context.private[function.id],
            );
            routes.operation(op, &before, &state, context.callees, context.program);
            if matches!(goal, Goal::Retained(_))
                && step.parameters[parameter].retained != Effect::No
            {
                match op {
                    Operation::Call(call) => {
                        let (argument, value) =
                            call.args.iter().enumerate().find(|(i, value)| {
                                let origin = before.get(value);
                                context.callees[call.function].parameters[*i].retained != Effect::No
                                    && (origin.unknown
                                        || origin.aliases.iter().any(|a| a.parameter == parameter))
                            })?;
                        let mut trail = routes
                            .trail(value, &before.get(value), parameter)
                            .unwrap_or_default();
                        trail.push(Step {
                            span: Some(call.span.clone()),
                            text: format!(
                                "argument {} of '{}' may be retained outside the call{}",
                                argument + 1,
                                context.program.functions[call.function].name,
                                if before.get(value).unknown {
                                    "; sharing origin is uncertain"
                                } else {
                                    ""
                                }
                            ),
                            callee: Some((call.function, Goal::Retained(argument))),
                        });
                        return Some(trail);
                    }
                    Operation::Rebind {
                        target: Place::Field { owner, index },
                        value,
                        span,
                    } => {
                        let origin = before.get(value);
                        let mut trail = routes.trail(value, &origin, parameter).unwrap_or_default();
                        trail.push(Step {
                            span: span.clone(),
                            text: format!("field store to {} may retain the shared value; destination is not proven private{}",
                                projection_name(&owner.ty, &Projection::Field(*index), context.program),
                                if origin.unknown { "; sharing origin is uncertain" } else { "" }),
                            callee: None,
                        });
                        return Some(trail);
                    }
                    _ => return None,
                }
            }
        }
        if let Goal::Returned(alias) = goal
            && let Terminator::Return {
                value: Some(value),
                span,
                ..
            } = &block.terminator
            && state.returned(value).aliases.contains(alias)
            && let Some(mut trail) = routes
                .values
                .get(&value.id)
                .and_then(|v| v.known.get(alias))
                .cloned()
        {
            trail.push(Step {
                span: span.clone(),
                text: format!("shared value returned from '{}'", function.name),
                callee: None,
            });
            return Some(trail);
        }
        let next = match &block.terminator {
            Terminator::Jump(target) => vec![target.0],
            Terminator::Branch { yes, no, .. } => vec![yes.0, no.0],
            _ => vec![],
        };
        for next in next {
            let cost = state_cost(&state, &routes)
                .saturating_add(entries[next].as_ref().map_or(0, |(s, r)| state_cost(s, r)));
            budget.spend(cost.saturating_mul(3))?;
            let changed = match &mut entries[next] {
                Some((entry, paths)) => {
                    let changed = entry.join(&state);
                    paths.join(&routes) || changed
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
    None
}

struct Renderer<'a, 'b> {
    context: &'a Context<'b>,
    budget: Budget,
    active: Vec<usize>,
    lines: Vec<String>,
    omitted: bool,
}
impl Renderer<'_, '_> {
    fn append(&mut self, trail: &Trail) {
        self.omitted |= trail.truncated;
        for step in &trail.steps {
            if self.lines.len() == MAX_STEPS {
                self.omitted = true;
                return;
            }
            self.lines.push(format!(
                "  {}",
                step.span
                    .as_ref()
                    .map_or_else(|| step.text.clone(), |span| span.error(&step.text))
            ));
            if let Some((function, goal)) = &step.callee {
                if self.lines.len() == MAX_STEPS {
                    self.omitted = true;
                    continue;
                }
                if self.active.len() == 16 || self.active.contains(function) {
                    self.note("recursive call or call-depth limit; internal sharing path omitted");
                    continue;
                }
                self.active.push(*function);
                match trace(*function, goal, self.context, &mut self.budget) {
                    Some(inner) => self.append(&inner),
                    None => self.note(
                        "internal sharing path uncertain or exceeds diagnostic analysis limits",
                    ),
                }
                self.active.pop();
            }
        }
    }
    fn note(&mut self, text: &str) {
        if self.lines.len() < MAX_STEPS {
            self.lines.push(format!("  {text}"));
        } else {
            self.omitted = true;
        }
    }
}
pub(super) fn render(trail: &Trail, context: &Context<'_>) -> String {
    let mut renderer = Renderer {
        context,
        budget: Budget(1_000_000),
        active: vec![],
        lines: vec![],
        omitted: false,
    };
    renderer.append(trail);
    if renderer.omitted {
        renderer
            .lines
            .push("  ... additional sharing steps omitted".into());
    }
    renderer.lines.join("\n")
}
