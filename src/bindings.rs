//! Binding identity contracts are independent of object mutation. Dataflow uses
//! possible initialization, so joins and loop backedges cannot initialize a
//! fixed binding twice. A new declaration resets the binding on each iteration.
use crate::{
    ast::{BindingMode, Span},
    hir::{BindingInfo, Place, ValueId},
    lir::{Function, Operation, Terminator},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) fn rebinding_error(info: &BindingInfo, span: &Span) -> String {
    format!(
        "{}\n{}\n{}",
        span.error(format!(
            "cannot rebind fixed variable '{}'; the binding may already be initialized",
            info.name
        )),
        info.declaration.error("binding declared here"),
        info.annotation.span.error("fixed modifier is here")
    )
}

pub(crate) fn validate(function: &Function) -> Result<(), String> {
    let mut annotations = BTreeMap::new();
    for block in &function.blocks {
        for op in &block.instructions {
            if let Operation::AnnotateBinding { value, info } = op {
                if !matches!(value.id, ValueId::Local(_)) {
                    return Err("binding annotation requires a local identity".into());
                }
                if annotations.insert(value.id.clone(), info).is_some() {
                    return Err("duplicate IR binding annotation".into());
                }
            }
        }
    }
    let annotations: BTreeMap<_, _> = annotations
        .into_iter()
        .filter(|(_, info)| info.annotation.mode == BindingMode::Fixed)
        .collect();
    if annotations.is_empty() {
        return Ok(());
    }
    let mut entries = vec![None::<BTreeSet<ValueId>>; function.blocks.len()];
    entries[0] = Some(BTreeSet::new());
    let mut pending = VecDeque::from([0]);
    while let Some(index) = pending.pop_front() {
        let mut initialized = entries[index].as_ref().unwrap().clone();
        let block = &function.blocks[index];
        for op in &block.instructions {
            match op {
                Operation::Declare { result, initial } if annotations.contains_key(&result.id) => {
                    if initial.is_some() {
                        initialized.insert(result.id.clone());
                    } else {
                        initialized.remove(&result.id);
                    }
                }
                Operation::Object { result, .. } | Operation::Literal { result, .. }
                    if annotations.contains_key(&result.id) =>
                {
                    initialized.insert(result.id.clone());
                }
                Operation::Call(call) if annotations.contains_key(&call.result.id) => {
                    initialized.insert(call.result.id.clone());
                }
                Operation::Rebind {
                    target: Place::Local(id),
                    span,
                    ..
                } => {
                    if initialized.contains(id)
                        && let Some(info) = annotations.get(id)
                        && info.annotation.mode == BindingMode::Fixed
                    {
                        return Err(rebinding_error(
                            info,
                            span.as_ref().unwrap_or(&info.declaration),
                        ));
                    }
                    if annotations.contains_key(id) {
                        initialized.insert(id.clone());
                    }
                }
                // Releasing an object does not undo initialization of a source
                // binding. A later declaration, not a release, resets it.
                _ => {}
            }
        }
        let successors = match &block.terminator {
            Terminator::Jump(target) => vec![target.0],
            Terminator::Branch { yes, no, .. } => vec![yes.0, no.0],
            _ => vec![],
        };
        for next in successors {
            let changed = match &mut entries[next] {
                Some(entry) => {
                    let old_len = entry.len();
                    entry.extend(initialized.iter().cloned());
                    entry.len() != old_len
                }
                slot @ None => {
                    *slot = Some(initialized.clone());
                    true
                }
            };
            if changed {
                pending.push_back(next);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::BindingAnnotation,
        hir::Value,
        lir::{Block, BlockId},
    };
    fn span(line: u32) -> Span {
        Span {
            file: "binding-test".into(),
            line,
            column: 1,
        }
    }
    fn local(id: usize) -> Value {
        Value {
            id: ValueId::Local(id),
            ty: "Int".into(),
        }
    }
    fn annotation(value: Value) -> Operation {
        Operation::AnnotateBinding {
            value,
            info: BindingInfo {
                name: "x".into(),
                declaration: span(1),
                annotation: BindingAnnotation {
                    mode: BindingMode::Fixed,
                    span: span(1),
                },
            },
        }
    }
    fn body(instructions: Vec<Operation>) -> Function {
        Function {
            slots: vec![local(0), local(1)],
            blocks: vec![Block {
                instructions,
                terminator: Terminator::Return {
                    span: None,
                    value: None,
                    cleanup: vec![],
                },
            }],
        }
    }
    #[test]
    fn releases_do_not_erase_fixed_initialization() {
        let function = body(vec![
            Operation::Declare {
                result: local(0),
                initial: Some(local(1)),
            },
            annotation(local(0)),
            Operation::Release(vec![local(0).id]),
            Operation::Rebind {
                target: Place::Local(local(0).id),
                value: local(1),
                span: Some(span(2)),
            },
        ]);
        let error = validate(&function).unwrap_err();
        assert!(
            error.contains("binding-test:2:") && error.contains("cannot rebind fixed variable")
        );
    }
    #[test]
    fn duplicate_and_nonlocal_annotations_are_rejected() {
        assert!(
            validate(&body(vec![annotation(local(0)), annotation(local(0))]))
                .unwrap_err()
                .contains("duplicate")
        );
        let parameter = Value {
            id: ValueId::Parameter(0),
            ty: "Int".into(),
        };
        assert!(
            validate(&body(vec![annotation(parameter)]))
                .unwrap_err()
                .contains("local identity")
        );
    }
    #[test]
    fn a_later_declaration_resets_initialization_on_a_backedge() {
        let mut function = body(vec![
            Operation::Declare {
                result: local(0),
                initial: None,
            },
            annotation(local(0)),
            Operation::Rebind {
                target: Place::Local(local(0).id),
                value: local(1),
                span: Some(span(2)),
            },
        ]);
        function.blocks[0].terminator = Terminator::Jump(BlockId(0));
        assert!(validate(&function).is_ok());
    }
}
