//! Access-path contracts. Qualifiers follow shared values, not every alias of
//! the underlying allocation. Known local stores use an allocation-site graph;
//! this pass never treats an unverified external implementation as readonly.
use crate::{
    ast::{AccessMode, Span},
    effects::{Effect, Projection},
    hir::{AccessInfo, Place, Rvalue, Value, ValueId},
    lir::{self, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
mod diagnostic;
mod heap;
use heap::{Heap, Reference};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AccessPaths(BTreeMap<Vec<Projection>, BTreeSet<AccessInfo>>, bool);
impl AccessPaths {
    fn diagnostic_cost(&self) -> usize {
        1 + self
            .0
            .iter()
            .map(|(path, sources)| path.len() + sources.len())
            .sum::<usize>()
    }
    fn root(info: AccessInfo) -> Self {
        Self([(vec![], [info].into())].into(), false)
    }
    fn join(&mut self, other: &Self) {
        self.1 |= other.1;
        for (path, sources) in &other.0 {
            self.0
                .entry(path.clone())
                .or_default()
                .extend(sources.iter().cloned());
        }
        // A restriction on an ancestor includes that source on descendants.
        // Canonicalization also makes path widening stable on recursive calls.
        let snapshot = self.0.clone();
        for (path, sources) in &mut self.0 {
            sources.retain(|source| {
                !(0..path.len()).any(|length| {
                    snapshot
                        .get(&path[..length])
                        .is_some_and(|ancestors| ancestors.contains(source))
                })
            });
        }
        self.0.retain(|_, sources| !sources.is_empty());
        if self.0.len() > 64 || self.0.keys().any(|p| p.len() > 16) {
            *self = self.collapse();
        }
    }
    fn project_root(&self) -> Self {
        Self(
            self.0
                .get(&vec![])
                .map(|sources| (vec![], sources.clone()))
                .into_iter()
                .collect(),
            self.1,
        )
    }
    fn collapse(&self) -> Self {
        let sources: BTreeSet<_> = self.0.values().flatten().cloned().collect();
        if sources.is_empty() {
            Self::default()
        } else {
            Self([(vec![], sources)].into(), true)
        }
    }
    fn contain(&self, projection: Projection) -> Self {
        let mut result = Self::default();
        for (path, sources) in &self.0 {
            let mut path = path.clone();
            path.insert(0, projection.clone());
            result.join(&Self([(path, sources.clone())].into(), self.1));
        }
        result
    }
    fn project(&self, projection: Projection) -> Self {
        let mut result = Self::default();
        for (path, sources) in &self.0 {
            if path.is_empty() {
                result.join(&Self([(vec![], sources.clone())].into(), self.1));
            } else if path.first() == Some(&projection) {
                result.join(&Self(
                    [(path[1..].to_vec(), sources.clone())].into(),
                    self.1,
                ));
            }
        }
        result
    }
    fn check(&self, span: &Span, operation: &str, unknown: bool) -> Result<(), String> {
        if let Some((path, sources)) = self.0.first_key_value()
            && let Some(source) = sources.first()
        {
            let reason = if unknown || self.1 {
                "cannot prove readonly access is preserved"
            } else {
                "cannot modify readonly access"
            };
            let route = if path.is_empty() {
                "the value or a derived alias".into()
            } else {
                format!("contained access {path:?}")
            };
            return Err(format!(
                "{}\n{}\n{}\nthrough {route}",
                span.error(format!("{reason}: {operation}")),
                source
                    .declaration
                    .error(format!("readonly access originates at '{}'", source.name)),
                source.annotation.span.error("readonly modifier is here")
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub returned: AccessPaths,
    pub retained: AccessPaths,
}
impl Summary {
    pub fn validate(
        &self,
        result: &str,
        types: &BTreeMap<String, crate::hir::Type>,
    ) -> Result<(), String> {
        for (path, sources) in &self.returned.0 {
            crate::effects::project_type(result, path, types)?;
            if path.len() > 16
                || sources.is_empty()
                || sources
                    .iter()
                    .any(|s| s.annotation.mode != AccessMode::Readonly)
            {
                return Err("invalid returned access restriction".into());
            }
        }
        for (path, sources) in &self.retained.0 {
            if !path.is_empty()
                || sources.is_empty()
                || sources
                    .iter()
                    .any(|s| s.annotation.mode != AccessMode::Readonly)
            {
                return Err("invalid retained access restriction".into());
            }
        }
        if self.returned.0.len() > 64 {
            return Err("unbounded access restrictions".into());
        }
        Ok(())
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    values: BTreeMap<ValueId, Reference>,
    objects: Heap,
    /// Unknown destinations and exposed objects may have untracked aliases.
    /// Their retained restrictions still widen to all aggregates/projections.
    heap: AccessPaths,
}
fn aggregate(ty: &str) -> bool {
    !matches!(ty, "Int" | "Bool" | "Char" | "Unit")
}
impl State {
    fn diagnostic_cost(&self) -> usize {
        self.heap.diagnostic_cost()
            + self.objects.diagnostic_cost()
            + self
                .values
                .values()
                .map(Reference::diagnostic_cost)
                .sum::<usize>()
    }
    fn reference(&self, value: &Value) -> Reference {
        self.values
            .get(&value.id)
            .cloned()
            .unwrap_or_else(Reference::unknown)
    }
    fn get(&self, value: &Value) -> AccessPaths {
        let mut access = self.objects.read(&self.reference(value));
        if aggregate(&value.ty) {
            access.join(&self.heap);
        }
        access
    }
    fn project(&self, owner: &Reference, projection: Projection) -> Reference {
        let mut value = self.objects.project(owner, projection);
        value.access.join(&self.heap);
        value
    }
    fn join(&mut self, other: &Self) -> bool {
        let old = self.clone();
        self.heap.join(&other.heap);
        self.objects.join(&other.objects);
        for (id, value) in &other.values {
            self.values.entry(id.clone()).or_default().join(value);
        }
        *self != old
    }
}
fn operation(
    op: &Operation,
    state: &mut State,
    result: &mut Summary,
    program: &lir::Program,
    summaries: &[Summary],
    declarations: &BTreeMap<ValueId, AccessInfo>,
    check: bool,
) -> Result<(), diagnostic::Failure> {
    match op {
        Operation::AnnotateAccess { value, info } => {
            if info.annotation.mode == AccessMode::Readonly {
                state
                    .values
                    .entry(value.id.clone())
                    .or_default()
                    .access
                    .join(&AccessPaths::root(info.clone()));
            }
        }
        Operation::Literal { result, .. } => {
            state
                .values
                .insert(result.id.clone(), Reference::fresh(result.id.clone()));
        }
        Operation::Declare { result, initial } => {
            let mut value = initial
                .as_ref()
                .map(|v| state.reference(v))
                .unwrap_or_default();
            if let Some(info) = declarations.get(&result.id) {
                value.access.join(&AccessPaths::root(info.clone()));
            }
            state.values.insert(result.id.clone(), value);
        }
        Operation::Object { result, expression } => {
            let value = match expression {
                Rvalue::Share(value) => state.reference(value),
                Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. } => {
                    Reference::fresh(result.id.clone())
                }
                Rvalue::Field { owner, index } => {
                    state.project(&state.reference(owner), Projection::Field(*index))
                }
                Rvalue::VariantField {
                    owner,
                    variant,
                    index,
                } => state.project(
                    &state.reference(owner),
                    Projection::VariantField {
                        variant: *variant,
                        field: *index,
                    },
                ),
                Rvalue::Aggregate(fields) | Rvalue::Variant { fields, .. } => {
                    let value = Reference::fresh(result.id.clone());
                    for (index, field) in fields.iter().enumerate() {
                        let projection = match expression {
                            Rvalue::Variant { index: variant, .. } => Projection::VariantField {
                                variant: *variant,
                                field: index,
                            },
                            _ => Projection::Field(index),
                        };
                        state
                            .objects
                            .store(&value, projection, &state.reference(field));
                    }
                    value
                }
            };
            state.values.insert(result.id.clone(), value);
        }
        Operation::Rebind {
            target,
            value,
            span,
        } => {
            let mut value = state.reference(value);
            match target {
                Place::Local(id) => {
                    if let Some(info) = declarations.get(id) {
                        value.access.join(&AccessPaths::root(info.clone()));
                    }
                    state.values.insert(id.clone(), value);
                }
                Place::Field { owner, index } => {
                    let owner_access = state.get(owner).project_root();
                    if check {
                        owner_access
                            .check(
                                span.as_ref().unwrap_or(&Span {
                                    file: "<IR>".into(),
                                    line: 1,
                                    column: 1,
                                }),
                                "field or tuple element rebinding",
                                false,
                            )
                            .map_err(|error| {
                                diagnostic::Failure::new(error, owner, &owner_access, false)
                            })?;
                    }
                    if state.objects.store(
                        &state.reference(owner),
                        Projection::Field(*index),
                        &value,
                    ) {
                        let access = state.objects.read(&value).collapse();
                        state.heap.join(&access);
                        result.retained.join(&access);
                    }
                }
            }
        }
        Operation::Call(call) => {
            let callee = &program.functions[call.function];
            let references: Vec<_> = call.args.iter().map(|v| state.reference(v)).collect();
            let args: Vec<_> = call.args.iter().map(|v| state.get(v)).collect();
            for (index, (actual, effect)) in args.iter().zip(&callee.effects.parameters).enumerate()
            {
                if check && (effect.write != Effect::No || effect.structural != Effect::No) {
                    // Parameter effects do not identify which contained member
                    // is written. A contained-only restriction is conservative.
                    let unknown = effect.write == Effect::Unknown
                        || effect.structural == Effect::Unknown
                        || !actual.0.contains_key(&vec![]);
                    actual
                        .check(
                            &call.span,
                            &format!("argument {} of '{}'", index + 1, callee.name),
                            unknown,
                        )
                        .map_err(|error| {
                            diagnostic::Failure::new(error, &call.args[index], actual, unknown)
                        })?;
                }
                if effect.retained != Effect::No {
                    state.objects.expose(&references[index]);
                    state.heap.join(&actual.collapse());
                    result.retained.join(&actual.collapse());
                }
            }
            let restriction = &summaries[call.function];
            state.heap.join(&restriction.retained);
            result.retained.join(&restriction.retained);
            let mut value = if callee.effects.returned.fresh {
                Reference::fresh(call.result.id.clone())
            } else {
                Reference::default()
            };
            value.access.join(&restriction.returned);
            if callee.effects.returned.unknown {
                value.unknown = true;
                for (actual, reference) in args.iter().zip(&references) {
                    value.access.join(&actual.collapse());
                    state.objects.expose(reference);
                }
                value.access.join(&state.heap);
            } else {
                // Resolve root aliases first, then populate members of the new
                // result. Descendant identities remain shared with arguments.
                for alias in callee
                    .effects
                    .returned
                    .aliases
                    .iter()
                    .filter(|a| a.result_path.is_empty())
                {
                    let mut actual = references[alias.parameter].clone();
                    for projection in &alias.parameter_path {
                        actual = state.project(&actual, projection.clone());
                    }
                    value.join(&actual);
                }
                for alias in callee
                    .effects
                    .returned
                    .aliases
                    .iter()
                    .filter(|a| !a.result_path.is_empty())
                {
                    let mut actual = references[alias.parameter].clone();
                    for projection in &alias.parameter_path {
                        actual = state.project(&actual, projection.clone());
                    }
                    if state
                        .objects
                        .store_path(&value, &alias.result_path, &actual)
                    {
                        let access = state.objects.read(&actual).collapse();
                        state.heap.join(&access);
                        result.retained.join(&access);
                    }
                }
            }
            state.values.insert(call.result.id.clone(), value);
        }
        Operation::StoreBool { target, .. } => {
            // Compiler-created snapshots are fresh. A restricted source here
            // would indicate an invalid lowering rather than a user call.
            if check {
                let access = state.get(target);
                access
                    .check(
                        &Span {
                            file: "<IR>".into(),
                            line: 1,
                            column: 1,
                        },
                        "Boolean store",
                        false,
                    )
                    .map_err(|error| diagnostic::Failure::new(error, target, &access, false))?;
            }
        }
        Operation::Release(_)
        | Operation::DescribeBinding { .. }
        | Operation::AnnotateBinding { .. }
        | Operation::AnnotateEscape { .. } => {}
    }
    Ok(())
}
fn analyze_body(
    body: &lir::Function,
    program: &lir::Program,
    summaries: &[Summary],
    check: bool,
) -> Result<Summary, String> {
    let mut declarations = BTreeMap::new();
    for block in &body.blocks {
        for op in &block.instructions {
            if let Operation::AnnotateAccess { value, info } = op
                && info.annotation.mode == AccessMode::Readonly
            {
                declarations.insert(value.id.clone(), info.clone());
            }
        }
    }
    let mut result = Summary::default();
    let mut entries = vec![None::<State>; body.blocks.len()];
    entries[0] = Some(State::default());
    let mut pending = VecDeque::from([0]);
    while let Some(index) = pending.pop_front() {
        let mut state = entries[index].as_ref().unwrap().clone();
        let block = &body.blocks[index];
        for (position, op) in block.instructions.iter().enumerate() {
            operation(
                op,
                &mut state,
                &mut result,
                program,
                summaries,
                &declarations,
                check,
            )
            .map_err(|failure| failure.explain(body, program, summaries, (index, position)))?;
        }
        let next = match &block.terminator {
            Terminator::Jump(target) => vec![target.0],
            Terminator::Branch { yes, no, .. } => vec![yes.0, no.0],
            Terminator::Return { value, .. } => {
                if let Some(value) = value {
                    result.returned.join(&state.get(value));
                }
                vec![]
            }
            Terminator::Unreachable => vec![],
        };
        for next in next {
            let changed = match &mut entries[next] {
                Some(entry) => entry.join(&state),
                slot @ None => {
                    *slot = Some(state.clone());
                    true
                }
            };
            if changed {
                pending.push_back(next);
            }
        }
    }
    Ok(result)
}
/// Solve the propagation of explicit restrictions through calls before checking
/// mutations. The effect summaries independently describe unannotated callees.
pub(crate) fn analyze(program: &lir::Program) -> Result<Vec<Summary>, String> {
    let mut summaries = vec![Summary::default(); program.functions.len()];
    let has_readonly = program
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .chain(std::iter::once(&program.main))
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.instructions)
        .any(|op| {
            matches!(op, Operation::AnnotateAccess { info, .. }
                if info.annotation.mode == AccessMode::Readonly)
        });
    if !has_readonly {
        return Ok(summaries);
    }

    loop {
        let mut changed = false;
        for (id, function) in program.functions.iter().enumerate() {
            if let Some(body) = &function.body {
                let summary = analyze_body(body, program, &summaries, false)?;
                if summaries[id] != summary {
                    summaries[id] = summary;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for function in &program.functions {
        if let Some(body) = &function.body {
            analyze_body(body, program, &summaries, true).map_err(|error| {
                format!(
                    "{error}\nwhile checking access contracts in '{}'",
                    function.name
                )
            })?;
        }
    }
    analyze_body(&program.main, program, &summaries, true)?;
    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> AccessInfo {
        AccessInfo {
            name: "x".into(),
            declaration: Span {
                file: "test".into(),
                line: 1,
                column: 1,
            },
            annotation: crate::ast::AccessAnnotation {
                mode: AccessMode::Readonly,
                span: Span {
                    file: "test".into(),
                    line: 1,
                    column: 1,
                },
            },
        }
    }
    #[test]
    fn ancestor_restrictions_dominate_descendants_and_widening_is_stable() {
        let root = AccessPaths::root(info());
        let mut nested = root.contain(Projection::Field(0));
        assert!(nested.project(Projection::Field(1)).0.is_empty());
        assert_eq!(nested.project(Projection::Field(0)), root);
        nested.join(&root);
        assert_eq!(nested, root);
        let mut wide = root.clone();
        for _ in 0..17 {
            wide = wide.contain(Projection::Field(0));
        }
        assert_eq!(wide.0, root.0);
        assert!(wide.1);
        let mut combined = wide.clone();
        combined.join(&wide.contain(Projection::Field(0)));
        assert_eq!(combined, wide);
    }
    #[test]
    fn returned_and_retained_restrictions_have_valid_paths_and_readonly_sources() {
        let types = [("Int".into(), crate::hir::Type::Primitive)].into();
        let mut summary = Summary {
            returned: AccessPaths::root(info()),
            retained: AccessPaths::default(),
        };
        assert!(summary.validate("Int", &types).is_ok());
        summary.returned = summary.returned.contain(Projection::Field(0));
        assert!(summary.validate("Int", &types).is_err());
        summary.returned = AccessPaths::default();
        let mut mutable = info();
        mutable.annotation.mode = AccessMode::Mutable;
        summary.retained = AccessPaths::root(mutable);
        assert!(summary.validate("Int", &types).is_err());
    }
}
