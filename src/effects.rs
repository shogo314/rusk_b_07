//! Conservative, target-independent summaries. Opaque code is never inferred
//! pure from its name or from target-language source.
mod private;
mod routes;
pub(crate) use routes::projection_name;

use crate::{
    hir::{Place, Rvalue, Value, ValueId},
    lir::{self, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Effect {
    #[default]
    No,
    May,
    Unknown,
}
impl Effect {
    fn join(&mut self, other: Self) {
        *self = (*self).max(other);
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParameterEffects {
    pub read: Effect,
    pub write: Effect,
    pub structural: Effect,
    pub escape: Effect,
    /// Retained outside the call other than via its returned value.
    pub retained: Effect,
}
impl ParameterEffects {
    fn unknown() -> Self {
        Self {
            read: Effect::Unknown,
            write: Effect::Unknown,
            structural: Effect::Unknown,
            escape: Effect::Unknown,
            retained: Effect::Unknown,
        }
    }
    fn join(&mut self, other: &Self) {
        self.read.join(other.read);
        self.write.join(other.write);
        self.structural.join(other.structural);
        self.escape.join(other.escape);
        self.retained.join(other.retained);
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Projection {
    Element,
    Field(usize),
    VariantField { variant: usize, field: usize },
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReturnAlias {
    pub result_path: Vec<Projection>,
    pub parameter: usize,
    pub parameter_path: Vec<Projection>,
}
/// A may-set, including references contained in an aggregate. Unknown is top;
/// fresh is a possible independent allocation, not a proof of uniqueness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReturnOrigins {
    pub aliases: BTreeSet<ReturnAlias>,
    pub fresh: bool,
    pub unknown: bool,
}
const MAX_ALIASES: usize = 64;
const MAX_PATH: usize = 16;
impl ReturnOrigins {
    fn unknown() -> Self {
        Self {
            unknown: true,
            ..Self::default()
        }
    }
    fn fresh() -> Self {
        Self {
            fresh: true,
            ..Self::default()
        }
    }
    fn parameter(parameter: usize) -> Self {
        Self {
            aliases: [ReturnAlias {
                result_path: vec![],
                parameter,
                parameter_path: vec![],
            }]
            .into(),
            ..Self::default()
        }
    }
    fn bounded(mut self) -> Self {
        if self.unknown
            || self.aliases.len() > MAX_ALIASES
            || self
                .aliases
                .iter()
                .any(|a| a.result_path.len() > MAX_PATH || a.parameter_path.len() > MAX_PATH)
        {
            self = Self::unknown();
        }
        self
    }
    fn join(&mut self, other: &Self) {
        self.unknown |= other.unknown;
        self.fresh |= other.fresh;
        self.aliases.extend(other.aliases.iter().cloned());
        *self = std::mem::take(self).bounded();
    }
    fn project(&self, projection: Projection) -> Self {
        if self.unknown {
            return Self::unknown();
        }
        let mut result = Self {
            fresh: self.fresh,
            ..Self::default()
        };
        for alias in &self.aliases {
            let mut alias = alias.clone();
            if alias.result_path.first() == Some(&projection) {
                alias.result_path.remove(0);
            } else if alias.result_path.is_empty() {
                alias.parameter_path.push(projection.clone());
            } else {
                continue;
            }
            result.aliases.insert(alias);
        }
        result.bounded()
    }
    fn contain(&self, projection: Projection) -> Self {
        let mut result = self.clone();
        result.aliases = result
            .aliases
            .into_iter()
            .map(|mut a| {
                a.result_path.insert(0, projection.clone());
                a
            })
            .collect();
        result.bounded()
    }
    fn substitute(&self, args: &[Self]) -> Self {
        if self.unknown {
            return Self::unknown();
        }
        let mut result = Self {
            fresh: self.fresh,
            ..Self::default()
        };
        for alias in &self.aliases {
            let mut actual = args[alias.parameter].clone();
            for projection in &alias.parameter_path {
                actual = actual.project(projection.clone());
            }
            for projection in alias.result_path.iter().rev() {
                actual = actual.contain(projection.clone());
            }
            result.join(&actual);
        }
        result
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SummarySource {
    Unanalyzed,
    Inferred,
    Opaque,
    NativeContract,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Summary {
    pub source: SummarySource,
    pub parameters: Vec<ParameterEffects>,
    pub returned: ReturnOrigins,
    pub io: Effect,
    pub external_write: Effect,
    pub may_fail: Effect,
    pub may_diverge: Effect,
}
impl Summary {
    pub fn unanalyzed(parameters: usize) -> Self {
        Self::unknown(parameters, SummarySource::Unanalyzed)
    }
    fn unknown(parameters: usize, source: SummarySource) -> Self {
        Self {
            source,
            parameters: vec![ParameterEffects::unknown(); parameters],
            returned: ReturnOrigins::unknown(),
            io: Effect::Unknown,
            external_write: Effect::Unknown,
            may_fail: Effect::Unknown,
            may_diverge: Effect::Unknown,
        }
    }
    pub fn opaque(parameters: usize) -> Self {
        Self::unknown(parameters, SummarySource::Opaque)
    }
    pub fn native(parameters: usize) -> Self {
        Self {
            source: SummarySource::NativeContract,
            ..Self::inferred(parameters)
        }
    }
    fn inferred(parameters: usize) -> Self {
        Self {
            source: SummarySource::Inferred,
            parameters: vec![ParameterEffects::default(); parameters],
            returned: ReturnOrigins::default(),
            io: Effect::No,
            external_write: Effect::No,
            may_fail: Effect::No,
            may_diverge: Effect::No,
        }
    }
    pub fn validate(&self, parameters: usize) -> Result<(), String> {
        if matches!(
            self.source,
            SummarySource::Opaque | SummarySource::Unanalyzed
        ) && *self != Self::unknown(parameters, self.source.clone())
        {
            return Err("opaque or unanalyzed effects must be unknown".into());
        }
        if self.parameters.len() != parameters {
            return Err("invalid effect summary parameter count".into());
        }
        if self
            .returned
            .aliases
            .iter()
            .any(|a| a.parameter >= parameters)
        {
            return Err("invalid effect summary return parameter".into());
        }
        if self
            .returned
            .aliases
            .iter()
            .any(|a| self.parameters[a.parameter].escape == Effect::No)
            || (self.returned.unknown && self.parameters.iter().any(|p| p.escape == Effect::No))
        {
            return Err("effect summary loses return escape".into());
        }
        if self.parameters.iter().any(|p| p.retained > p.escape) {
            return Err("effect summary loses retained escape".into());
        }
        if self.returned.clone().bounded() != self.returned {
            return Err("invalid effect summary provenance bounds".into());
        }
        Ok(())
    }
    pub fn validate_signature(
        &self,
        parameters: &[String],
        result: &str,
        types: &BTreeMap<String, crate::hir::Type>,
        has_body: bool,
    ) -> Result<(), String> {
        self.validate(parameters.len())?;
        if !has_body && self.source == SummarySource::Inferred {
            return Err("cannot infer effects for an opaque body".into());
        }
        if has_body && self.source == SummarySource::NativeContract {
            return Err("native contract requires an opaque body".into());
        }
        for alias in &self.returned.aliases {
            let source = project_type(&parameters[alias.parameter], &alias.parameter_path, types)?;
            let target = project_type(result, &alias.result_path, types)?;
            if source != target {
                return Err("effect summary alias type mismatch".into());
            }
        }
        Ok(())
    }
    fn affect(&mut self, value: &ReturnOrigins, effect: &ParameterEffects) {
        if value.unknown {
            let uncertain = |effect| {
                if effect == Effect::No {
                    Effect::No
                } else {
                    Effect::Unknown
                }
            };
            let effect = ParameterEffects {
                read: uncertain(effect.read),
                write: uncertain(effect.write),
                structural: uncertain(effect.structural),
                escape: uncertain(effect.escape),
                retained: uncertain(effect.retained),
            };
            for parameter in &mut self.parameters {
                parameter.join(&effect);
            }
        } else {
            for origin in &value.aliases {
                self.parameters[origin.parameter].join(effect);
            }
        }
    }
}
pub(crate) fn project_type(
    root: &str,
    path: &[Projection],
    types: &BTreeMap<String, crate::hir::Type>,
) -> Result<String, String> {
    let mut ty = root.to_string();
    for projection in path {
        ty = match (types.get(&ty), projection) {
            (Some(crate::hir::Type::Vector(element)), Projection::Element) => Some(element.clone()),
            (Some(crate::hir::Type::Primitive), Projection::Element) if ty == "Str" => {
                Some("Char".into())
            }
            (Some(kind), Projection::Field(index)) => kind
                .fields()
                .and_then(|fields| fields.get(*index))
                .map(|(_, ty)| ty.clone()),
            (
                Some(crate::hir::Type::Enum(variants)),
                Projection::VariantField { variant, field },
            ) => variants
                .get(*variant)
                .and_then(|(_, fields)| fields.get(*field))
                .cloned(),
            _ => None,
        }
        .ok_or("invalid effect summary projection")?;
    }
    Ok(ty)
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    values: BTreeMap<ValueId, ReturnOrigins>,
    /// Member stores and opaque calls currently invalidate heap projections.
    /// A later field-sensitive heap analysis can replace this conservative top.
    heap_unknown: bool,
}
impl State {
    fn get(&self, value: &Value) -> ReturnOrigins {
        self.values
            .get(&value.id)
            .cloned()
            .unwrap_or_else(ReturnOrigins::unknown)
    }
    fn returned(&self, value: &Value) -> ReturnOrigins {
        if self.heap_unknown && !matches!(value.ty.as_str(), "Int" | "Bool" | "Char" | "Unit") {
            ReturnOrigins::unknown()
        } else {
            self.get(value)
        }
    }
    fn join(&mut self, other: &Self) -> bool {
        let old = self.clone();
        self.heap_unknown |= other.heap_unknown;
        for (id, value) in &other.values {
            self.values.entry(id.clone()).or_default().join(value);
        }
        *self != old
    }
}
fn read(summary: &mut Summary, origins: &ReturnOrigins) {
    summary.affect(
        origins,
        &ParameterEffects {
            read: Effect::May,
            ..ParameterEffects::default()
        },
    );
}
fn escapes(summary: &mut Summary, origins: &ReturnOrigins) {
    summary.affect(
        origins,
        &ParameterEffects {
            escape: Effect::May,
            ..ParameterEffects::default()
        },
    );
}
fn operation(
    op: &Operation,
    state: &mut State,
    summary: &mut Summary,
    callees: &[Summary],
    private: &BTreeSet<ValueId>,
) {
    match op {
        Operation::Literal { result, .. } => {
            state
                .values
                .insert(result.id.clone(), ReturnOrigins::fresh());
        }
        Operation::Declare { result, initial } => {
            let value = initial.as_ref().map(|v| state.get(v)).unwrap_or_default();
            state.values.insert(result.id.clone(), value);
        }
        Operation::Object { result, expression } => {
            let value = match expression {
                Rvalue::Share(value) => state.get(value),
                Rvalue::BoolCopy(value) => {
                    read(summary, &state.get(value));
                    ReturnOrigins::fresh()
                }
                Rvalue::Field { owner, index } => {
                    let owner = state.get(owner);
                    read(summary, &owner);
                    if state.heap_unknown {
                        ReturnOrigins::unknown()
                    } else {
                        owner.project(Projection::Field(*index))
                    }
                }
                Rvalue::VariantField {
                    owner,
                    variant,
                    index,
                } => {
                    let owner = state.get(owner);
                    read(summary, &owner);
                    if state.heap_unknown {
                        ReturnOrigins::unknown()
                    } else {
                        owner.project(Projection::VariantField {
                            variant: *variant,
                            field: *index,
                        })
                    }
                }
                Rvalue::VariantTest { owner, .. } => {
                    read(summary, &state.get(owner));
                    ReturnOrigins::fresh()
                }
                Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                    let mut value = ReturnOrigins::fresh();
                    for (index, field) in fields.iter().enumerate() {
                        let projection = match expression {
                            Rvalue::Variant { index: variant, .. } => Projection::VariantField {
                                variant: *variant,
                                field: index,
                            },
                            _ => Projection::Field(index),
                        };
                        value.join(&state.get(field).contain(projection));
                    }
                    value
                }
            };
            state.values.insert(result.id.clone(), value);
        }
        Operation::Rebind { target, value, .. } => {
            let value = state.get(value);
            match target {
                Place::Local(id) => {
                    state.values.insert(id.clone(), value);
                }
                Place::Field { owner, .. } => {
                    summary.affect(
                        &state.get(owner),
                        &ParameterEffects {
                            write: Effect::May,
                            ..ParameterEffects::default()
                        },
                    );
                    // A function-wide proof rules out publication of these
                    // destinations through any alias. Other stores can retain
                    // the value outside the call, including stores performed
                    // before an eventual publication of their destination.
                    if !private.contains(&owner.id) {
                        summary.affect(
                            &value,
                            &ParameterEffects {
                                escape: Effect::May,
                                retained: Effect::May,
                                ..ParameterEffects::default()
                            },
                        );
                    }
                    state.heap_unknown = true;
                }
            }
        }
        Operation::StoreBool { target, value } => {
            read(summary, &state.get(value));
            summary.affect(
                &state.get(target),
                &ParameterEffects {
                    write: Effect::May,
                    ..ParameterEffects::default()
                },
            );
        }
        Operation::Call(call) => {
            let callee = &callees[call.function];
            let args: Vec<_> = call.args.iter().map(|v| state.get(v)).collect();
            for (actual, effect) in args.iter().zip(&callee.parameters) {
                let mut passed = effect.clone();
                // Returning a reference only escapes the callee. The caller's
                // uses of that result determine whether it escapes again.
                passed.escape = effect.retained;
                summary.affect(actual, &passed);
                if effect.write != Effect::No || effect.structural != Effect::No {
                    state.heap_unknown = true;
                }
            }
            summary.io.join(callee.io);
            summary.external_write.join(callee.external_write);
            state.heap_unknown |= callee.external_write != Effect::No;
            summary.may_fail.join(callee.may_fail);
            summary.may_diverge.join(callee.may_diverge);
            let returned = if state.heap_unknown
                && callee
                    .returned
                    .aliases
                    .iter()
                    .any(|alias| !alias.parameter_path.is_empty())
            {
                ReturnOrigins::unknown()
            } else {
                callee.returned.substitute(&args)
            };
            state.values.insert(call.result.id.clone(), returned);
        }
        Operation::Release(_)
        | Operation::DescribeBinding { .. }
        | Operation::AnnotateBinding { .. }
        | Operation::AnnotateAccess { .. }
        | Operation::AnnotateEscape { .. } => {}
    }
}
fn analyze_function(
    function: &lir::ProgramFunction,
    callees: &[Summary],
    private: &BTreeSet<ValueId>,
) -> Summary {
    let Some(body) = &function.body else {
        return if function.effects.source == SummarySource::NativeContract {
            function.effects.clone()
        } else {
            Summary::unknown(function.params.len(), SummarySource::Opaque)
        };
    };
    let mut summary = Summary::inferred(function.params.len());
    let initial = State {
        values: (0..function.params.len())
            .map(|i| (ValueId::Parameter(i), ReturnOrigins::parameter(i)))
            .collect(),
        heap_unknown: false,
    };
    let mut entries = vec![None; body.blocks.len()];
    entries[0] = Some(initial);
    let mut pending = VecDeque::from([0]);
    while let Some(index) = pending.pop_front() {
        let mut state = entries[index].as_ref().unwrap().clone();
        let block = &body.blocks[index];
        for op in &block.instructions {
            operation(op, &mut state, &mut summary, callees, private);
        }
        let mut successors = vec![];
        match &block.terminator {
            Terminator::Jump(target) => {
                successors.push(target.0);
            }
            Terminator::Branch { condition, yes, no } => {
                read(&mut summary, &state.get(condition));
                successors.extend([yes.0, no.0]);
            }
            Terminator::Return {
                value: Some(value), ..
            } => {
                // Mutated aggregates may retain sources beyond their initial fields.
                let origins = state.returned(value);
                escapes(&mut summary, &origins);
                summary.returned.join(&origins);
            }
            Terminator::Return { value: None, .. } | Terminator::Unreachable => {}
        }
        for successor in successors {
            let changed = match &mut entries[successor] {
                Some(entry) => entry.join(&state),
                entry @ None => {
                    *entry = Some(state.clone());
                    true
                }
            };
            if changed {
                pending.push_back(successor);
            }
        }
    }
    // A backward-numbered edge can be an acyclic join. Detect actual cycles
    // among reachable blocks rather than depending on lowering's numbering.
    let mut indegree = vec![0usize; body.blocks.len()];
    let successors = |index: usize| -> Vec<usize> {
        match &body.blocks[index].terminator {
            Terminator::Jump(target) => vec![target.0],
            Terminator::Branch { yes, no, .. } => vec![yes.0, no.0],
            _ => vec![],
        }
    };
    for (index, entry) in entries.iter().enumerate() {
        if entry.is_some() {
            for target in successors(index) {
                indegree[target] += 1;
            }
        }
    }
    let reachable = entries.iter().filter(|entry| entry.is_some()).count();
    let mut ready: VecDeque<_> = (0..entries.len())
        .filter(|&index| entries[index].is_some() && indegree[index] == 0)
        .collect();
    let mut visited = 0;
    while let Some(index) = ready.pop_front() {
        visited += 1;
        for target in successors(index) {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                ready.push_back(target);
            }
        }
    }
    if visited < reachable {
        summary.may_diverge.join(Effect::May);
    }
    summary
}
#[derive(Debug)]
struct EscapeWitness {
    span: Option<crate::ast::Span>,
    effect: Effect,
    reason: String,
    route: String,
}
fn origin_route(origins: &ReturnOrigins, parameter: usize) -> String {
    if origins.unknown {
        return "sharing origin is unknown (heap mutation, opaque call or analysis limit)".into();
    }
    if let Some(alias) = origins.aliases.iter().find(|a| a.parameter == parameter) {
        format!(
            "parameter {} -> {:?} -> contained at {:?}",
            parameter + 1,
            alias.parameter_path,
            alias.result_path
        )
    } else {
        "through a derived alias or a retained value".into()
    }
}
fn first_escape(
    function: &lir::ProgramFunction,
    callees: &[Summary],
    parameter: usize,
    program: &lir::Program,
    private: &[BTreeSet<ValueId>],
) -> Option<EscapeWitness> {
    let body = function.body.as_ref()?;
    let context = routes::Context {
        program,
        callees,
        private,
    };
    let initial = State {
        values: (0..function.params.len())
            .map(|i| (ValueId::Parameter(i), ReturnOrigins::parameter(i)))
            .collect(),
        heap_unknown: false,
    };
    let mut entries = vec![None; body.blocks.len()];
    entries[0] = Some((initial, routes::Routes::new(body, parameter)));
    let mut pending = VecDeque::from([0]);
    while let Some(index) = pending.pop_front() {
        let (mut state, mut routes) = entries[index].as_ref().unwrap().clone();
        let block = &body.blocks[index];
        for op in &block.instructions {
            let before = state.clone();
            let mut step = Summary::inferred(function.params.len());
            operation(op, &mut state, &mut step, callees, &private[function.id]);
            routes.operation(op, &before, &state, callees, program);
            if step.parameters[parameter].escape != Effect::No {
                let (span, effect, reason, route) = match op {
                    Operation::Call(call) => {
                        let callee = &program.functions[call.function];
                        let actual = call.args.iter().enumerate().find(|(i, arg)| {
                            let origin = before.get(arg);
                            callee.effects.parameters[*i].retained != Effect::No
                                && (origin.unknown
                                    || origin.aliases.iter().any(|a| a.parameter == parameter))
                        });
                        let (argument, route) = actual
                            .map(|(i, arg)| {
                                (
                                    format!("argument {}", i + 1),
                                    format!(
                                        "{}{}",
                                        origin_route(&before.get(arg), parameter),
                                        routes.retained(
                                            arg,
                                            &before.get(arg),
                                            parameter,
                                            call.function,
                                            i,
                                            &context
                                        )
                                    ),
                                )
                            })
                            .unwrap_or(("argument".into(), "unknown retention path".into()));
                        (
                            Some(call.span.clone()),
                            step.parameters[parameter].escape,
                            format!(
                                "{argument} of '{}' may be retained outside the call",
                                callee.name
                            ),
                            route,
                        )
                    }
                    Operation::Rebind {
                        target: Place::Field { owner, index },
                        value,
                        span,
                    } => (
                        span.clone(),
                        Effect::Unknown,
                        format!(
                            "a field store to {} may retain the shared value beyond this call; destination is not proven private",
                            routes::projection_name(&owner.ty, &Projection::Field(*index), program)
                        ),
                        format!(
                            "{}{}",
                            origin_route(&before.get(value), parameter),
                            routes.describe(value, &before.get(value), parameter, &context)
                        ),
                    ),
                    _ => (
                        None,
                        step.parameters[parameter].escape,
                        "an operation may retain the parameter".into(),
                        "unknown retention path".into(),
                    ),
                };
                return Some(EscapeWitness {
                    span,
                    effect,
                    reason,
                    route,
                });
            }
        }
        let next = match &block.terminator {
            Terminator::Jump(target) => vec![target.0],
            Terminator::Branch { yes, no, .. } => vec![yes.0, no.0],
            Terminator::Return {
                value: Some(value),
                span,
                ..
            } => {
                let origins = state.returned(value);
                let mut step = Summary::inferred(function.params.len());
                escapes(&mut step, &origins);
                if step.parameters[parameter].escape != Effect::No {
                    return Some(EscapeWitness {
                        span: span.clone(),
                        effect: step.parameters[parameter].escape,
                        reason: "the return value may share the parameter, one of its elements or a captured value".into(),
                        route: format!("{}{}", origin_route(&origins, parameter), routes.describe(value, &origins, parameter, &context)),
                    });
                }
                vec![]
            }
            _ => vec![],
        };
        for next in next {
            let changed = match &mut entries[next] {
                Some((entry, paths)) => {
                    let state_changed = entry.join(&state);
                    let routes_changed = paths.join(&routes);
                    state_changed || routes_changed
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
/// Annotations assert an inferred fact; they never replace an unknown effect
/// with an optimistic one. Returning a value is distinguished from a callee
/// merely returning an alias which the caller later copies or discards.
pub(crate) fn validate_noescape(program: &lir::Program) -> Result<(), String> {
    for function in &program.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for op in &block.instructions {
                let Operation::AnnotateEscape { parameter, info } = op else {
                    continue;
                };
                if info.annotation.mode != crate::ast::EscapeMode::Noescape
                    || function.effects.parameters[*parameter].escape == Effect::No
                {
                    continue;
                }
                let callees: Vec<_> = program
                    .functions
                    .iter()
                    .map(|f| f.effects.clone())
                    .collect();
                // Reconstruct the same frozen proof used by inference, rather
                // than deriving a different proof from the final summaries.
                let structural = private_plans(program, None);
                let baseline = solve(program, &structural);
                let plans = private_plans(program, Some(&baseline));
                let witness = first_escape(function, &callees, *parameter, program, &plans);
                let span = witness
                    .as_ref()
                    .and_then(|w| w.span.as_ref())
                    .unwrap_or(&info.declaration);
                let effect = witness.as_ref().map_or(Effect::Unknown, |w| w.effect);
                let message = if effect == Effect::Unknown {
                    format!("cannot prove noescape for parameter '{}'", info.name)
                } else {
                    format!("noescape parameter '{}' may escape", info.name)
                };
                let detail = witness
                    .as_ref()
                    .map(|w| format!("{}\nthrough {}", w.reason, w.route))
                    .unwrap_or_else(|| "inferred sharing effects are unknown".into());
                return Err(format!(
                    "{}\n{}\n{}\n{}\nwhile checking noescape in '{}'",
                    span.error(message),
                    info.declaration.error("parameter declared here"),
                    info.annotation.span.error("noescape modifier is here"),
                    detail,
                    function.name
                ));
            }
        }
    }
    Ok(())
}
fn private_plans(program: &lir::Program, callees: Option<&[Summary]>) -> Vec<BTreeSet<ValueId>> {
    program
        .functions
        .iter()
        .map(|function| {
            function
                .body
                .as_ref()
                .map(|body| private::destinations(body, callees))
                .unwrap_or_default()
        })
        .collect()
}

pub(crate) fn analyze(program: &lir::Program) -> Vec<Summary> {
    // Freeze call facts before using them to prove private stores. Otherwise a
    // store's retention and the proof permitting that store could depend on
    // one another, making the recursive equation non-monotone.
    let structural = private_plans(program, None);
    let baseline = solve(program, &structural);
    let refined = private_plans(program, Some(&baseline));
    if refined == structural {
        baseline
    } else {
        solve(program, &refined)
    }
}

/// Solve recursive calls with fixed private destinations. Return provenance
/// starts at bottom; unbounded paths and alias sets widen to unknown.
fn solve(program: &lir::Program, private: &[BTreeSet<ValueId>]) -> Vec<Summary> {
    let mut summaries: Vec<_> = program
        .functions
        .iter()
        .map(|f| {
            if f.body.is_some() {
                Summary::inferred(f.params.len())
            } else {
                if f.effects.source == SummarySource::NativeContract {
                    f.effects.clone()
                } else {
                    Summary::unknown(f.params.len(), SummarySource::Opaque)
                }
            }
        })
        .collect();
    // Any function on a call cycle may diverge. Propagation below also marks
    // callers; no optimistic termination claim is used to erase such calls.
    let edges: Vec<BTreeSet<usize>> = program
        .functions
        .iter()
        .map(|f| {
            f.body
                .as_ref()
                .into_iter()
                .flat_map(|b| &b.blocks)
                .flat_map(|b| &b.instructions)
                .filter_map(|op| {
                    if let Operation::Call(c) = op {
                        Some(c.function)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let recursive: Vec<_> = (0..edges.len())
        .map(|root| {
            let mut todo: Vec<_> = edges[root].iter().copied().collect();
            let mut seen = BTreeSet::new();
            while let Some(id) = todo.pop() {
                if id == root {
                    return true;
                }
                if seen.insert(id) {
                    todo.extend(&edges[id]);
                }
            }
            false
        })
        .collect();
    loop {
        let mut next: Vec<_> = program
            .functions
            .iter()
            .map(|f| analyze_function(f, &summaries, &private[f.id]))
            .collect();
        for (i, recursive) in recursive.iter().enumerate() {
            if *recursive {
                next[i].may_diverge.join(Effect::May);
            }
        }
        if next == summaries {
            return next;
        }
        summaries = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{self, Body, Instruction as I, Literal};

    fn param(i: usize, ty: &str) -> Value {
        Value {
            id: ValueId::Parameter(i),
            ty: ty.into(),
        }
    }
    fn local(i: usize, ty: &str) -> Value {
        Value {
            id: ValueId::Local(i),
            ty: ty.into(),
        }
    }
    fn function(params: &[&str], result: &str, body: Option<Vec<I>>) -> hir::Function {
        hir::Function {
            id: 0,
            name: "test".into(),
            params: params.iter().map(|s| (*s).into()).collect(),
            result: result.into(),
            effects: Summary::unanalyzed(params.len()),
            access: crate::readonly::Summary::default(),
            body: body.map(|instructions| Body { instructions }),
        }
    }
    fn program(mut functions: Vec<hir::Function>) -> lir::Program {
        for (id, f) in functions.iter_mut().enumerate() {
            f.id = id;
        }
        let mut hir = hir::Program {
            types: [
                ("Int".into(), hir::Type::Primitive),
                ("Bool".into(), hir::Type::Primitive),
                ("Unit".into(), hir::Type::Primitive),
                (
                    "Maybe".into(),
                    hir::Type::Enum(vec![
                        ("Empty".into(), vec![]),
                        ("Full".into(), vec!["Int".into()]),
                    ]),
                ),
                (
                    "Box".into(),
                    hir::Type::Record(vec![("value".into(), "Int".into())]),
                ),
            ]
            .into(),
            functions,
            main: Body {
                instructions: vec![I::Return {
                    value: None,
                    span: None,
                }],
            },
        };
        hir.prune_unreachable();
        hir.validate().unwrap();
        let lir = lir::lower_program(&hir);
        lir.validate().unwrap();
        lir
    }
    fn analyze_checked(functions: Vec<hir::Function>) -> Vec<Summary> {
        let mut program = program(functions);
        let results = analyze(&program);
        for (f, summary) in program.functions.iter_mut().zip(&results) {
            f.effects = summary.clone();
        }
        program.validate().unwrap();
        results
    }
    fn call(id: usize, args: Vec<Value>, result: Value) -> I {
        I::Call(hir::Call {
            function: id,
            args,
            result,
            inherit_location: false,
            span: crate::ast::Span {
                file: "effects-test".into(),
                line: 1,
                column: 1,
            },
        })
    }
    fn alias(
        parameter: usize,
        parameter_path: Vec<Projection>,
        result_path: Vec<Projection>,
    ) -> ReturnAlias {
        ReturnAlias {
            parameter,
            parameter_path,
            result_path,
        }
    }

    #[test]
    fn identity_literal_and_local_rebinding_have_distinct_origins() {
        let summaries = analyze_checked(vec![
            function(
                &["Int"],
                "Int",
                Some(vec![I::Return {
                    value: Some(param(0, "Int")),
                    span: None,
                }]),
            ),
            function(
                &["Int"],
                "Int",
                Some(vec![
                    I::Declare {
                        result: local(0, "Int"),
                        initial: Some(param(0, "Int")),
                    },
                    I::Literal {
                        result: local(1, "Int"),
                        value: Literal::Int(7),
                    },
                    I::Rebind {
                        span: None,
                        target: Place::Local(local(0, "Int").id),
                        value: local(1, "Int"),
                    },
                    I::Return {
                        value: Some(local(0, "Int")),
                        span: None,
                    },
                ]),
            ),
        ]);
        assert_eq!(summaries[0].returned, ReturnOrigins::parameter(0));
        assert_eq!(summaries[0].parameters[0].write, Effect::No);
        assert_eq!(summaries[0].parameters[0].escape, Effect::May);
        assert_eq!(summaries[1].returned, ReturnOrigins::fresh());
        assert_eq!(summaries[1].parameters[0].escape, Effect::No);
        assert_eq!(summaries[1].parameters[0].write, Effect::No);
    }
    #[test]
    fn returned_sharing_only_escapes_again_when_the_caller_exports_it() {
        let summaries = analyze_checked(vec![
            function(
                &["Bool"],
                "Unit",
                Some(vec![
                    call(3, vec![param(0, "Bool")], local(0, "Bool")),
                    I::Return {
                        value: None,
                        span: None,
                    },
                ]),
            ),
            function(
                &["Bool"],
                "Bool",
                Some(vec![
                    call(3, vec![param(0, "Bool")], local(0, "Bool")),
                    I::Return {
                        value: Some(local(0, "Bool")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Bool"],
                "Bool",
                Some(vec![
                    call(3, vec![param(0, "Bool")], local(0, "Bool")),
                    I::Object {
                        result: local(1, "Bool"),
                        expression: Rvalue::BoolCopy(local(0, "Bool")),
                    },
                    I::Return {
                        value: Some(local(1, "Bool")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Bool"],
                "Bool",
                Some(vec![I::Return {
                    value: Some(param(0, "Bool")),
                    span: None,
                }]),
            ),
        ]);
        assert_eq!(summaries[0].parameters[0].escape, Effect::No);
        assert_eq!(summaries[1].parameters[0].escape, Effect::May);
        assert_eq!(summaries[2].parameters[0].escape, Effect::No);
        assert!(summaries[2].returned.fresh);
        for summary in summaries {
            assert_eq!(summary.parameters[0].retained, Effect::No);
        }
    }
    #[test]
    fn fields_and_contained_references_survive_function_calls() {
        let summaries = analyze_checked(vec![
            function(
                &["Int"],
                "Int",
                Some(vec![
                    call(1, vec![param(0, "Int")], local(0, "Box")),
                    I::Object {
                        result: local(1, "Int"),
                        expression: Rvalue::Field {
                            owner: local(0, "Box"),
                            index: 0,
                        },
                    },
                    I::Return {
                        value: Some(local(1, "Int")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Int"],
                "Box",
                Some(vec![
                    I::Object {
                        result: local(0, "Box"),
                        expression: Rvalue::Aggregate(vec![param(0, "Int")]),
                    },
                    I::Return {
                        value: Some(local(0, "Box")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Box"],
                "Int",
                Some(vec![
                    I::Object {
                        result: local(0, "Int"),
                        expression: Rvalue::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                    },
                    I::Return {
                        value: Some(local(0, "Int")),
                        span: None,
                    },
                ]),
            ),
        ]);
        assert_eq!(
            summaries[0].returned.aliases,
            [alias(0, vec![], vec![])].into()
        );
        assert_eq!(
            summaries[1].returned.aliases,
            [alias(0, vec![], vec![Projection::Field(0)])].into()
        );
        assert_eq!(
            summaries[2].returned.aliases,
            [alias(0, vec![Projection::Field(0)], vec![])].into()
        );
        assert_eq!(summaries[2].parameters[0].read, Effect::May);
    }
    #[test]
    fn enum_payload_paths_are_composed_and_type_checked() {
        let projection = Projection::VariantField {
            variant: 1,
            field: 0,
        };
        let summaries = analyze_checked(vec![
            function(
                &["Int"],
                "Int",
                Some(vec![
                    call(1, vec![param(0, "Int")], local(0, "Maybe")),
                    call(2, vec![local(0, "Maybe")], local(1, "Int")),
                    I::Return {
                        value: Some(local(1, "Int")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Int"],
                "Maybe",
                Some(vec![
                    I::Object {
                        result: local(0, "Maybe"),
                        expression: Rvalue::Variant {
                            index: 1,
                            fields: vec![param(0, "Int")],
                        },
                    },
                    I::Return {
                        value: Some(local(0, "Maybe")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Maybe"],
                "Int",
                Some(vec![
                    I::Object {
                        result: local(0, "Int"),
                        expression: Rvalue::VariantField {
                            owner: param(0, "Maybe"),
                            variant: 1,
                            index: 0,
                        },
                    },
                    I::Return {
                        value: Some(local(0, "Int")),
                        span: None,
                    },
                ]),
            ),
        ]);
        assert_eq!(
            summaries[0].returned.aliases,
            [alias(0, vec![], vec![])].into()
        );
        assert_eq!(
            summaries[1].returned.aliases,
            [alias(0, vec![], vec![projection.clone()])].into()
        );
        assert_eq!(
            summaries[2].returned.aliases,
            [alias(0, vec![projection], vec![])].into()
        );
    }
    #[test]
    fn branch_returns_and_recursive_calls_reach_a_fixed_point() {
        let summaries = analyze_checked(vec![
            function(
                &["Bool", "Int", "Int"],
                "Int",
                Some(vec![I::If {
                    condition: param(0, "Bool"),
                    yes: Body {
                        instructions: vec![I::Return {
                            value: Some(param(1, "Int")),
                            span: None,
                        }],
                    },
                    no: Body {
                        instructions: vec![I::Return {
                            value: Some(param(2, "Int")),
                            span: None,
                        }],
                    },
                }]),
            ),
            function(
                &["Bool"],
                "Bool",
                Some(vec![I::If {
                    condition: param(0, "Bool"),
                    yes: Body {
                        instructions: vec![I::Return {
                            value: Some(param(0, "Bool")),
                            span: None,
                        }],
                    },
                    no: Body {
                        instructions: vec![
                            call(2, vec![param(0, "Bool")], local(0, "Bool")),
                            I::Return {
                                value: Some(local(0, "Bool")),
                                span: None,
                            },
                        ],
                    },
                }]),
            ),
            function(
                &["Bool"],
                "Bool",
                Some(vec![
                    call(1, vec![param(0, "Bool")], local(0, "Bool")),
                    I::Return {
                        value: Some(local(0, "Bool")),
                        span: None,
                    },
                ]),
            ),
        ]);
        assert_eq!(
            summaries[0].returned.aliases,
            [alias(1, vec![], vec![]), alias(2, vec![], vec![])].into()
        );
        assert_eq!(summaries[0].may_diverge, Effect::No);
        for s in &summaries[1..] {
            assert_eq!(s.returned, ReturnOrigins::parameter(0));
            assert_eq!(s.parameters[0].write, Effect::No);
            assert_eq!(s.may_diverge, Effect::May);
        }
    }
    #[test]
    fn mutations_and_retention_propagate_through_calls() {
        let summaries = analyze_checked(vec![
            function(
                &["Bool"],
                "Unit",
                Some(vec![
                    call(1, vec![param(0, "Bool")], local(0, "Unit")),
                    I::Return {
                        value: None,
                        span: None,
                    },
                ]),
            ),
            function(
                &["Bool"],
                "Unit",
                Some(vec![
                    I::Literal {
                        result: local(0, "Bool"),
                        value: Literal::Bool(true),
                    },
                    I::StoreBool {
                        target: param(0, "Bool"),
                        value: local(0, "Bool"),
                    },
                    I::Return {
                        value: None,
                        span: None,
                    },
                ]),
            ),
            function(
                &["Box", "Int"],
                "Unit",
                Some(vec![
                    I::Rebind {
                        span: None,
                        target: Place::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                        value: param(1, "Int"),
                    },
                    I::Return {
                        value: None,
                        span: None,
                    },
                ]),
            ),
        ]);
        assert_eq!(summaries[0].parameters[0].write, Effect::May);
        assert_eq!(summaries[0].parameters[0].escape, Effect::No);
        assert_eq!(summaries[2].parameters[0].write, Effect::May);
        assert_eq!(summaries[2].parameters[1].escape, Effect::May);
        assert_eq!(summaries[2].parameters[1].retained, Effect::May);
    }
    #[test]
    fn opaque_names_are_not_trusted_and_unknown_global_writes_invalidate_fields() {
        let mut opaque = function(&[], "Unit", None);
        opaque.name = "copy".into();
        let summaries = analyze_checked(vec![
            function(
                &["Box"],
                "Int",
                Some(vec![
                    call(1, vec![], local(0, "Unit")),
                    I::Object {
                        result: local(1, "Int"),
                        expression: Rvalue::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                    },
                    I::Return {
                        value: Some(local(1, "Int")),
                        span: None,
                    },
                ]),
            ),
            opaque,
        ]);
        assert_eq!(summaries[1].source, SummarySource::Opaque);
        assert_eq!(summaries[0].external_write, Effect::Unknown);
        assert!(summaries[0].returned.unknown);
        assert_eq!(summaries[0].parameters[0].escape, Effect::Unknown);
    }
    #[test]
    fn overwritten_heap_fields_are_not_reported_as_old_aliases() {
        let summaries = analyze_checked(vec![function(
            &["Box", "Int"],
            "Int",
            Some(vec![
                I::Rebind {
                    span: None,
                    target: Place::Field {
                        owner: param(0, "Box"),
                        index: 0,
                    },
                    value: param(1, "Int"),
                },
                I::Object {
                    result: local(0, "Int"),
                    expression: Rvalue::Field {
                        owner: param(0, "Box"),
                        index: 0,
                    },
                },
                I::Return {
                    value: Some(local(0, "Int")),
                    span: None,
                },
            ]),
        )]);
        assert!(summaries[0].returned.unknown);
        assert_eq!(summaries[0].parameters[1].escape, Effect::Unknown);
    }
    #[test]
    fn callee_projections_respect_heap_changes_but_old_snapshots_survive() {
        let summaries = analyze_checked(vec![
            function(
                &["Box", "Int"],
                "Int",
                Some(vec![
                    I::Rebind {
                        span: None,
                        target: Place::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                        value: param(1, "Int"),
                    },
                    call(2, vec![param(0, "Box")], local(0, "Int")),
                    I::Return {
                        value: Some(local(0, "Int")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Box", "Int"],
                "Int",
                Some(vec![
                    I::Object {
                        result: local(0, "Int"),
                        expression: Rvalue::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                    },
                    I::Rebind {
                        span: None,
                        target: Place::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                        value: param(1, "Int"),
                    },
                    I::Return {
                        value: Some(local(0, "Int")),
                        span: None,
                    },
                ]),
            ),
            function(
                &["Box"],
                "Int",
                Some(vec![
                    I::Object {
                        result: local(0, "Int"),
                        expression: Rvalue::Field {
                            owner: param(0, "Box"),
                            index: 0,
                        },
                    },
                    I::Return {
                        value: Some(local(0, "Int")),
                        span: None,
                    },
                ]),
            ),
        ]);
        assert!(summaries[0].returned.unknown);
        assert_eq!(
            summaries[1].returned.aliases,
            [alias(0, vec![Projection::Field(0)], vec![])].into()
        );
    }
    #[test]
    fn loop_join_keeps_both_initial_and_rebound_origins() {
        let summaries = analyze_checked(vec![function(
            &["Bool", "Int", "Int"],
            "Int",
            Some(vec![
                I::Declare {
                    result: local(0, "Int"),
                    initial: Some(param(1, "Int")),
                },
                I::Loop {
                    id: 0,
                    body: Body {
                        instructions: vec![
                            I::If {
                                condition: param(0, "Bool"),
                                yes: Body {
                                    instructions: vec![I::Break(0)],
                                },
                                no: Body::default(),
                            },
                            I::Rebind {
                                span: None,
                                target: Place::Local(local(0, "Int").id),
                                value: param(2, "Int"),
                            },
                            I::Continue(0),
                        ],
                    },
                },
                I::Return {
                    value: Some(local(0, "Int")),
                    span: None,
                },
            ]),
        )]);
        assert_eq!(
            summaries[0].returned.aliases,
            [alias(1, vec![], vec![]), alias(2, vec![], vec![])].into()
        );
        assert_eq!(summaries[0].may_diverge, Effect::May);
    }
    #[test]
    fn provenance_widens_and_invalid_summaries_are_rejected() {
        let mut value = ReturnOrigins::parameter(0);
        for _ in 0..=MAX_PATH {
            value = value.project(Projection::Field(0));
        }
        assert_eq!(value, ReturnOrigins::unknown());
        value.join(&ReturnOrigins::fresh());
        assert_eq!(value, ReturnOrigins::unknown());
        let mut summary = Summary::inferred(1);
        summary.returned = ReturnOrigins::parameter(0);
        assert!(summary.validate(1).unwrap_err().contains("return escape"));
        summary.parameters[0].escape = Effect::May;
        summary.validate(1).unwrap();
        assert!(summary.validate(0).is_err());
        summary.returned.aliases = [alias(2, vec![], vec![])].into();
        assert!(
            summary
                .validate(1)
                .unwrap_err()
                .contains("return parameter")
        );
        let types = [("Int".into(), crate::hir::Type::Primitive)].into();
        let mut invalid_path = Summary::inferred(1);
        invalid_path.parameters[0].escape = Effect::May;
        invalid_path.returned.aliases = [alias(0, vec![Projection::Field(0)], vec![])].into();
        assert!(
            invalid_path
                .validate_signature(&["Int".into()], "Int", &types, true)
                .unwrap_err()
                .contains("projection")
        );
        assert!(
            Summary::inferred(0)
                .validate_signature(&[], "Int", &types, false)
                .unwrap_err()
                .contains("opaque body")
        );
        let mut opaque = Summary::unknown(1, SummarySource::Opaque);
        opaque.parameters[0].write = Effect::No;
        assert!(opaque.validate(1).unwrap_err().contains("must be unknown"));
    }
}
