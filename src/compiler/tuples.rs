use super::*;

pub(super) fn type_parts(ty: &str) -> Option<(&str, Vec<&str>)> {
    let (base, rest) = ty.split_once('<')?;
    let inner = rest.strip_suffix('>')?;
    let mut depth = 0usize;
    let mut start = 0;
    let mut parts = vec![];
    for (index, c) in inner.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                parts.push(&inner[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    parts.push(&inner[start..]);
    Some((base, parts))
}

impl Compiler {
    pub(super) fn specialize_tuple(&mut self, name: &Name) -> Result<(), String> {
        let (_, types) =
            type_parts(&name.text).ok_or_else(|| name.span.error("invalid tuple type"))?;
        if types.len() < 2 {
            return Err(name.span.error("tuple types require at least two elements"));
        }
        let mut fields = vec![];
        for (index, ty) in types.iter().enumerate() {
            let ty = Name {
                text: (*ty).into(),
                span: name.span.clone(),
            };
            self.check_type(&ty)?;
            fields.push((
                Name {
                    text: index.to_string(),
                    span: name.span.clone(),
                },
                ty,
            ));
        }
        self.types.insert(
            name.text.clone(),
            format!("RuskRecord{}", self.records.len()),
        );
        self.records.insert(
            name.text.clone(),
            records::Record {
                name: name.clone(),
                fields,
            },
        );
        self.prepare_record(&name.text)
    }

    pub(super) fn tuple_value(
        &mut self,
        items: &[Expr],
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Value, String> {
        let hints = expected
            .and_then(type_parts)
            .filter(|(base, _)| *base == "Tuple")
            .map(|(_, args)| args);
        let mut values = vec![];
        for (index, item) in items.iter().enumerate() {
            values.push(self.expr(item, hints.as_ref().and_then(|h| h.get(index).copied()))?);
        }
        self.pack_tuple(values, span)
    }

    pub(super) fn tuple_item(
        &mut self,
        tuple: &Value,
        index: usize,
        span: &Span,
    ) -> Result<Value, String> {
        if !tuple.ty.starts_with("Tuple<") {
            return Err(span.error(format!("expected a tuple, found '{}'", tuple.ty)));
        }
        let ty = self.records[&tuple.ty]
            .fields
            .get(index)
            .ok_or_else(|| span.error("tuple index out of range"))?
            .1
            .text
            .clone();
        Ok(self.emit(
            &ty,
            Rvalue::Field {
                owner: tuple.clone(),
                index,
            },
        ))
    }

    pub(super) fn declare_binding(
        &mut self,
        binding: &Binding,
        value: Value,
    ) -> Result<(), String> {
        match binding {
            Binding::Name(name) if name.text == "_" => Ok(()),
            Binding::Name(name) => self.declare_annotated(name, value.ty.clone(), Some(value)),
            Binding::Tuple(bindings, span) => {
                if !value.ty.starts_with("Tuple<") {
                    return Err(span.error(format!("expected a tuple, found '{}'", value.ty)));
                }
                if bindings.len() != self.records[&value.ty].fields.len() {
                    return Err(span.error("tuple binding arity mismatch"));
                }
                for (index, binding) in bindings.iter().enumerate() {
                    let item = self.tuple_item(&value, index, binding.span())?;
                    self.declare_binding(binding, item)?;
                }
                Ok(())
            }
        }
    }

    pub(super) fn automatic_value(&self, ty: &str) -> bool {
        if let Some(inner) = vector_element(ty) {
            return self.automatic_value(inner);
        }
        if ty.starts_with("Tuple<") {
            return self
                .records
                .get(ty)
                .is_some_and(|r| r.fields.iter().all(|(_, t)| self.automatic_value(&t.text)));
        }
        matches!(ty, "Int" | "Bool" | "Char" | "Str")
    }

    pub(super) fn prepare_tuple_functions(
        &mut self,
        record: &records::Record,
    ) -> Result<(), String> {
        let ty = &record.name.text;
        if !ty.starts_with("Tuple<") || !self.automatic_value(ty) {
            return Ok(());
        }
        let native = &self.types[ty];
        for (name, expression) in [
            ("__eq__", "value_equal(a,b)"),
            ("__ne__", "!value_equal(a,b)"),
            ("__lt__", "value_less(a,b)"),
            ("__le__", "!value_less(b,a)"),
            ("__gt__", "value_less(b,a)"),
            ("__ge__", "!value_less(a,b)"),
        ] {
            let cpp = format!("{name}_{native}");
            self.functions.push(Function { name: Name { text:name.into(), span:record.name.span.clone() }, params:vec![ty.clone();2], result:"Bool".into(), cpp:cpp.clone(), code:format!("Obj<bool> {cpp}(Obj<{native}> a, Obj<{native}> b, Loc) {{ return make_obj<bool>({expression}); }}"), body:None, checked:None });
        }
        let cpp = format!("write_{native}");
        self.functions.push(Function {
            name: Name {
                text: "__write__".into(),
                span: record.name.span.clone(),
            },
            params: vec![ty.clone()],
            result: "Unit".into(),
            cpp: cpp.clone(),
            code: format!("void {cpp}(Obj<{native}> value, Loc) {{ write_value(value); }}"),
            body: None,
            checked: None,
        });
        Ok(())
    }

    pub(super) fn tuple_cpp(&self) -> String {
        let tuples: Vec<_> = self
            .records
            .iter()
            .filter(|(ty, _)| ty.starts_with("Tuple<") && self.automatic_value(ty))
            .collect();
        let mut code = String::new();
        for (ty, _) in &tuples {
            let native = &self.types[*ty];
            code.push_str(&format!("bool operator==(const {native}& a, const {native}& b);\nbool operator<(const {native}& a, const {native}& b);\nvoid write_value(const Obj<{native}>& value);\n"));
        }
        for (ty, record) in tuples {
            let native = &self.types[ty];
            code.push_str(&format!(
                "bool operator==(const {native}& a, const {native}& b) {{ return {} ; }}\n",
                (0..record.fields.len())
                    .map(|i| format!("value_equal(a.field_{i},b.field_{i})"))
                    .collect::<Vec<_>>()
                    .join(" && ")
            ));
            code.push_str(&format!(
                "bool operator<(const {native}& a, const {native}& b) {{\n"
            ));
            for i in 0..record.fields.len() {
                code.push_str(&format!("if(value_less(a.field_{i},b.field_{i})) return true; if(value_less(b.field_{i},a.field_{i})) return false;\n"));
            }
            code.push_str("return false; }\n");
            code.push_str(&format!(
                "void write_value(const Obj<{native}>& value) {{\n"
            ));
            for i in 0..record.fields.len() {
                if i > 0 {
                    code.push_str("rusk_write(' ');\n");
                }
                code.push_str(&format!("write_value(value->field_{i});\n"));
            }
            code.push_str("}\n");
        }
        code
    }
}

pub(super) fn literal_index(index: &Expr) -> Result<usize, String> {
    match &index.kind {
        ExprKind::Literal(ty, text) if ty == "Int" => text
            .parse::<usize>()
            .map_err(|_| index.span.error("tuple index out of range")),
        _ => Err(index.span.error("tuple index must be an integer literal")),
    }
}

impl Compiler {
    pub(super) fn assign_tuple_value(
        &mut self,
        op: &str,
        left: &Expr,
        value: Value,
    ) -> Result<Value, String> {
        if value.is_never() {
            return Ok(Value::never());
        }

        if let ExprKind::Tuple(targets) = &left.kind {
            if !value.ty.starts_with("Tuple<") {
                return Err(left.span.error("tuple assignment requires a tuple value"));
            }
            if targets.len() != self.records[&value.ty].fields.len() {
                return Err(left.span.error("tuple assignment arity mismatch"));
            }
            let mut values = vec![];
            for (index, target) in targets.iter().enumerate() {
                let item = self.tuple_item(&value, index, &target.span)?;
                values.push(self.assign_tuple_value(op, target, item)?);
            }
            return self.pack_tuple(values, &left.span);
        }
        if op == "=" {
            let target = self.expr(left, None)?;
            self.same(&target.ty, &value.ty, &left.span)?;
            self.call("__assign__", vec![target.clone(), value], None, &left.span)?;
            return Ok(target);
        }
        match &left.kind {
            ExprKind::Var(name) => {
                let target = self.find_var(name)?;
                self.same(&target.ty, &value.ty, &left.span)?;
                self.check_fixed_rebind(&target, &left.span)?;
                self.body.push(Instruction::Rebind {
                    span: Some(left.span.clone()),
                    target: Place::Local(target.id),
                    value: value.clone(),
                });
                for scope in self.scopes.iter_mut().rev() {
                    if let Some(variable) = scope.get_mut(&name.text) {
                        variable.initialized = true;
                        break;
                    }
                }
                Ok(value)
            }
            ExprKind::Field(owner, name) => {
                let owner = self.expr(owner, None)?;
                let (index, ty) = self.field(&owner.ty, name)?;
                self.same(&ty, &value.ty, &left.span)?;
                self.body.push(Instruction::Rebind {
                    span: Some(left.span.clone()),
                    target: Place::Field { owner, index },
                    value: value.clone(),
                });
                Ok(value)
            }
            ExprKind::Index(owner, index) => {
                let owner = self.expr(owner, None)?;
                if owner.ty.starts_with("Tuple<") {
                    let position = literal_index(index)?;
                    let item = self.tuple_item(&owner, position, &index.span)?;
                    self.same(&item.ty, &value.ty, &left.span)?;
                    self.body.push(Instruction::Rebind {
                        span: Some(left.span.clone()),
                        target: Place::Field {
                            owner,
                            index: position,
                        },
                        value: value.clone(),
                    });
                    Ok(value)
                } else {
                    let index = self.expr(index, Some("Int"))?;
                    self.call("__setitem__", vec![owner, index, value], None, &left.span)
                }
            }
            _ => Err(left
                .span
                .error("rebinding requires a variable, field or index access")),
        }
    }

    fn pack_tuple(&mut self, values: Vec<Value>, span: &Span) -> Result<Value, String> {
        let name = Name {
            text: format!(
                "Tuple<{}>",
                values
                    .iter()
                    .map(|v| v.ty.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            span: span.clone(),
        };
        self.check_type(&name)?;
        Ok(self.emit(&name.text, Rvalue::Aggregate(values)))
    }
}
