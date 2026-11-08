//! Target-independent basic blocks. Every block has one explicit terminator.
use crate::hir::{self, Body, Instruction, Value, ValueId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockId(pub usize);
#[derive(Clone, Debug)]
pub(crate) enum Operation {
    DescribeBinding {
        value: Value,
        name: crate::ast::Name,
        captured: bool,
    },
    AnnotateEscape {
        parameter: usize,
        info: hir::EscapeInfo,
    },
    AnnotateAccess {
        value: Value,
        info: hir::AccessInfo,
    },
    AnnotateBinding {
        value: Value,
        info: hir::BindingInfo,
    },
    Object {
        result: Value,
        expression: hir::Rvalue,
    },
    Literal {
        result: Value,
        value: hir::Literal,
    },
    Call(hir::Call),
    Declare {
        result: Value,
        initial: Option<Value>,
    },
    Rebind {
        target: hir::Place,
        value: Value,
        span: Option<crate::ast::Span>,
    },
    StoreBool {
        target: Value,
        value: Value,
    },
    Release(Vec<ValueId>),
}
#[derive(Debug)]
pub(crate) enum Terminator {
    Jump(BlockId),
    Branch {
        condition: Value,
        yes: BlockId,
        no: BlockId,
    },
    Return {
        span: Option<crate::ast::Span>,
        value: Option<Value>,
        cleanup: Vec<ValueId>,
    },
    Unreachable,
}
#[derive(Debug)]
pub(crate) struct Block {
    pub instructions: Vec<Operation>,
    pub terminator: Terminator,
}
#[derive(Debug)]
pub(crate) struct Function {
    pub blocks: Vec<Block>,
    pub slots: Vec<Value>,
}
struct Loop {
    id: hir::LoopId,
    head: BlockId,
    end: BlockId,
    depth: usize,
}
struct Lower {
    blocks: Vec<Block>,
    current: BlockId,
    terminated: bool,
    scopes: Vec<Vec<ValueId>>,
    loops: Vec<Loop>,
    slots: BTreeMap<ValueId, Value>,
}
impl Lower {
    fn block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(Block {
            instructions: vec![],
            terminator: Terminator::Unreachable,
        });
        id
    }
    fn switch(&mut self, id: BlockId) {
        self.current = id;
        self.terminated = false;
    }
    fn terminate(&mut self, t: Terminator) {
        self.blocks[self.current.0].terminator = t;
        self.terminated = true;
    }
    fn jump(&mut self, id: BlockId) {
        if !self.terminated {
            self.terminate(Terminator::Jump(id));
        }
    }
    fn cleanup(&self, depth: usize) -> Vec<ValueId> {
        self.scopes[depth..]
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev().cloned())
            .collect()
    }
    fn emit(&mut self, i: Operation) {
        self.blocks[self.current.0].instructions.push(i);
    }
    fn scoped(&mut self, body: &Body) {
        self.scopes.push(vec![]);
        self.body(body);
        if !self.terminated {
            self.emit(Operation::Release(self.cleanup(self.scopes.len() - 1)));
        }
        self.scopes.pop();
    }
    fn body(&mut self, body: &Body) {
        for instruction in &body.instructions {
            if self.terminated {
                let dead = self.block();
                self.switch(dead);
            }
            match instruction {
                Instruction::Scope(body) => self.scoped(body),
                Instruction::If { condition, yes, no } => {
                    let a = self.block();
                    let b = self.block();
                    let end = self.block();
                    self.terminate(Terminator::Branch {
                        condition: condition.clone(),
                        yes: a,
                        no: b,
                    });
                    self.switch(a);
                    self.scoped(yes);
                    self.jump(end);
                    self.switch(b);
                    self.scoped(no);
                    self.jump(end);
                    self.switch(end);
                }
                Instruction::Loop { id, body } => {
                    let head = self.block();
                    let end = self.block();
                    self.jump(head);
                    self.switch(head);
                    self.loops.push(Loop {
                        id: *id,
                        head,
                        end,
                        depth: self.scopes.len(),
                    });
                    self.scoped(body);
                    self.jump(head);
                    self.loops.pop();
                    self.switch(end);
                }
                Instruction::Break(id) | Instruction::Continue(id) => {
                    let target = self
                        .loops
                        .iter()
                        .rev()
                        .find(|l| l.id == *id)
                        .expect("resolved loop target");
                    let dest = if matches!(instruction, Instruction::Break(_)) {
                        target.end
                    } else {
                        target.head
                    };
                    let cleanup = self.cleanup(target.depth);
                    self.emit(Operation::Release(cleanup));
                    self.terminate(Terminator::Jump(dest));
                }
                Instruction::Unreachable => self.terminate(Terminator::Unreachable),
                Instruction::Return { value, span } => self.terminate(Terminator::Return {
                    span: span.clone(),
                    value: value.clone(),
                    cleanup: self.cleanup(0),
                }),
                operation => {
                    match operation {
                        Instruction::Object { result, .. }
                        | Instruction::Literal { result, .. }
                        | Instruction::Declare { result, .. } => self.register(result),
                        Instruction::Call(call) => self.register(&call.result),
                        _ => {}
                    }
                    let operation = match operation {
                        Instruction::Object { result, expression } => Operation::Object {
                            result: result.clone(),
                            expression: expression.clone(),
                        },
                        Instruction::Literal { result, value } => Operation::Literal {
                            result: result.clone(),
                            value: value.clone(),
                        },
                        Instruction::Call(call) => Operation::Call(call.clone()),
                        Instruction::Declare { result, initial } => Operation::Declare {
                            result: result.clone(),
                            initial: initial.clone(),
                        },
                        Instruction::Rebind {
                            target,
                            value,
                            span,
                        } => Operation::Rebind {
                            target: target.clone(),
                            value: value.clone(),
                            span: span.clone(),
                        },
                        Instruction::DescribeBinding {
                            value,
                            name,
                            captured,
                        } => Operation::DescribeBinding {
                            value: value.clone(),
                            name: name.clone(),
                            captured: *captured,
                        },
                        Instruction::AnnotateEscape { parameter, info } => {
                            Operation::AnnotateEscape {
                                parameter: *parameter,
                                info: info.clone(),
                            }
                        }
                        Instruction::AnnotateAccess { value, info } => Operation::AnnotateAccess {
                            value: value.clone(),
                            info: info.clone(),
                        },
                        Instruction::AnnotateBinding { value, info } => {
                            Operation::AnnotateBinding {
                                value: value.clone(),
                                info: info.clone(),
                            }
                        }
                        Instruction::StoreBool { target, value } => Operation::StoreBool {
                            target: target.clone(),
                            value: value.clone(),
                        },
                        Instruction::Release(values) => Operation::Release(values.clone()),
                        _ => unreachable!("structured HIR handled above"),
                    };
                    self.emit(operation);
                }
            }
        }
    }
    fn register(&mut self, value: &Value) {
        assert!(
            self.slots.insert(value.id.clone(), value.clone()).is_none(),
            "unique binding identity"
        );
        self.scopes.last_mut().unwrap().push(value.id.clone());
    }
}
pub(crate) fn lower(body: &Body) -> Function {
    let mut lower = Lower {
        blocks: vec![],
        current: BlockId(0),
        terminated: false,
        scopes: vec![vec![]],
        loops: vec![],
        slots: BTreeMap::new(),
    };
    lower.block();
    lower.body(body);
    let mut function = Function {
        blocks: lower.blocks,
        slots: lower.slots.into_values().collect(),
    };
    crate::lir_cleanup::simplify(&mut function);
    function
}

#[derive(Debug)]
pub(crate) struct ProgramFunction {
    pub id: hir::FunctionId,
    pub name: String,
    pub params: Vec<String>,
    pub result: String,
    pub body: Option<Function>,
    pub effects: crate::effects::Summary,
    pub access: crate::readonly::Summary,
}
#[derive(Debug)]
pub(crate) struct Program {
    pub types: BTreeMap<String, hir::Type>,
    pub functions: Vec<ProgramFunction>,
    pub main: Function,
}
pub(crate) fn lower_program(program: &hir::Program) -> Program {
    Program {
        types: program.types.clone(),
        functions: program
            .functions
            .iter()
            .map(|f| ProgramFunction {
                id: f.id,
                name: f.name.clone(),
                params: f.params.clone(),
                result: f.result.clone(),
                body: f.body.as_ref().map(lower),
                effects: f.effects.clone(),
                access: f.access.clone(),
            })
            .collect(),
        main: lower(&program.main),
    }
}

impl Program {
    pub fn validate(&self) -> Result<(), String> {
        for (id, f) in self.functions.iter().enumerate() {
            if f.id != id
                || !self.types.contains_key(&f.result)
                || f.params.iter().any(|t| !self.types.contains_key(t))
            {
                return Err(format!("invalid LIR signature for {}", f.name));
            }
            f.effects
                .validate_signature(&f.params, &f.result, &self.types, f.body.is_some())?;
            f.access.validate(&f.result, &self.types)?;
            if let Some(body) = &f.body {
                self.validate_body(body, &f.params, &f.result, true)?;
                crate::bindings::validate(body).map_err(|error| {
                    format!("{error}\nwhile checking binding contracts in '{}'", f.name)
                })?;
            }
        }
        self.validate_body(&self.main, &[], "Unit", false)?;
        crate::bindings::validate(&self.main)
    }
    fn validate_body(
        &self,
        body: &Function,
        params: &[String],
        result: &str,
        caller_location: bool,
    ) -> Result<(), String> {
        if body.blocks.is_empty() {
            return Err("LIR requires an entry block".into());
        }
        let mut values = params
            .iter()
            .enumerate()
            .map(|(i, t)| (ValueId::Parameter(i), t.clone()))
            .collect::<BTreeMap<_, _>>();
        for value in &body.slots {
            if !matches!(value.id, ValueId::Local(_))
                || !self.types.contains_key(&value.ty)
                || values.insert(value.id.clone(), value.ty.clone()).is_some()
            {
                return Err("invalid LIR slot".into());
            }
        }
        let value = |v: &Value| -> Result<(), String> {
            if values.get(&v.id) == Some(&v.ty) {
                Ok(())
            } else {
                Err(format!("invalid LIR value {:?}", v.id))
            }
        };
        let release = |ids: &[ValueId]| -> Result<(), String> {
            if ids.iter().all(|id| values.contains_key(id)) {
                Ok(())
            } else {
                Err("invalid LIR release".into())
            }
        };
        let target = |id: BlockId| -> Result<(), String> {
            if id.0 < body.blocks.len() {
                Ok(())
            } else {
                Err("invalid LIR jump target".into())
            }
        };
        let mut access_bindings = std::collections::BTreeSet::new();
        let mut escape_parameters = std::collections::BTreeSet::new();
        for block in &body.blocks {
            for op in &block.instructions {
                match op {
                    Operation::Object { result, expression } => {
                        value(result)?;
                        match expression {
                            hir::Rvalue::Share(v)
                            | hir::Rvalue::BoolCopy(v)
                            | hir::Rvalue::VariantTest { owner: v, .. }
                            | hir::Rvalue::VariantField { owner: v, .. } => value(v)?,
                            hir::Rvalue::Field { owner, .. } => value(owner)?,
                            hir::Rvalue::Aggregate(fields)
                            | hir::Rvalue::Variant { fields, .. } => {
                                for f in fields {
                                    value(f)?;
                                }
                            }
                        }
                    }
                    Operation::Literal { result, .. } => value(result)?,
                    Operation::Call(call) => {
                        if call.inherit_location && !caller_location {
                            return Err("caller location is not available in main LIR".into());
                        }
                        value(&call.result)?;
                        let f = self
                            .functions
                            .get(call.function)
                            .ok_or("invalid LIR callee")?;
                        if call.result.ty != f.result || call.args.len() != f.params.len() {
                            return Err("invalid LIR call signature".into());
                        }
                        for (v, t) in call.args.iter().zip(&f.params) {
                            value(v)?;
                            if &v.ty != t {
                                return Err("invalid LIR argument type".into());
                            }
                        }
                    }
                    Operation::Declare { result, initial } => {
                        value(result)?;
                        if let Some(v) = initial {
                            value(v)?;
                        }
                    }
                    Operation::Rebind {
                        target, value: v, ..
                    } => {
                        value(v)?;
                        match target {
                            hir::Place::Local(id) => {
                                if values.get(id) != Some(&v.ty) {
                                    return Err("invalid LIR rebind".into());
                                }
                            }
                            hir::Place::Field { owner, .. } => value(owner)?,
                        }
                    }
                    Operation::StoreBool { target, value: v } => {
                        value(target)?;
                        value(v)?;
                        if target.ty != "Bool" || v.ty != "Bool" {
                            return Err("invalid LIR boolean store".into());
                        }
                    }
                    Operation::Release(ids) => release(ids)?,
                    Operation::DescribeBinding { value: v, .. } => {
                        value(v)?;
                        if !matches!(v.id, hir::ValueId::Local(_)) {
                            return Err("source binding requires a local identity".into());
                        }
                    }
                    Operation::AnnotateBinding { value: v, .. } => value(v)?,
                    Operation::AnnotateEscape { parameter, .. } => {
                        if *parameter >= params.len() || !escape_parameters.insert(*parameter) {
                            return Err("invalid or duplicate LIR escape annotation".into());
                        }
                    }
                    Operation::AnnotateAccess { value: v, .. } => {
                        value(v)?;
                        if !matches!(v.id, ValueId::Local(_))
                            || !access_bindings.insert(v.id.clone())
                        {
                            return Err("invalid or duplicate LIR access annotation".into());
                        }
                    }
                }
            }
            match &block.terminator {
                Terminator::Jump(id) => target(*id)?,
                Terminator::Branch { condition, yes, no } => {
                    value(condition)?;
                    if condition.ty != "Bool" {
                        return Err("invalid LIR branch condition".into());
                    }
                    target(*yes)?;
                    target(*no)?;
                }
                Terminator::Return {
                    value: v, cleanup, ..
                } => {
                    if let Some(v) = v {
                        value(v)?;
                        if v.ty != result {
                            return Err("invalid LIR return type".into());
                        }
                    } else if result != "Unit" {
                        return Err("invalid empty LIR return".into());
                    }
                    release(cleanup)?;
                }
                Terminator::Unreachable => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn int(id: usize) -> Value {
        Value {
            id: ValueId::Local(id),
            ty: "Int".into(),
        }
    }
    #[test]
    fn an_outer_loop_exit_cleans_inner_scopes_but_preserves_the_returned_owner() {
        let body = Body {
            instructions: vec![
                Instruction::Literal {
                    result: int(1),
                    value: hir::Literal::Int(42),
                },
                Instruction::Loop {
                    id: 10,
                    body: Body {
                        instructions: vec![
                            Instruction::Literal {
                                result: int(2),
                                value: hir::Literal::Int(2),
                            },
                            Instruction::Loop {
                                id: 20,
                                body: Body {
                                    instructions: vec![
                                        Instruction::Literal {
                                            result: int(3),
                                            value: hir::Literal::Int(3),
                                        },
                                        Instruction::Break(10),
                                    ],
                                },
                            },
                        ],
                    },
                },
                Instruction::Return {
                    value: Some(int(1)),
                    span: None,
                },
            ],
        };
        let function = lower(&body);
        let end = BlockId(
            function
                .blocks
                .iter()
                .position(|b| matches!(b.terminator, Terminator::Return { .. }))
                .unwrap(),
        );
        let exit = function
            .blocks
            .iter()
            .find(|b| matches!(b.terminator,Terminator::Jump(id) if id==end))
            .unwrap();
        let Operation::Release(ids) = exit.instructions.last().unwrap() else {
            panic!("exit must release its scopes")
        };
        assert_eq!(ids, &vec![ValueId::Local(3), ValueId::Local(2)]);
        let Terminator::Return { cleanup, .. } = &function.blocks[end.0].terminator else {
            unreachable!()
        };
        assert_eq!(cleanup, &vec![ValueId::Local(1)]);
    }
    #[test]
    fn corrupt_jump_targets_are_rejected() {
        let mut program = Program {
            types: BTreeMap::new(),
            functions: vec![],
            main: Function {
                slots: vec![],
                blocks: vec![Block {
                    instructions: vec![],
                    terminator: Terminator::Jump(BlockId(9)),
                }],
            },
        };
        assert!(program.validate().unwrap_err().contains("jump target"));
        program.main.blocks[0].terminator = Terminator::Return {
            span: None,
            value: None,
            cleanup: vec![],
        };
        assert!(program.validate().is_ok());
    }
    #[test]
    fn escape_metadata_requires_unique_parameter_identities_and_preserves_return_locations() {
        let span = crate::ast::Span {
            file: "test".into(),
            line: 4,
            column: 2,
        };
        let info = hir::EscapeInfo {
            name: "x".into(),
            declaration: span.clone(),
            annotation: crate::ast::EscapeAnnotation {
                mode: crate::ast::EscapeMode::Noescape,
                span: span.clone(),
            },
        };
        let body = Body {
            instructions: vec![
                Instruction::AnnotateEscape {
                    parameter: 0,
                    info: info.clone(),
                },
                Instruction::Return {
                    value: None,
                    span: Some(span.clone()),
                },
            ],
        };
        let function = lower(&body);
        assert!(function.slots.is_empty());
        let Terminator::Return {
            span: Some(location),
            ..
        } = &function.blocks[0].terminator
        else {
            panic!("return location was lost")
        };
        assert_eq!(location, &span);
        let types = BTreeMap::from([
            ("Int".into(), hir::Type::Primitive),
            ("Unit".into(), hir::Type::Primitive),
        ]);
        let mut program = Program {
            types,
            functions: vec![],
            main: function,
        };
        assert!(
            program
                .validate_body(&program.main, &["Int".into()], "Unit", true)
                .is_ok()
        );
        assert!(
            program
                .validate()
                .unwrap_err()
                .contains("escape annotation")
        );
        program.main.blocks[0]
            .instructions
            .push(Operation::AnnotateEscape { parameter: 0, info });
        assert!(
            program
                .validate_body(&program.main, &["Int".into()], "Unit", true)
                .unwrap_err()
                .contains("duplicate")
        );
    }
}
