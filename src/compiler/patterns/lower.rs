use super::*;

#[derive(Clone)]
struct Probe {
    matched: Value,
    bindings: Vec<(Name, Value)>,
}

impl Compiler {
    fn pattern_probe(&mut self, pattern: &TypedPattern, value: Value) -> Result<Probe, String> {
        let mut bindings = vec![];
        for (_, (name, ty)) in pattern.bindings()? {
            let result = Value {
                id: self.temp(),
                ty,
            };
            self.body.push(Instruction::Declare {
                result: result.clone(),
                initial: None,
            });
            self.pending_temps.push(result.id.clone());
            bindings.push((name, result));
        }
        let matched = self.literal("Bool", "false", &pattern.span)?;
        let yes = self.literal("Bool", "true", &pattern.span)?;
        let probe = Probe { matched, bindings };
        self.pattern_tasks(
            &[(pattern.clone(), value)],
            BTreeMap::new(),
            &probe,
            &yes,
            &mut 100_000,
        )?;
        Ok(probe)
    }

    fn pattern_branch(
        &mut self,
        condition: Value,
        action: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let outer = std::mem::take(&mut self.pending_temps);
        let body = self.capture(|c| {
            action(c)?;
            c.release_temps();
            Ok(())
        })?;
        self.pending_temps = outer;
        self.body.push(Instruction::If {
            condition,
            yes: body,
            no: Body::default(),
        });
        Ok(())
    }
    fn pattern_tasks(
        &mut self,
        tasks: &[(TypedPattern, Value)],
        mut bindings: BTreeMap<String, Value>,
        probe: &Probe,
        yes: &Value,
        budget: &mut usize,
    ) -> Result<(), String> {
        let Some(((pattern, value), tail)) = tasks.split_first() else {
            for (name, slot) in &probe.bindings {
                self.body.push(Instruction::Rebind {
                    span: None,
                    target: Place::Local(slot.id.clone()),
                    value: bindings[&name.text].clone(),
                });
            }
            self.body.push(Instruction::StoreBool {
                target: probe.matched.clone(),
                value: yes.clone(),
            });
            return Ok(());
        };
        if *budget == 0 {
            return Err(pattern.span.error("pattern lowering limit exceeded"));
        }
        *budget -= 1;
        match &pattern.kind {
            Kind::Wild(name) => {
                if let Some(n) = name {
                    bindings.insert(n.text.clone(), value.clone());
                }
                self.pattern_tasks(tail, bindings, probe, yes, budget)
            }
            Kind::Or(alternatives) => {
                for alternative in alternatives {
                    let condition =
                        self.call("__not__", vec![probe.matched.clone()], None, &pattern.span)?;
                    let mut next = vec![(alternative.clone(), value.clone())];
                    next.extend_from_slice(tail);
                    self.pattern_branch(condition, |c| {
                        c.pattern_tasks(&next, bindings.clone(), probe, yes, budget)
                    })?;
                }
                Ok(())
            }
            Kind::Constructor(constructor, fields) => {
                let condition = match constructor {
                    Constructor::Variant(index) => Some(self.emit(
                        "Bool",
                        Rvalue::VariantTest {
                            owner: value.clone(),
                            index: *index,
                        },
                    )),
                    Constructor::Literal(_) => Some(self.call(
                        "__eq__",
                        vec![value.clone(), pattern.literal.clone().unwrap()],
                        None,
                        &pattern.span,
                    )?),
                    Constructor::Product => None,
                };
                let action = |c: &mut Self| {
                    let mut next = vec![];
                    for (index, field) in fields.iter().enumerate() {
                        if matches!(field.kind, Kind::Wild(None)) {
                            continue;
                        }
                        let expression = match constructor {
                            Constructor::Variant(variant) => Rvalue::VariantField {
                                owner: value.clone(),
                                variant: *variant,
                                index,
                            },
                            _ => Rvalue::Field {
                                owner: value.clone(),
                                index,
                            },
                        };
                        let item = c.emit(&field.ty, expression);
                        next.push((field.clone(), item));
                    }
                    next.extend_from_slice(tail);
                    c.pattern_tasks(&next, bindings, probe, yes, budget)
                };
                if let Some(condition) = condition {
                    self.pattern_branch(condition, action)
                } else {
                    action(self)
                }
            }
        }
    }
    pub(in crate::compiler) fn pattern_condition(
        &mut self,
        condition: &Condition,
    ) -> Result<(Value, Vec<(Name, Value)>), String> {
        match condition {
            Condition::Expr(e) => Ok((self.condition(e)?, vec![])),
            Condition::Let(pattern, source) => {
                let value = self.expr(source, None)?;
                if self.diverged {
                    return Ok((Value::never(), vec![]));
                }
                let pattern = self.resolve_pattern(pattern, &value.ty)?;
                let probe = self.pattern_probe(&pattern, value)?;
                Ok((probe.matched, probe.bindings))
            }
        }
    }
    pub(in crate::compiler) fn scoped_pattern_values(
        &mut self,
        bindings: &[(Name, Value)],
        action: impl FnOnce(&mut Self) -> Result<Value, String>,
    ) -> Result<Value, String> {
        if bindings.is_empty() {
            return action(self);
        }
        let outer = std::mem::take(&mut self.pending_temps);
        self.scopes.push(BTreeMap::new());
        let mut result = Value::never();
        let slot = self.temp();
        let body = self.capture(|c| {
            for (name, value) in bindings {
                c.declare(name, value.ty.clone(), Some(value.clone()))?;
            }
            result = action(c)?;
            if !c.diverged {
                c.body.push(Instruction::Rebind {
                    span: None,
                    target: Place::Local(slot.clone()),
                    value: result.clone(),
                });
                c.release_temps();
            }
            Ok(())
        })?;
        self.scopes.pop();
        self.pending_temps = outer;
        if self.diverged {
            self.body.push(Instruction::Scope(body));
            return Ok(Value::never());
        }
        result.id = slot;
        self.body.push(Instruction::Declare {
            result: result.clone(),
            initial: None,
        });
        self.body.push(Instruction::Scope(body));
        self.pending_temps.push(result.id.clone());
        Ok(result)
    }
    pub(in crate::compiler) fn pattern_statement(
        &mut self,
        bindings: &[(Name, Value)],
        statement: &Stmt,
        span: &Span,
    ) -> Result<(), String> {
        if bindings.is_empty() {
            return self.statement(statement);
        }
        self.scoped_pattern_values(bindings, |c| {
            c.statement(statement)?;
            if c.diverged {
                Ok(Value::never())
            } else {
                c.literal("Unit", "", span)
            }
        })?;
        Ok(())
    }
    pub(in crate::compiler) fn match_expression(
        &mut self,
        source: &Expr,
        arms: &[crate::ast::MatchArm],
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Value, String> {
        let value = self.expr(source, None)?;
        if self.diverged {
            return Ok(Value::never());
        }
        let mut coverage = vec![];
        let mut typed = vec![];
        for arm in arms {
            let pattern = self.resolve_pattern(&arm.pattern, &value.ty)?;
            if self
                .pattern_witness(&coverage, std::slice::from_ref(&pattern))?
                .is_none()
            {
                return Err(pattern
                    .span
                    .error("unreachable match arm (duplicate or covered pattern)"));
            }
            if arm.guard.is_none() {
                coverage.push(vec![pattern.clone()]);
            }
            typed.push(pattern);
        }
        let wildcard = TypedPattern::wild(&value.ty, span);
        if let Some(missing) = self.pattern_witness(&coverage, &[wildcard])? {
            return Err(span.error(format!(
                "non-exhaustive match; missing: {}",
                missing.join(", ")
            )));
        }
        let probes = typed
            .iter()
            .map(|p| self.pattern_probe(p, value.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        self.pattern_arms(arms, &probes, expected, span)
    }
    fn pattern_arms(
        &mut self,
        arms: &[crate::ast::MatchArm],
        probes: &[Probe],
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Value, String> {
        if arms.len() == 1 {
            return self
                .scoped_pattern_values(&probes[0].bindings, |c| c.expr(&arms[0].body, expected));
        }
        let mut after_guard = None;
        let mut guard_exits = false;
        let condition = if let Some(guard) = &arms[0].guard {
            self.conditional(
                probes[0].matched.clone(),
                Some("Bool"),
                |c, _| {
                    let value = c.scoped_pattern_values(&probes[0].bindings, |c| {
                        let result = c.condition(guard)?;
                        if !c.diverged {
                            let writeback = c.capture(|c| {
                                for (name, slot) in &probes[0].bindings {
                                    let current = c.find_var(name)?;
                                    c.body.push(Instruction::Rebind {
                                        span: None,
                                        target: Place::Local(slot.id.clone()),
                                        value: Value {
                                            id: current.id,
                                            ty: current.ty,
                                        },
                                    });
                                }
                                Ok(())
                            })?;
                            c.body.push(Instruction::If {
                                condition: result.clone(),
                                yes: writeback,
                                no: Body::default(),
                            });
                        }
                        Ok(result)
                    })?;
                    guard_exits = c.diverged;
                    after_guard = Some(c.scopes.clone());
                    Ok(value)
                },
                |c, _| c.literal("Bool", "false", span),
                span,
            )?
        } else {
            probes[0].matched.clone()
        };
        self.conditional(
            condition,
            expected,
            |c, h| {
                if let Some(scopes) = after_guard {
                    c.scopes = scopes;
                }
                c.scoped_pattern_values(&probes[0].bindings, |c| {
                    if guard_exits {
                        c.diverged = true;
                        c.check_unreachable(&Stmt::Expr(arms[0].body.clone()))?;
                        c.body.push(Instruction::Unreachable);
                        Ok(Value::never())
                    } else {
                        c.expr(&arms[0].body, h)
                    }
                })
            },
            |c, h| c.pattern_arms(&arms[1..], &probes[1..], h, span),
            span,
        )
    }
}
