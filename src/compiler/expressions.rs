use super::*;

impl Compiler {
    pub(super) fn conditional<F, G>(
        &mut self,
        condition: Value,
        expected: Option<&str>,
        yes: F,
        no: G,
        span: &Span,
    ) -> Result<Value, String>
    where
        F: FnOnce(&mut Self, Option<&str>) -> Result<Value, String>,
        G: FnOnce(&mut Self, Option<&str>) -> Result<Value, String>,
    {
        if self.diverged {
            return Ok(Value::never());
        }
        self.same("Bool", &condition.ty, span)?;
        let slot = self.temp();
        let outer_body = std::mem::take(&mut self.body);
        let outer_temps = std::mem::take(&mut self.pending_temps);
        let before = self.scopes.clone();
        let a = yes(self, expected)?;
        let yes_exit = self.diverged;
        if !yes_exit {
            if let Some(expected) = expected {
                self.same(expected, &a.ty, span)?;
            }
            self.body.push(Instruction::Rebind {
                span: None,
                target: Place::Local(slot.clone()),
                value: a.clone(),
            });
            self.release_temps();
        }
        let yes_body = std::mem::take(&mut self.body);
        let yes_scopes = self.scopes.clone();
        self.scopes = before;
        self.diverged = false;
        let hint = if yes_exit {
            expected
        } else {
            Some(a.ty.as_str())
        };
        let b = no(self, hint)?;
        let no_exit = self.diverged;
        if !no_exit {
            if let Some(hint) = hint {
                self.same(hint, &b.ty, span)?;
            }
            self.body.push(Instruction::Rebind {
                span: None,
                target: Place::Local(slot.clone()),
                value: b.clone(),
            });
            self.release_temps();
        }
        let no_body = std::mem::take(&mut self.body);
        if yes_exit {
        } else if no_exit {
            self.scopes = yes_scopes;
        } else {
            self.intersect_initialization(&yes_scopes);
        }
        self.body = outer_body;
        self.pending_temps = outer_temps;
        self.diverged = yes_exit && no_exit;
        let result = if self.diverged {
            Value::never()
        } else {
            Value {
                id: slot,
                ty: if yes_exit { b.ty } else { a.ty },
            }
        };
        if !self.diverged {
            self.body.push(Instruction::Declare {
                result: result.clone(),
                initial: None,
            });
            self.pending_temps.push(result.id.clone());
        }
        self.body.push(Instruction::If {
            condition,
            yes: yes_body,
            no: no_body,
        });
        Ok(result)
    }

    pub(super) fn if_expression(
        &mut self,
        condition: &Condition,
        yes: &Expr,
        no: &Expr,
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Value, String> {
        let (condition, bindings) = self.pattern_condition(condition)?;
        self.conditional(
            condition,
            expected,
            |c, hint| c.scoped_pattern_values(&bindings, |c| c.expr(yes, hint)),
            |c, ty| c.expr(no, ty),
            span,
        )
    }

    pub(super) fn coalesce(
        &mut self,
        left: &Expr,
        right: &Expr,
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Value, String> {
        let hint = expected.map(|ty| format!("Option<{ty}>"));
        let value = self.expr(left, hint.as_deref())?;
        if self.diverged {
            return Ok(Value::never());
        }
        let element = self
            .option_element(&value.ty)
            .ok_or_else(|| span.error("left operand of ?? must be Option<T>"))?;
        let condition = self.emit(
            "Bool",
            Rvalue::VariantTest {
                owner: value.clone(),
                index: 0,
            },
        );
        self.conditional(
            condition,
            Some(&element),
            |c, _| {
                Ok(c.emit(
                    &element,
                    Rvalue::VariantField {
                        owner: value,
                        variant: 0,
                        index: 0,
                    },
                ))
            },
            |c, ty| c.expr(right, ty),
            span,
        )
    }

    pub(super) fn comparison_chain(
        &mut self,
        first: &Expr,
        rest: &[(String, Expr, Span)],
        span: &Span,
    ) -> Result<Value, String> {
        let first = self.expr(first, None)?;
        let result = self.literal("Bool", "true", span)?;
        self.chain_step(first, rest, &result)?;
        Ok(result)
    }
    fn chain_step(
        &mut self,
        left: Value,
        rest: &[(String, Expr, Span)],
        result: &Value,
    ) -> Result<(), String> {
        let (op, right, span) = &rest[0];
        let right = self.expr(right, None)?;
        if self.diverged {
            return Ok(());
        }
        let value = self.call(binary_name(op), vec![left, right.clone()], None, span)?;
        self.same("Bool", &value.ty, span)?;
        self.body.push(Instruction::StoreBool {
            target: result.clone(),
            value,
        });
        if rest.len() > 1 {
            let before = self.scopes.clone();
            let outer = std::mem::take(&mut self.pending_temps);
            let branch = self.capture(|c| {
                c.chain_step(right, &rest[1..], result)?;
                c.release_temps();
                Ok(())
            })?;
            self.body.push(Instruction::If {
                condition: result.clone(),
                yes: branch,
                no: Body::default(),
            });
            self.diverged = false;
            self.pending_temps = outer;
            self.scopes = before;
        }
        Ok(())
    }
}
