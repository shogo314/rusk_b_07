use super::*;

#[derive(Clone)]
pub(super) struct Record {
    pub name: Name,
    pub fields: Vec<(Name, Name)>,
}

impl Compiler {
    pub(super) fn prepare_records(&mut self) -> Result<(), String> {
        let names: Vec<_> = self.records.keys().cloned().collect();
        for name in names {
            self.prepare_record(&name)?;
        }
        Ok(())
    }
    pub(super) fn prepare_record(&mut self, name: &str) -> Result<(), String> {
        if self.prepared_records.contains(name) {
            return Ok(());
        }
        let record = self.records[name].clone();
        if record.fields.is_empty() && !name.starts_with("__RuskCallable") {
            return Err(record
                .name
                .span
                .error("a struct currently requires at least one field"));
        }
        let mut names = BTreeSet::new();
        for (name, _) in &record.fields {
            if !names.insert(&name.text) {
                return Err(name.span.error(format!("duplicate field '{}'", name.text)));
            }
        }
        self.check_record_cycle(&record.name, &mut vec![])?;
        for (_, ty) in &record.fields {
            self.check_type(ty)?;
        }
        let native = self.types[&record.name.text].clone();
        let copy = format!("copy_{native}");
        let assign = format!("assign_{native}");
        for (name, params, result, cpp, code) in [
            (
                "copy",
                vec![record.name.text.clone()],
                record.name.text.clone(),
                copy.clone(),
                format!(
                    "Obj<{native}> {copy}(Obj<{native}> value, Loc loc) {{ return deep_copy(value,loc); }}"
                ),
            ),
            (
                "__assign__",
                vec![record.name.text.clone(); 2],
                "Unit".into(),
                assign.clone(),
                format!(
                    "void {assign}(Obj<{native}> target, Obj<{native}> source, Loc loc) {{ auto snapshot=deep_copy(source,loc); *target=std::move(*snapshot); }}"
                ),
            ),
        ] {
            self.functions.push(Function {
                name: Name {
                    text: name.into(),
                    span: record.name.span.clone(),
                },
                params,
                result,
                cpp,
                code,
                body: None,
                checked: None,
            });
        }
        self.prepared_records.insert(name.into());
        self.prepare_tuple_functions(&record)?;
        Ok(())
    }

    fn check_record_cycle(&self, name: &Name, path: &mut Vec<String>) -> Result<(), String> {
        let Some(record) = self.records.get(&name.text) else {
            if let Some((_, args)) = super::tuples::type_parts(&name.text) {
                for arg in args {
                    self.check_record_cycle(
                        &Name {
                            text: arg.into(),
                            span: name.span.clone(),
                        },
                        path,
                    )?;
                }
            }
            return Ok(());
        };
        if path.contains(&name.text) {
            path.push(name.text.clone());
            return Err(name.span.error(format!(
                "recursive structs are not implemented yet: {}",
                path.join(" -> ")
            )));
        }
        path.push(name.text.clone());
        for (_, ty) in &record.fields {
            let mut inner = ty.text.as_str();
            while let Some(element) = vector_element(inner) {
                inner = element;
            }
            self.check_record_cycle(
                &Name {
                    text: inner.into(),
                    span: ty.span.clone(),
                },
                path,
            )?;
        }
        path.pop();
        Ok(())
    }

    pub(super) fn record_element(&self, ty: &str) -> bool {
        let mut inner = ty;
        while let Some(element) = vector_element(inner) {
            inner = element;
        }
        !self.automatic_value(inner)
    }

    pub(super) fn record_cpp(&self) -> String {
        let mut code = String::new();
        for name in self.records.keys() {
            code.push_str(&format!("struct {};\n", self.types[name]));
        }
        for (name, record) in &self.records {
            let native = &self.types[name];
            code.push_str(&format!("struct {native} {{\n"));
            for (index, (_, ty)) in record.fields.iter().enumerate() {
                code.push_str(&format!("Obj<{}> field_{index};\n", self.types[&ty.text]));
            }
            code.push_str("};\n");
        }
        code.push_str(&self.tuple_cpp());
        for name in self.records.keys() {
            let native = &self.types[name];
            code.push_str(&format!("Obj<{native}> deep_copy(const Obj<{native}>& value, Loc loc = Loc{{\"<copy>\",0,0}});\n"));
        }
        code
    }

    pub(super) fn prepare_record_copies(&mut self) -> Result<(), String> {
        let records: Vec<_> = self
            .records
            .values()
            .filter(|r| !self.record_copies.contains_key(&r.name.text))
            .cloned()
            .collect();
        for record in records {
            let mut copies = vec![];
            for (_, ty) in &record.fields {
                let value = Value {
                    id: ValueId::Probe,
                    ty: ty.text.clone(),
                };
                let index = self
                    .resolve("copy", &[value], None, &ty.span)?
                    .ok_or_else(|| {
                        ty.span
                            .error(format!("field type '{}' has no copy function", ty.text))
                    })?;
                self.same(&ty.text, &self.functions[index].result, &ty.span)?;
                self.used.insert(index);
                copies.push(index);
            }
            self.record_copies.insert(record.name.text, copies);
        }
        Ok(())
    }

    pub(super) fn record_copy_cpp(&self) -> String {
        let mut code = String::new();
        for name in self.records.keys() {
            let native = &self.types[name];
            let values = self.record_copies[name]
                .iter()
                .enumerate()
                .map(|(i, &function)| {
                    format!("{}(value->field_{i},loc)", self.functions[function].cpp)
                })
                .collect::<Vec<_>>()
                .join(",");
            code.push_str(&format!("Obj<{native}> deep_copy(const Obj<{native}>& value, Loc loc) {{ return make_obj<{native}>({native}{{{values}}}); }}\n"));
        }
        code
    }

    pub(super) fn field(&self, ty: &str, name: &Name) -> Result<(usize, String), String> {
        if ty == crate::hir::DIVERGED {
            return Err(crate::hir::DIVERGED.into());
        }
        if self.callables.contains_key(ty) {
            return Err(name.span.error("callable environment fields are private"));
        }
        let record = self
            .records
            .get(ty)
            .ok_or_else(|| name.span.error(format!("type '{ty}' has no fields")))?;
        record
            .fields
            .iter()
            .enumerate()
            .find(|(_, (n, _))| n.text == name.text)
            .map(|(index, (_, ty))| (index, ty.text.clone()))
            .ok_or_else(|| {
                name.span
                    .error(format!("unknown field '{}' on '{ty}'", name.text))
            })
    }

    pub(super) fn record_value(
        &mut self,
        name: &Name,
        fields: &[(Name, Expr)],
    ) -> Result<Value, String> {
        self.check_type(name)?;
        let record = self
            .records
            .get(&name.text)
            .cloned()
            .ok_or_else(|| name.span.error(format!("unknown struct '{}'", name.text)))?;
        let mut values = BTreeMap::new();
        for (field, expr) in fields {
            let (index, ty) = self.field(&name.text, field)?;
            if values.contains_key(&index) {
                return Err(field
                    .span
                    .error(format!("duplicate field '{}'", field.text)));
            }
            let value = self.expr(expr, Some(&ty))?;
            self.same(&ty, &value.ty, &expr.span)?;
            values.insert(index, value);
        }
        for (index, (field, _)) in record.fields.iter().enumerate() {
            if !values.contains_key(&index) {
                return Err(name
                    .span
                    .error(format!("missing field '{}' in '{}'", field.text, name.text)));
            }
        }
        Ok(self.emit(
            &name.text,
            Rvalue::Aggregate(values.into_values().collect()),
        ))
    }
}
