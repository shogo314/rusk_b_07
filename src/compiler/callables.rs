use super::*;

#[derive(Clone)]
pub(super) struct Callable {
    pub parameters: Vec<String>,
    pub result: String,
    pub function: usize,
}

#[derive(Clone)]
pub(super) struct CaptureContext {
    available: BTreeMap<String, Variable>,
    captures: Vec<(Name, Variable)>,
    pub(super) locals: BTreeMap<String, Variable>,
    prologue: Body,
    ty: String,
    span: Span,
}

impl Compiler {
    pub(super) fn local_variable(&self, name: &str) -> Option<Variable> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).cloned()
    }

    pub(super) fn lookup_variable(
        &mut self,
        name: &Name,
        callable_only: bool,
    ) -> Result<Option<Variable>, String> {
        if let Some(variable) = self.local_variable(&name.text) {
            return Ok(Some(variable));
        }
        let Some(context) = self.capture_contexts.last() else {
            return Ok(None);
        };
        if let Some(local) = context.locals.get(&name.text).cloned() {
            if callable_only && !self.callables.contains_key(&local.ty) {
                return Ok(None);
            }
            self.scopes[0].insert(name.text.clone(), local.clone());
            return Ok(Some(local));
        }
        let Some(variable) = context.available.get(&name.text).cloned() else {
            return Ok(None);
        };
        if callable_only && !self.callables.contains_key(&variable.ty) {
            return Ok(None);
        }
        if !variable.initialized {
            return Err(context.span.error(format!(
                "captured variable '{}' used before initialization",
                name.text
            )));
        }
        let env = context.ty.clone();
        let index = context.captures.len();
        let value = Value {
            id: self.temp(),
            ty: variable.ty.clone(),
        };
        let context = self.capture_contexts.last_mut().unwrap();
        context.captures.push((name.clone(), variable.clone()));
        context.prologue.push(Instruction::Object {
            result: value.clone(),
            expression: Rvalue::Field {
                owner: Value {
                    id: ValueId::Parameter(0),
                    ty: env,
                },
                index,
            },
        });
        context.prologue.push(Instruction::DescribeBinding {
            value: value.clone(),
            name: name.clone(),
            captured: true,
        });
        if let Some(info) = &variable.binding {
            context.prologue.push(Instruction::AnnotateBinding {
                value: value.clone(),
                info: info.clone(),
            });
        }
        if let Some(info) = &variable.access {
            context.prologue.push(Instruction::AnnotateAccess {
                value: value.clone(),
                info: info.clone(),
            });
        }
        let local = Variable {
            id: value.id,
            ty: value.ty,
            initialized: true,
            binding: variable.binding.clone(),
            access: variable.access.clone(),
        };
        self.capture_contexts
            .last_mut()
            .unwrap()
            .locals
            .insert(name.text.clone(), local.clone());
        self.scopes[0].insert(name.text.clone(), local.clone());
        Ok(Some(local))
    }

    pub(super) fn invoke(
        &mut self,
        callee: Value,
        args: Vec<Value>,
        span: &Span,
    ) -> Result<Value, String> {
        if self.diverged {
            return Ok(Value::never());
        }

        let callable = self
            .callables
            .get(&callee.ty)
            .cloned()
            .ok_or_else(|| span.error(format!("type '{}' is not callable", callee.ty)))?;
        if args.len() != callable.parameters.len() {
            return Err(span.error("callable argument count mismatch"));
        }
        for (value, ty) in args.iter().zip(&callable.parameters) {
            self.same(ty, &value.ty, span)?;
        }
        let name = self.functions[callable.function].name.text.clone();
        let mut values = vec![callee];
        values.extend(args);
        self.call(&name, values, None, span)
    }

    pub(super) fn lambda(
        &mut self,
        parameters: &[(DeclaredName, Name)],
        body: &Expr,
        span: &Span,
    ) -> Result<Value, String> {
        let mut bound = BTreeSet::new();
        for (name, ty) in parameters {
            self.check_type(ty)?;
            if !bound.insert(name.text.clone()) {
                return Err(name.span.error("duplicate lambda parameter"));
            }
        }
        let mut available = self
            .capture_contexts
            .last()
            .map(|c| c.available.clone())
            .unwrap_or_default();
        for scope in &self.scopes {
            available.extend(scope.clone());
        }
        let key = format!(
            "{}:{}:{}:{:?}:{:?}:{:?}",
            span.file,
            span.line,
            span.column,
            self.current_function,
            parameters.iter().map(|(_, t)| &t.text).collect::<Vec<_>>(),
            available
                .iter()
                .map(|(n, v)| (n, &v.ty, v.initialized))
                .collect::<Vec<_>>()
        );
        let ty = if let Some(ty) = self.lambda_types.get(&key) {
            ty.clone()
        } else {
            if self.callables.len() >= 256 {
                return Err(span.error("callable specialization limit exceeded"));
            }
            let ty = format!("__RuskCallable{}", self.lambda_counter);
            self.lambda_counter += 1;
            let native = format!("RuskRecord{}", self.records.len());
            self.types.insert(ty.clone(), native);
            self.records.insert(
                ty.clone(),
                records::Record {
                    name: Name {
                        text: ty.clone(),
                        span: span.clone(),
                    },
                    fields: vec![],
                },
            );
            self.capture_contexts.push(CaptureContext {
                available,
                captures: vec![],
                locals: BTreeMap::new(),
                prologue: Body::default(),
                ty: ty.clone(),
                span: span.clone(),
            });
            let saved_body = std::mem::take(&mut self.body);
            let saved_scopes = std::mem::take(&mut self.scopes);
            let saved_temps = std::mem::take(&mut self.pending_temps);
            let saved_loops = std::mem::take(&mut self.loops);
            let saved_return = self.return_type.take();
            let saved_exit = self.diverged;
            let saved_infer = self.infer_return;
            let saved_inferred = self.inferred_return.take();
            self.diverged = false;
            self.infer_return = true;
            self.scopes = vec![BTreeMap::new()];
            self.return_type = Some("Unit".into());
            let checked = (|| {
                for (index, (name, ty)) in parameters.iter().enumerate() {
                    self.declare_annotated(
                        name,
                        ty.text.clone(),
                        Some(Value {
                            ty: ty.text.clone(),
                            id: ValueId::Parameter(index + 1),
                        }),
                    )?;
                }
                self.expr(body, None)
            })();
            let mut context = self.capture_contexts.pop().unwrap();
            // Keep the established capture/copy order independent of first use.
            let original = context
                .captures
                .iter()
                .map(|(n, _)| n.text.clone())
                .collect::<Vec<_>>();
            context
                .captures
                .sort_by(|(a, _), (b, _)| a.text.cmp(&b.text));
            let positions = context
                .captures
                .iter()
                .enumerate()
                .map(|(i, (n, _))| (n.text.clone(), i))
                .collect::<BTreeMap<_, _>>();
            for instruction in &mut context.prologue.instructions {
                if let Instruction::Object {
                    expression: Rvalue::Field { index, .. },
                    ..
                } = instruction
                {
                    *index = positions[&original[*index]];
                }
            }
            let mut checked_body = context.prologue;
            checked_body
                .instructions
                .extend(std::mem::take(&mut self.body).instructions);
            let exits = self.diverged;
            let inferred = self.inferred_return.take();
            self.body = saved_body;
            self.scopes = saved_scopes;
            self.pending_temps = saved_temps;
            self.loops = saved_loops;
            self.return_type = saved_return;
            self.diverged = saved_exit;
            self.infer_return = saved_infer;
            self.inferred_return = saved_inferred;
            let value = checked?;
            self.records.get_mut(&ty).unwrap().fields = context
                .captures
                .iter()
                .map(|(name, var)| {
                    (
                        name.clone(),
                        Name {
                            text: var.ty.clone(),
                            span: name.span.clone(),
                        },
                    )
                })
                .collect();
            self.prepare_record(&ty)?;
            let result = if value.is_never() {
                inferred.clone().unwrap_or("Unit".into())
            } else {
                if let Some(inferred) = &inferred {
                    self.same(inferred, &value.ty, span)?;
                }
                value.ty.clone()
            };
            let index = self.functions.len();
            let f = Function {
                name: Name {
                    text: format!("$invoke{index}"),
                    span: span.clone(),
                },
                params: std::iter::once(ty.clone())
                    .chain(parameters.iter().map(|(_, t)| t.text.clone()))
                    .collect(),
                result: result.clone(),
                cpp: format!("rusk_callable_{index}"),
                code: String::new(),
                body: Some((vec![], Box::new(Stmt::Block(vec![], None)))),
                checked: Some({
                    if !exits {
                        checked_body.push(Instruction::Return {
                            value: if result == "Unit" {
                                None
                            } else {
                                Some(value.clone())
                            },
                            span: Some(body.span.clone()),
                        });
                    }
                    checked_body
                }),
            };
            self.functions.push(f);
            self.callables.insert(
                ty.clone(),
                Callable {
                    parameters: parameters.iter().map(|(_, t)| t.text.clone()).collect(),
                    result,
                    function: index,
                },
            );
            self.lambda_types.insert(key, ty.clone());
            ty
        };
        let fields = self.records[&ty].fields.clone();
        let mut values = vec![];
        for (name, _) in fields {
            let variable = self
                .lookup_variable(&name, false)?
                .ok_or_else(|| name.span.error("missing captured variable"))?;
            if !variable.initialized {
                return Err(name.span.error(format!(
                    "captured variable '{}' used before initialization",
                    name.text
                )));
            }
            values.push(Value {
                id: variable.id,
                ty: variable.ty,
            });
        }
        Ok(self.emit(&ty, Rvalue::Aggregate(values)))
    }

    pub(super) fn function_value(&mut self, name: &Name) -> Result<Value, String> {
        let candidates = self
            .functions
            .iter()
            .enumerate()
            .filter(|(i, f)| f.name.text == name.text && !self.generic_calls.contains_key(i))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(name.span.error(if candidates.is_empty() {
                format!("unknown variable or concrete function '{}'", name.text)
            } else {
                format!(
                    "passing overloaded function '{}' is not supported",
                    name.text
                )
            }));
        }
        let index = candidates[0];
        if let Some(ty) = self.function_types.get(&index).cloned() {
            return Ok(self.emit(&ty, Rvalue::Aggregate(vec![])));
        }
        let f = self.functions[index].clone();
        let parameters = f
            .params
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                (
                    DeclaredName::plain(Name {
                        text: format!("$argument{i}"),
                        span: name.span.clone(),
                    }),
                    Name {
                        text: ty.clone(),
                        span: name.span.clone(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let expr = Expr {
            span: name.span.clone(),
            kind: ExprKind::Call(
                name.clone(),
                parameters
                    .iter()
                    .map(|(n, _)| Expr {
                        kind: ExprKind::Var(n.name.clone()),
                        span: n.span.clone(),
                    })
                    .collect(),
            ),
        };
        let value = self.lambda(&parameters, &expr, &name.span)?;
        self.function_types.insert(index, value.ty.clone());
        Ok(value)
    }
}

impl Compiler {
    pub(super) fn invoke_arguments(
        &mut self,
        callee: Value,
        args: &[Expr],
        span: &Span,
    ) -> Result<Value, String> {
        if self.diverged {
            return Ok(Value::never());
        }

        let info = self
            .callables
            .get(&callee.ty)
            .cloned()
            .ok_or_else(|| span.error(format!("type '{}' is not callable", callee.ty)))?;
        let mut values = vec![];
        for (index, arg) in args.iter().enumerate() {
            values.push(self.expr(arg, info.parameters.get(index).map(String::as_str))?);
        }
        self.invoke(callee, values, span)
    }
    pub(super) fn invoke_expression(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        span: &Span,
    ) -> Result<Value, String> {
        if let ExprKind::Field(owner, name) = &callee.kind {
            let owner = self.expr(owner, None)?;
            if !self.callables.contains_key(&owner.ty)
                && let Ok((index, ty)) = self.field(&owner.ty, name)
                && self.callables.contains_key(&ty)
            {
                let field = self.emit(&ty, Rvalue::Field { owner, index });
                return self.invoke_arguments(field, args, span);
            }
            if self
                .lookup_variable(name, true)?
                .is_some_and(|v| self.callables.contains_key(&v.ty))
            {
                let function = self.expr(
                    &Expr {
                        kind: ExprKind::Var(name.clone()),
                        span: name.span.clone(),
                    },
                    None,
                )?;
                let info =
                    self.callables.get(&function.ty).cloned().ok_or_else(|| {
                        span.error("method name refers to a non-callable variable")
                    })?;
                let mut values = vec![owner];
                for (index, arg) in args.iter().enumerate() {
                    values
                        .push(self.expr(arg, info.parameters.get(index + 1).map(String::as_str))?);
                }
                return self.invoke(function, values, span);
            }
            let mut values = vec![owner];
            for arg in args {
                let hints = self.argument_hints(&name.text, args.len() + 1, &values);
                let builtin_hint =
                    self.container_argument_hint(&name.text, args.len() + 1, &values, None);
                let hint = if hints.len() == 1 {
                    hints.first().map(String::as_str)
                } else {
                    builtin_hint.as_deref()
                };
                values.push(self.expr(arg, hint)?);
            }
            return self.call(&name.text, values, None, span);
        }
        let callee = self.expr(callee, None)?;
        self.invoke_arguments(callee, args, span)
    }
}
