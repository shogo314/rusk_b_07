use super::*;
impl Compiler {
    pub(super) fn check_unreachable(&mut self, statement: &Stmt) -> Result<(), String> {
        let saved_body = std::mem::take(&mut self.body);
        let temps = std::mem::take(&mut self.pending_temps);
        let loops = self.loops.clone();
        let inferred = self.inferred_return.clone();
        self.diverged = false;
        let result = self.statement_inner(statement);
        self.body = saved_body;
        self.pending_temps = temps;
        self.loops = loops;
        self.inferred_return = inferred;
        self.diverged = true;
        result
    }
    pub(super) fn declare_dead_binding(
        &mut self,
        binding: &Binding,
        ty: &str,
    ) -> Result<(), String> {
        match binding {
            Binding::Name(name) => {
                if name.text != "_" {
                    if self.scopes.last().unwrap().contains_key(&name.text) {
                        return Err(name.span.error("duplicate variable"));
                    }
                    self.scopes.last_mut().unwrap().insert(
                        name.text.clone(),
                        Variable {
                            id: ValueId::Probe,
                            ty: ty.into(),
                            initialized: true,
                            access: name
                                .access
                                .as_ref()
                                .map(|annotation| crate::hir::AccessInfo {
                                    name: name.text.clone(),
                                    declaration: name.span.clone(),
                                    annotation: annotation.clone(),
                                }),
                            binding: name.binding.as_ref().map(|annotation| {
                                crate::hir::BindingInfo {
                                    name: name.text.clone(),
                                    declaration: name.span.clone(),
                                    annotation: annotation.clone(),
                                }
                            }),
                        },
                    );
                }
            }
            Binding::Tuple(items, _) => {
                let fields = self.records.get(ty).map(|r| r.fields.clone());
                for (i, item) in items.iter().enumerate() {
                    self.declare_dead_binding(
                        item,
                        fields
                            .as_ref()
                            .and_then(|f| f.get(i))
                            .map_or(crate::hir::DIVERGED, |(_, t)| t.text.as_str()),
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn loop_target(&self, label: Option<&Name>, span: &Span) -> Result<usize, String> {
        if let Some(label) = label {
            self.loops
                .iter()
                .rposition(|l| l.label.as_ref().is_some_and(|n| n.text == label.text))
                .ok_or_else(|| {
                    label
                        .span
                        .error(format!("unknown loop label '{}'", label.text))
                })
        } else {
            self.loops
                .len()
                .checked_sub(1)
                .ok_or_else(|| span.error("break or continue outside a loop"))
        }
    }
    pub(super) fn block_expression(
        &mut self,
        statements: &[Stmt],
        tail: Option<&Expr>,
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Value, String> {
        let outer = std::mem::take(&mut self.pending_temps);
        self.scopes.push(BTreeMap::new());
        let mut result = Value::never();
        let slot = self.temp();
        let body = self.capture(|c| {
            for statement in statements {
                c.statement(statement)?;
            }
            if c.diverged
                && let Some(tail) = tail
            {
                c.statement(&Stmt::Expr(tail.clone()))?;
            }
            if !c.diverged {
                result = if let Some(tail) = tail {
                    c.expr(tail, expected)?
                } else {
                    c.literal("Unit", "", span)?
                };
                if !c.diverged {
                    if let Some(expected) = expected {
                        c.same(expected, &result.ty, span)?;
                    }
                    c.body.push(Instruction::Rebind {
                        span: None,
                        target: Place::Local(slot.clone()),
                        value: result.clone(),
                    });
                    c.release_temps();
                }
            }
            Ok(())
        })?;
        self.scopes.pop();
        self.pending_temps = outer;
        if self.diverged {
            self.body.push(Instruction::Scope(body));
            return Ok(Value::never());
        }
        let result = Value {
            id: slot,
            ty: result.ty,
        };
        self.body.push(Instruction::Declare {
            result: result.clone(),
            initial: None,
        });
        self.body.push(Instruction::Scope(body));
        self.pending_temps.push(result.id.clone());
        Ok(result)
    }
}
