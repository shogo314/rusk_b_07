//! Structural and type invariants checked before any target-specific lowering.
use crate::hir::*;
use std::collections::{BTreeMap, BTreeSet};
struct Check<'a> {
    program: &'a Program,
    values: BTreeMap<ValueId, String>,
    loops: BTreeSet<LoopId>,
    bindings: BTreeSet<ValueId>,
    access_bindings: BTreeSet<ValueId>,
    escape_parameters: BTreeSet<usize>,
    result: &'a str,
    caller_location: bool,
}
impl Check<'_> {
    fn ty(&self, ty: &str) -> Result<(), String> {
        if self.program.types.contains_key(ty) {
            Ok(())
        } else {
            Err(format!("unknown IR type {ty}"))
        }
    }
    fn same(&self, a: &str, b: &str) -> Result<(), String> {
        if a == b {
            Ok(())
        } else {
            Err(format!("IR type mismatch: {a} / {b}"))
        }
    }
    fn value(&self, value: &Value) -> Result<(), String> {
        self.ty(&value.ty)?;
        let ty = self
            .values
            .get(&value.id)
            .ok_or_else(|| format!("undefined IR value {:?}", value.id))?;
        self.same(ty, &value.ty)
    }
    fn define(&mut self, value: &Value) -> Result<(), String> {
        self.ty(&value.ty)?;
        if !matches!(value.id, ValueId::Local(_)) {
            return Err("IR definition requires a local ID".into());
        }
        if self
            .values
            .insert(value.id.clone(), value.ty.clone())
            .is_some()
        {
            return Err("duplicate IR value ID".into());
        }
        Ok(())
    }
    fn field(&self, owner: &Value, index: usize) -> Result<&str, String> {
        self.value(owner)?;
        self.program.types[&owner.ty]
            .fields()
            .and_then(|fields| fields.get(index))
            .map(|(_, t)| t.as_str())
            .ok_or_else(|| "invalid IR field".into())
    }
    fn body(&mut self, body: &Body) -> Result<(), String> {
        for op in &body.instructions {
            match op {
                Instruction::Literal { result, value } => {
                    let ty = match value {
                        Literal::Unit => "Unit",
                        Literal::Int(_) => "Int",
                        Literal::Bool(_) => "Bool",
                        Literal::Char(_) => "Char",
                        Literal::Str(_) => "Str",
                    };
                    self.same(&result.ty, ty)?;
                    self.define(result)?;
                }
                Instruction::Object { result, expression } => {
                    match expression {
                        Rvalue::Share(value) => {
                            self.value(value)?;
                            self.same(&result.ty, &value.ty)?;
                        }
                        Rvalue::Field { owner, index } => {
                            self.same(&result.ty, self.field(owner, *index)?)?
                        }
                        Rvalue::Aggregate(values) => {
                            let fields = self
                                .program
                                .types
                                .get(&result.ty)
                                .and_then(Type::fields)
                                .ok_or("IR aggregate requires a record type")?;
                            if fields.len() != values.len() {
                                return Err("IR aggregate arity mismatch".into());
                            }
                            for ((_, ty), v) in fields.iter().zip(values) {
                                self.value(v)?;
                                self.same(ty, &v.ty)?;
                            }
                        }
                        Rvalue::BoolCopy(value) => {
                            self.value(value)?;
                            self.same(&value.ty, "Bool")?;
                            self.same(&result.ty, "Bool")?;
                        }
                        Rvalue::Variant { index, fields } => {
                            let Type::Enum(variants) = &self.program.types[&result.ty] else {
                                return Err("IR variant requires enum".into());
                            };
                            let (_, types) =
                                variants.get(*index).ok_or("invalid IR variant index")?;
                            if types.len() != fields.len() {
                                return Err("IR variant arity mismatch".into());
                            }
                            for (ty, v) in types.iter().zip(fields) {
                                self.value(v)?;
                                self.same(ty, &v.ty)?;
                            }
                        }
                        Rvalue::VariantTest { owner, index }
                        | Rvalue::VariantField {
                            owner,
                            variant: index,
                            ..
                        } => {
                            self.value(owner)?;
                            let Type::Enum(variants) = &self.program.types[&owner.ty] else {
                                return Err("IR variant operation requires enum".into());
                            };
                            let (_, fields) =
                                variants.get(*index).ok_or("invalid IR variant index")?;
                            if let Rvalue::VariantField { index, .. } = expression {
                                self.same(
                                    &result.ty,
                                    fields.get(*index).ok_or("invalid IR variant field")?,
                                )?;
                            } else {
                                self.same(&result.ty, "Bool")?;
                            }
                        }
                    }
                    self.define(result)?;
                }
                Instruction::Call(call) => {
                    if call.inherit_location && !self.caller_location {
                        return Err("caller location is not available in main".into());
                    }
                    let function = self
                        .program
                        .functions
                        .get(call.function)
                        .ok_or("invalid IR function ID")?;
                    if function.params.len() != call.args.len() {
                        return Err("IR call arity mismatch".into());
                    }
                    for (ty, value) in function.params.iter().zip(&call.args) {
                        self.value(value)?;
                        self.same(ty, &value.ty)?;
                    }
                    self.same(&function.result, &call.result.ty)?;
                    self.define(&call.result)?;
                }
                Instruction::Declare { result, initial } => {
                    if let Some(initial) = initial {
                        self.value(initial)?;
                        self.same(&result.ty, &initial.ty)?;
                    }
                    self.define(result)?;
                }
                Instruction::Rebind { target, value, .. } => {
                    self.value(value)?;
                    let ty = match target {
                        Place::Local(id) => self
                            .values
                            .get(id)
                            .map(String::as_str)
                            .ok_or("invalid IR rebind target")?,
                        Place::Field { owner, index } => self.field(owner, *index)?,
                    };
                    self.same(ty, &value.ty)?;
                }
                Instruction::DescribeBinding { value, .. } => {
                    self.value(value)?;
                    if !matches!(value.id, ValueId::Local(_)) {
                        return Err("source binding requires a local identity".into());
                    }
                }
                Instruction::AnnotateEscape { parameter, .. } => {
                    if !self.values.contains_key(&ValueId::Parameter(*parameter))
                        || !self.escape_parameters.insert(*parameter)
                    {
                        return Err("invalid or duplicate IR escape annotation".into());
                    }
                }
                Instruction::AnnotateAccess { value, .. } => {
                    self.value(value)?;
                    if !matches!(value.id, ValueId::Local(_))
                        || !self.access_bindings.insert(value.id.clone())
                    {
                        return Err("invalid or duplicate IR access annotation".into());
                    }
                }
                Instruction::AnnotateBinding { value, .. } => {
                    self.value(value)?;
                    if !matches!(value.id, ValueId::Local(_)) {
                        return Err("binding annotation requires a local identity".into());
                    }
                    if !self.bindings.insert(value.id.clone()) {
                        return Err("duplicate IR binding annotation".into());
                    }
                }
                Instruction::StoreBool { target, value } => {
                    self.value(target)?;
                    self.value(value)?;
                    self.same(&target.ty, "Bool")?;
                    self.same(&value.ty, "Bool")?;
                }
                Instruction::Release(values) => {
                    for id in values {
                        if !self.values.contains_key(id) {
                            return Err("release of undefined IR value".into());
                        }
                    }
                }
                Instruction::Scope(body) => self.body(body)?,
                Instruction::If { condition, yes, no } => {
                    self.value(condition)?;
                    self.same(&condition.ty, "Bool")?;
                    self.body(yes)?;
                    self.body(no)?;
                }
                Instruction::Loop { id, body } => {
                    if !self.loops.insert(*id) {
                        return Err("duplicate active IR loop ID".into());
                    }
                    self.body(body)?;
                    self.loops.remove(id);
                }
                Instruction::Break(id) | Instruction::Continue(id) => {
                    if !self.loops.contains(id) {
                        return Err("invalid IR loop target".into());
                    }
                }
                Instruction::Unreachable => {}
                Instruction::Return { value, .. } => {
                    if let Some(value) = value {
                        self.value(value)?;
                        self.same(&value.ty, self.result)?;
                    } else {
                        self.same(self.result, "Unit")?;
                    }
                }
            }
        }
        Ok(())
    }
}
impl Program {
    pub fn validate(&self) -> Result<(), String> {
        let base = Check {
            program: self,
            values: BTreeMap::new(),
            loops: BTreeSet::new(),
            bindings: BTreeSet::new(),
            access_bindings: BTreeSet::new(),
            escape_parameters: BTreeSet::new(),
            result: "Unit",
            caller_location: false,
        };
        for ty in self.types.values() {
            match ty {
                Type::Record(fields) => {
                    for (_, t) in fields {
                        base.ty(t)?;
                    }
                }
                Type::Callable {
                    fields,
                    function,
                    params,
                    result,
                } => {
                    for (_, t) in fields {
                        base.ty(t)?;
                    }
                    let f = self
                        .functions
                        .get(*function)
                        .ok_or("invalid callable function ID")?;
                    if f.params.get(1..) != Some(params.as_slice()) || &f.result != result {
                        return Err("IR callable signature mismatch".into());
                    }
                }
                Type::Enum(variants) => {
                    for (_, fields) in variants {
                        for ty in fields {
                            base.ty(ty)?;
                        }
                    }
                }
                Type::Vector(t) | Type::Set(t) | Type::Iterator { element: t } => base.ty(t)?,
                Type::Dict { key, value } => {
                    base.ty(key)?;
                    base.ty(value)?;
                }
                Type::Primitive | Type::Opaque => {}
            }
        }
        for (id, f) in self.functions.iter().enumerate() {
            f.access.validate(&f.result, &self.types)?;
            f.effects
                .validate_signature(&f.params, &f.result, &self.types, f.body.is_some())?;
            if id != f.id {
                return Err("unstable IR function ID".into());
            }
            base.ty(&f.result)?;
            let mut check = Check {
                result: &f.result,
                ..Check {
                    program: self,
                    values: BTreeMap::new(),
                    loops: BTreeSet::new(),
                    bindings: BTreeSet::new(),
                    access_bindings: BTreeSet::new(),
                    escape_parameters: BTreeSet::new(),
                    result: "Unit",
                    caller_location: true,
                }
            };
            for (i, t) in f.params.iter().enumerate() {
                check.ty(t)?;
                check.values.insert(ValueId::Parameter(i), t.clone());
            }
            if let Some(body) = &f.body {
                check
                    .body(body)
                    .map_err(|e| format!("invalid HIR in {}: {e}", f.name))?;
            }
        }
        let mut check = base;
        check
            .body(&self.main)
            .map_err(|e| format!("invalid main HIR: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_overload_signatures_and_unresolved_values_are_rejected() {
        let result = Value {
            id: ValueId::Local(1),
            ty: "Int".into(),
        };
        let argument = Value {
            id: ValueId::Local(0),
            ty: "Bool".into(),
        };
        let mut program = Program {
            types: [
                ("Int".into(), Type::Primitive),
                ("Bool".into(), Type::Primitive),
                ("Unit".into(), Type::Primitive),
            ]
            .into(),
            functions: vec![Function {
                id: 0,
                name: "identity".into(),
                effects: crate::effects::Summary::unanalyzed(1),
                access: crate::readonly::Summary::default(),
                params: vec!["Int".into()],
                result: "Int".into(),
                body: None,
            }],
            main: Body {
                instructions: vec![
                    Instruction::Literal {
                        result: argument.clone(),
                        value: Literal::Bool(true),
                    },
                    Instruction::Call(Call {
                        function: 0,
                        inherit_location: false,
                        args: vec![argument],
                        span: crate::ast::Span {
                            file: "test".into(),
                            line: 1,
                            column: 1,
                        },
                        result,
                    }),
                ],
            },
        };
        assert!(program.validate().unwrap_err().contains("type mismatch"));
        if let Instruction::Call(call) = &mut program.main.instructions[1] {
            call.args[0] = Value {
                id: ValueId::Local(99),
                ty: "Int".into(),
            };
        }
        assert!(
            program
                .validate()
                .unwrap_err()
                .contains("undefined IR value")
        );
        program.functions[0].params = vec!["Bool".into()];
        if let Instruction::Call(call) = &mut program.main.instructions[1] {
            call.args[0] = Value {
                id: ValueId::Local(0),
                ty: "Bool".into(),
            };
            call.inherit_location = true;
        }
        assert!(program.validate().unwrap_err().contains("caller location"));
        assert!(
            crate::lir::lower_program(&program)
                .validate()
                .unwrap_err()
                .contains("caller location")
        );
    }
}
