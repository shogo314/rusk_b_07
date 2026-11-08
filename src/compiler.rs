use crate::ast::{
    Binding, BindingMode, Condition, DeclaredName, Expr, ExprKind, Initializer, Name, Pattern,
    PatternKind, Span, Stmt,
};
use crate::hir::{Body, Call, Instruction, Literal, Place, Rvalue, Value, ValueId};
use std::collections::{BTreeMap, BTreeSet};
mod bindings;
mod callables;
mod containers;
mod cpp;
mod effects;
mod enums;
mod expressions;
mod flow;
mod generics;
mod higher_order;
mod ir_types;
mod patterns;
mod records;
mod tuples;

#[derive(Clone)]
struct Function {
    name: Name,
    params: Vec<String>,
    result: String,
    cpp: String,
    code: String,
    body: Option<(Vec<DeclaredName>, Box<Stmt>)>,
    checked: Option<Body>,
}
#[derive(Clone)]
struct Variable {
    id: ValueId,
    ty: String,
    initialized: bool,
    binding: Option<crate::hir::BindingInfo>,
    access: Option<crate::hir::AccessInfo>,
}
#[derive(Clone)]
struct LoopContext {
    id: usize,
    label: Option<Name>,
    broken: Option<Value>,
    break_seen: bool,
    break_states: Vec<Vec<BTreeMap<String, Variable>>>,
}
#[derive(Default)]
pub struct Compiler {
    types: BTreeMap<String, String>,
    enum_definitions: BTreeMap<String, enums::Definition>,
    enums: BTreeMap<String, enums::Enum>,
    enum_copies: BTreeMap<String, Vec<Vec<usize>>>,
    preparing_enums: Vec<String>,
    containers: BTreeMap<String, containers::Container>,
    container_copies: BTreeMap<String, Vec<usize>>,
    container_functions: BTreeMap<String, usize>,
    callables: BTreeMap<String, callables::Callable>,
    function_types: BTreeMap<usize, String>,
    lambda_types: BTreeMap<String, String>,
    iterator_types: BTreeMap<String, higher_order::IteratorType>,
    lambda_counter: usize,
    capture_contexts: Vec<callables::CaptureContext>,
    current_function: Option<usize>,
    records: BTreeMap<String, records::Record>,
    prepared_records: BTreeSet<String>,
    record_copies: BTreeMap<String, Vec<usize>>,
    generic_records: BTreeMap<String, generics::GenericRecord>,
    generic_functions: Vec<generics::GenericFunction>,
    generic_instances: BTreeMap<(usize, Vec<String>), usize>,
    generic_calls: BTreeMap<usize, Span>,
    functions: Vec<Function>,
    native_contracts: BTreeMap<usize, crate::effects::Summary>,
    symbols: BTreeMap<String, String>,
    scopes: Vec<BTreeMap<String, Variable>>,
    body: Body,
    no_optimization: bool,
    counter: usize,
    used: BTreeSet<usize>,
    loops: Vec<LoopContext>,
    diverged: bool,
    infer_return: bool,
    inferred_return: Option<String>,
    return_type: Option<String>,
    pending_temps: Vec<ValueId>,
}
fn property(n: &Name, p: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    p.get(key)
        .cloned()
        .ok_or_else(|| n.span.error(format!("missing property '{key}'")))
}
fn validate(n: &Name, p: &BTreeMap<String, String>, allowed: &[&str]) -> Result<(), String> {
    for key in p.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(n.span.error(format!("unknown property '{key}'")));
        }
    }
    Ok(())
}
impl Compiler {
    pub fn with_optimization(enabled: bool) -> Self {
        Self {
            no_optimization: !enabled,
            ..Self::default()
        }
    }
    pub fn compile(
        mut self,
        sources: &[Vec<Stmt>],
        emit: crate::backend::Emit,
    ) -> Result<String, String> {
        for source in sources {
            for s in source {
                if let Stmt::UserFunction(name, parameters, arguments, result, body) = s
                    && !parameters.is_empty()
                {
                    self.register_generic_function(generics::GenericFunction {
                        name: name.clone(),
                        parameters: parameters.clone(),
                        arguments: arguments.clone(),
                        result: result.clone(),
                        body: body.clone(),
                    })?;
                }
                if let Stmt::Struct(name, parameters, fields) = s {
                    if name.text.starts_with("__Rusk")
                        || matches!(
                            name.text.as_str(),
                            "Vec" | "Tuple" | "Iter" | "Option" | "Dict" | "Set"
                        )
                    {
                        return Err(name.span.error("reserved built-in type constructor"));
                    }
                    if !parameters.is_empty() {
                        self.register_generic_record(generics::GenericRecord {
                            name: name.clone(),
                            parameters: parameters.clone(),
                            fields: fields.clone(),
                        })?;
                        continue;
                    }
                    let native = format!("RuskRecord{}", self.records.len());
                    if self.generic_records.contains_key(&name.text)
                        || self.types.insert(name.text.clone(), native).is_some()
                    {
                        return Err(name.span.error("duplicate type"));
                    }
                    self.records.insert(
                        name.text.clone(),
                        records::Record {
                            name: name.clone(),
                            fields: fields.clone(),
                        },
                    );
                }
                if let Stmt::Type(n, p) = s {
                    if n.text.starts_with("__Rusk") {
                        return Err(n.span.error("reserved internal type name"));
                    }
                    if self.generic_records.contains_key(&n.text) {
                        return Err(n.span.error("duplicate type"));
                    }
                    validate(n, p, &["cpp_name"])?;
                    if self
                        .types
                        .insert(n.text.clone(), property(n, p, "cpp_name")?)
                        .is_some()
                    {
                        return Err(n.span.error("duplicate type"));
                    }
                }
            }
        }
        for source in sources {
            for s in source {
                if let Stmt::Enum(name, parameters, variants) = s {
                    self.register_enum(name, parameters, variants)?;
                }
            }
        }
        let enum_names = self
            .enum_definitions
            .values()
            .filter(|d| d.parameters.is_empty())
            .map(|d| d.name.clone())
            .collect::<Vec<_>>();
        for name in enum_names {
            self.check_type(&name)?;
        }
        self.prepare_records()?;
        for source in sources {
            for s in source {
                if let Stmt::UserFunction(name, ..) = s
                    && matches!(
                        name.text.as_str(),
                        "Some" | "None" | "Option" | "Dict" | "Set" | "try_read"
                    )
                {
                    return Err(name.span.error("reserved built-in constructor name"));
                }
                if let Stmt::UserFunction(_, parameters, ..) = s
                    && !parameters.is_empty()
                {
                    continue;
                }
                match s {
                    Stmt::Function(n, params, result, _)
                    | Stmt::UserFunction(n, _, params, result, _) => {
                        let (cpp, code, body) = match s {
                            Stmt::Function(_, _, _, p) => {
                                validate(n, p, &["cpp_name", "code", "effects"])?;
                                (property(n, p, "cpp_name")?, property(n, p, "code")?, None)
                            }
                            Stmt::UserFunction(_, _, params, _, body) => (
                                format!("rusk_fn{}", self.functions.len()),
                                String::new(),
                                Some((
                                    params.iter().map(|(n, _)| n.clone()).collect(),
                                    body.clone(),
                                )),
                            ),
                            _ => unreachable!(),
                        };
                        let mut names = BTreeSet::new();
                        for (param, ty) in params {
                            if matches!(s, Stmt::Function(..))
                                && let Some(annotation) = &param.binding
                            {
                                return Err(annotation.span.error("binding modifiers on extern parameters require a Rusk function body"));
                            }
                            if matches!(s, Stmt::Function(..))
                                && let Some(annotation) = &param.access
                            {
                                return Err(annotation.span.error("access modifiers on extern parameters require a Rusk function body"));
                            }
                            if matches!(s, Stmt::Function(..))
                                && let Some(annotation) = &param.escape
                            {
                                return Err(annotation.span.error("escape modifiers on extern parameters require a Rusk function body"));
                            }
                            self.check_type(ty)?;
                            if !names.insert(&param.text) {
                                return Err(param.span.error("duplicate parameter"));
                            }
                        }
                        if let Some(r) = result {
                            self.check_type(r)?;
                        }
                        let result = result
                            .as_ref()
                            .map(|r| r.text.clone())
                            .unwrap_or("Unit".into());
                        let params: Vec<_> = params.iter().map(|(_, t)| t.text.clone()).collect();
                        if let Some(old) = self.functions.iter().find(|f| {
                            f.name.text == n.text
                                && f.params == params
                                && (!return_selected(&n.text) || f.result == result)
                        }) {
                            return Err(n.span.error(format!(
                                "duplicate function; previous definition at {}:{}:{}",
                                old.name.span.file, old.name.span.line, old.name.span.column
                            )));
                        }
                        if let Stmt::Function(_, _, _, properties) = s
                            && let Some(effects) =
                                self.declared_native_effects(n, &params, &result, properties)?
                        {
                            self.native_contracts.insert(self.functions.len(), effects);
                        }
                        self.functions.push(Function {
                            name: n.clone(),
                            params,
                            result,
                            cpp,
                            code,
                            body,
                            checked: None,
                        });
                    }
                    Stmt::Symbol(n, p) => {
                        validate(n, p, &["code"])?;
                        if self
                            .symbols
                            .insert(n.text.clone(), property(n, p, "code")?)
                            .is_some()
                        {
                            return Err(n.span.error("duplicate symbol"));
                        }
                    }
                    _ => {}
                }
            }
        }
        // Check every body, including unused functions, in an isolated variable scope.
        for i in 0..self.functions.len() {
            if self.functions[i].body.is_some() {
                self.compile_function(i)?;
            }
        }
        self.scopes.push(BTreeMap::new());
        for source in sources {
            for s in source {
                self.statement(s)?;
            }
        }
        let main_body = std::mem::take(&mut self.body);
        self.diverged = false;
        let mut index = 0;
        loop {
            while index < self.functions.len() {
                if self.functions[index].body.is_some()
                    && self.functions[index].checked.is_none()
                    && let Err(error) = self.compile_function(index)
                {
                    if let Some(span) = self.generic_calls.get(&index) {
                        return Err(format!(
                            "{error}\nwhile instantiating '{}' at {}:{}:{}",
                            self.generic_instances
                                .iter()
                                .find(|(_, value)| **value == index)
                                .map(|((template, arguments), _)| format!(
                                    "{}<{}>",
                                    self.generic_functions[*template].name.text,
                                    arguments.join(",")
                                ))
                                .unwrap_or_else(|| self.functions[index].name.text.clone()),
                            span.file,
                            span.line,
                            span.column
                        ));
                    }
                    return Err(error);
                }
                index += 1;
            }
            self.prepare_record_copies()?;
            self.prepare_container_copies()?;
            self.prepare_enum_copies()?;
            if index == self.functions.len()
                && self.record_copies.len() == self.records.len()
                && self.container_copies.len() == self.containers.len()
                && self.enum_copies.len() == self.enums.len()
            {
                break;
            }
        }
        self.body = main_body;
        self.body.push(Instruction::Return {
            value: None,
            span: None,
        });
        let mut hir = crate::hir::Program {
            types: self.ir_types(),
            functions: self
                .functions
                .iter()
                .enumerate()
                .map(|(id, f)| crate::hir::Function {
                    id,
                    name: f.name.text.clone(),
                    params: f.params.clone(),
                    result: f.result.clone(),
                    body: f.checked.clone(),
                    access: crate::readonly::Summary::default(),
                    effects: self
                        .native_contracts
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| crate::effects::Summary::unanalyzed(f.params.len())),
                })
                .collect(),
            main: std::mem::take(&mut self.body),
        };
        hir.prune_unreachable();
        hir.validate()?;
        let mut lir = crate::lir::lower_program(&hir);
        lir.validate()?;
        let summaries = crate::effects::analyze(&lir);
        for (id, summary) in summaries.into_iter().enumerate() {
            hir.functions[id].effects = summary.clone();
            lir.functions[id].effects = summary;
        }
        hir.validate()?;
        lir.validate()?;
        crate::effects::validate_noescape(&lir)?;
        let access = crate::readonly::analyze(&lir)?;
        for (id, summary) in access.into_iter().enumerate() {
            hir.functions[id].access = summary.clone();
            lir.functions[id].access = summary;
        }
        hir.validate()?;
        lir.validate()?;
        if matches!(emit, crate::backend::Emit::Hir) {
            return Ok(format!("{hir:#?}\n"));
        }
        if matches!(emit, crate::backend::Emit::Lir) {
            return Ok(format!("{lir:#?}\n"));
        }
        Ok(crate::backend::emit_program(
            &lir,
            &self.cpp_context(),
            !self.no_optimization,
        ))
    }
    fn compile_function(&mut self, index: usize) -> Result<(), String> {
        self.current_function = Some(index);
        let f = self.functions[index].clone();
        let (params, body) = f.body.as_ref().unwrap();
        self.diverged = false;
        self.infer_return = false;
        self.return_type = Some(f.result.clone());
        self.scopes = vec![BTreeMap::new()];
        self.body.clear();
        for (i, (param, ty)) in params.iter().zip(&f.params).enumerate() {
            self.declare_annotated(
                param,
                ty.clone(),
                Some(Value {
                    id: ValueId::Parameter(i),
                    ty: ty.clone(),
                }),
            )?;
        }
        let Stmt::Block(statements, tail) = body.as_ref() else {
            unreachable!()
        };
        for statement in statements {
            self.statement(statement)?;
        }
        if let Some(tail) = tail {
            self.statement(&Stmt::Expr((**tail).clone()))?;
        }
        if f.result != "Unit" && !self.diverged {
            return Err(f
                .name
                .span
                .error("function may finish without returning a value"));
        }
        if f.result == "Unit" {
            self.body.push(Instruction::Return {
                value: None,
                span: None,
            });
        }
        self.functions[index].checked = Some(std::mem::take(&mut self.body));
        self.scopes.clear();
        self.body.clear();
        self.return_type = None;
        self.diverged = false;
        self.current_function = None;
        Ok(())
    }
    fn check_type(&mut self, n: &Name) -> Result<(), String> {
        if self.diverged {
            return Err(crate::hir::DIVERGED.into());
        }
        if self.types.contains_key(&n.text)
            || self.specialize_enum(n)?
            || self.specialize_container(n)?
        {
            Ok(())
        } else if n.text.starts_with("Tuple<") {
            self.specialize_tuple(n)
        } else if let Some(inner) = vector_element(&n.text) {
            let inner = inner.to_string();
            self.check_type(&Name {
                text: inner.clone(),
                span: n.span.clone(),
            })?;
            if !matches!(inner.as_str(), "Int" | "Bool" | "Char" | "Str")
                && vector_element(&inner).is_none()
                && !self.records.contains_key(&inner)
                && !self.containers.contains_key(&inner)
                && !self.enums.contains_key(&inner)
            {
                return Err(n
                    .span
                    .error("vectors require basic, struct or vector element types"));
            }
            self.specialize_vector(&n.text, &inner)?;
            Ok(())
        } else {
            if self.specialize_record(n)? {
                Ok(())
            } else {
                Err(n.span.error(format!("unknown type '{}'", n.text)))
            }
        }
    }
    fn specialize_vector(&mut self, ty: &str, inner: &str) -> Result<(), String> {
        let native_element = self.types[inner].clone();
        let native = format!("RuskVec<{native_element}>");
        let iterator = format!("Iter<{ty}>");
        let native_iterator = format!("RuskIter<{native_element}>");
        self.types.insert(ty.into(), native.clone());
        self.types.insert(iterator.clone(), native_iterator.clone());
        let prefix = format!("vector_{}_", self.functions.len());
        let source = include_str!("../lib/vec.rusk")
            .replace("NativeIterator", &native_iterator)
            .replace("NativeElement", &native_element)
            .replace("NativeVec", &native)
            .replace("Iterator", &iterator)
            .replace("Element", inner)
            .replace("Vector", ty)
            .replace("vector_function_", &prefix);
        for statement in crate::parse(&source, "<stdlib/vec.rusk>")? {
            if let Stmt::Function(name, params, result, properties) = statement {
                if name.text == "__read__"
                    && (vector_element(inner).is_some()
                        || self.records.contains_key(inner)
                        || self.containers.contains_key(inner)
                        || self.enums.contains_key(inner))
                {
                    continue;
                }
                if self.record_element(inner)
                    && matches!(
                        name.text.as_str(),
                        "__write__"
                            | "sort"
                            | "sorted"
                            | "bisect_left"
                            | "bisect_right"
                            | "contains"
                            | "__eq__"
                            | "__ne__"
                            | "__lt__"
                            | "__le__"
                            | "__gt__"
                            | "__ge__"
                    )
                {
                    continue;
                }
                let parameters = params
                    .into_iter()
                    .map(|(_, ty)| ty.text)
                    .collect::<Vec<_>>();
                let result = result.map_or("Unit".into(), |ty| ty.text);
                if let Some(effects) =
                    self.declared_native_effects(&name, &parameters, &result, &properties)?
                {
                    self.native_contracts.insert(self.functions.len(), effects);
                }
                self.functions.push(Function {
                    cpp: property(&name, &properties, "cpp_name")?,
                    code: property(&name, &properties, "code")?,
                    name,
                    params: parameters,
                    result,
                    body: None,
                    checked: None,
                });
            }
        }
        Ok(())
    }
    fn temp(&mut self) -> ValueId {
        let r = ValueId::Local(self.counter);
        self.counter += 1;
        r
    }
    fn emit(&mut self, ty: &str, code: Rvalue) -> Value {
        if self.diverged {
            return Value::never();
        }
        let cpp = self.temp();
        let value = Value {
            id: cpp.clone(),
            ty: ty.into(),
        };
        self.body.instructions.push(Instruction::Object {
            result: value.clone(),
            expression: code,
        });
        self.pending_temps.push(cpp);
        value
    }
    fn find_var(&mut self, n: &Name) -> Result<Variable, String> {
        self.lookup_variable(n, false)?
            .ok_or_else(|| n.span.error(format!("unknown variable '{}'", n.text)))
    }
    fn declare(&mut self, n: &Name, ty: String, value: Option<Value>) -> Result<(), String> {
        if self.scopes.last().unwrap().contains_key(&n.text) {
            return Err(n.span.error(format!("duplicate variable '{}'", n.text)));
        }
        if let Some(v) = &value {
            self.same(&ty, &v.ty, &n.span)?;
        }
        let cpp = self.temp();
        self.body.push(Instruction::Declare {
            result: Value {
                id: cpp.clone(),
                ty: ty.clone(),
            },
            initial: value.clone(),
        });
        self.body.push(Instruction::DescribeBinding {
            value: Value {
                id: cpp.clone(),
                ty: ty.clone(),
            },
            name: n.clone(),
            captured: false,
        });
        self.scopes.last_mut().unwrap().insert(
            n.text.clone(),
            Variable {
                id: cpp,
                ty,
                initialized: value.is_some(),
                binding: None,
                access: None,
            },
        );
        Ok(())
    }
    fn same(&self, a: &str, b: &str, s: &Span) -> Result<(), String> {
        if a == b || a == crate::hir::DIVERGED || b == crate::hir::DIVERGED {
            Ok(())
        } else {
            Err(s.error(format!("type mismatch: expected {a}, found {b}")))
        }
    }
    fn select(
        &self,
        name: &str,
        args: &[Value],
        expected: Option<&str>,
        s: &Span,
    ) -> Result<Option<usize>, String> {
        let ids: Vec<_> = self
            .functions
            .iter()
            .enumerate()
            .filter(|(index, f)| {
                !self.generic_calls.contains_key(index)
                    && f.name.text == name
                    && f.params == args.iter().map(|v| v.ty.clone()).collect::<Vec<_>>()
                    && (!return_selected(name) || expected.is_none_or(|t| f.result == t))
            })
            .map(|(i, _)| i)
            .collect();
        if ids.len() > 1 {
            return Err(s.error(format!("ambiguous call to '{name}'")));
        }
        Ok(ids.first().copied())
    }
    fn call(
        &mut self,
        name: &str,
        args: Vec<Value>,
        expected: Option<&str>,
        s: &Span,
    ) -> Result<Value, String> {
        if self.diverged {
            return Ok(Value::never());
        }
        if let Some(value) = self.enum_constructor(name, &args, expected, s)? {
            return Ok(value);
        }
        let constructor = self.container_constructor(name, &args, expected, s)?;
        let resolved = if constructor.is_some() {
            constructor
        } else {
            self.resolve(name, &args, expected, s)?
        };
        let resolved = if resolved.is_some() {
            resolved
        } else {
            self.collection_operation(name, &args, expected, s)?
        };
        let resolved = if resolved.is_some() {
            resolved
        } else {
            self.higher_order(name, &args, s)?
        };
        let i = resolved.ok_or_else(|| {
            s.error(format!(
                "no matching function '{name}' for ({})",
                args.iter()
                    .map(|v| v.ty.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        self.used.insert(i);
        let f = self.functions[i].clone();
        let result = Value {
            id: self.temp(),
            ty: f.result.clone(),
        };
        self.pending_temps.push(result.id.clone());
        self.body.push(Instruction::Call(Call {
            function: i,
            args,
            span: s.clone(),
            inherit_location: self
                .current_function
                .is_some_and(|i| self.functions[i].name.span.file == "<stdlib/core.rusk>"),
            result: result.clone(),
        }));
        Ok(result)
    }

    fn literal(&mut self, ty: &str, text: &str, s: &Span) -> Result<Value, String> {
        if self.diverged {
            return Ok(Value::never());
        }
        let _cpp = self
            .types
            .get(ty)
            .ok_or_else(|| s.error(format!("literal type '{ty}' is not defined")))?
            .clone();
        let value = match ty {
            "Int" => {
                let v = text
                    .parse::<i64>()
                    .map_err(|_| s.error("integer literal out of range"))?;
                Literal::Int(v)
            }
            "Str" => {
                let v: String = serde_json::from_str(text).map_err(|e| s.error(e))?;
                if !v.is_ascii() {
                    return Err(s.error("Str currently requires ASCII"));
                }
                Literal::Str(v)
            }
            "Char" => {
                let inner = &text[1..text.len() - 1];
                let c = if inner.starts_with('\\') {
                    match inner {
                        "\\n" => b'\n',
                        "\\r" => b'\r',
                        "\\t" => b'\t',
                        "\\0" => 0,
                        "\\\\" => b'\\',
                        "\\'" => b'\'',
                        "\\\"" => b'"',
                        _ => return Err(s.error("unsupported character escape")),
                    }
                } else if inner.len() == 1 && inner.is_ascii() {
                    inner.as_bytes()[0]
                } else {
                    return Err(s.error("Char requires one ASCII character"));
                };
                Literal::Char(c)
            }
            "Bool" => Literal::Bool(text == "true"),
            "Unit" => Literal::Unit,
            _ => return Err(s.error("unsupported literal")),
        };
        let result = Value {
            id: self.temp(),
            ty: ty.into(),
        };
        self.pending_temps.push(result.id.clone());
        self.body.instructions.push(Instruction::Literal {
            result: result.clone(),
            value,
        });
        Ok(result)
    }
    fn release_temps(&mut self) {
        self.body.push(Instruction::Release(std::mem::take(
            &mut self.pending_temps,
        )));
    }
    fn capture(
        &mut self,
        work: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<Body, String> {
        let saved = std::mem::take(&mut self.body);
        let result = work(self);
        let body = std::mem::replace(&mut self.body, saved);
        result?;
        Ok(body)
    }
    fn loop_id(&mut self) -> usize {
        let id = self.counter;
        self.counter += 1;
        id
    }
    fn statement(&mut self, s: &Stmt) -> Result<(), String> {
        if self.diverged {
            return self.check_unreachable(s);
        }
        let outer = std::mem::take(&mut self.pending_temps);
        self.statement_inner(s)?;
        self.release_temps();
        self.pending_temps = outer;
        Ok(())
    }
    fn statement_inner(&mut self, s: &Stmt) -> Result<(), String> {
        self.statement_labeled(s, None)
    }
    fn statement_labeled(&mut self, s: &Stmt, label: Option<Name>) -> Result<(), String> {
        match s {
            Stmt::Labeled(label, body) => {
                if self
                    .loops
                    .iter()
                    .any(|l| l.label.as_ref().is_some_and(|n| n.text == label.text))
                {
                    return Err(label.span.error("duplicate active loop label"));
                }
                self.statement_labeled(body, Some(label.clone()))?;
            }
            Stmt::Include(_, span) => {
                return Err(span.error("include is only allowed at top level"));
            }
            Stmt::Type(n, _)
            | Stmt::Struct(n, ..)
            | Stmt::Enum(n, ..)
            | Stmt::Function(n, ..)
            | Stmt::UserFunction(n, ..)
            | Stmt::Symbol(n, _) => {
                if self.return_type.is_some() || self.scopes.len() != 1 {
                    return Err(n
                        .span
                        .error("type and function declarations are currently top-level only"));
                }
            }
            Stmt::Return(span, value) => {
                let declared = self
                    .return_type
                    .clone()
                    .ok_or_else(|| span.error("return outside a function"))?;
                let hint = if self.infer_return {
                    self.inferred_return.clone()
                } else {
                    Some(declared)
                };
                let value = if let Some(value) = value {
                    Some(self.expr(value, hint.as_deref())?)
                } else {
                    None
                };
                if !self.diverged {
                    let ty = value.as_ref().map_or("Unit", |v| v.ty.as_str());
                    if let Some(hint) = &hint {
                        self.same(hint, ty, span)?;
                    }
                    if self.infer_return {
                        self.inferred_return = Some(ty.into());
                    }
                    self.body.push(Instruction::Return {
                        value: if ty == "Unit" { None } else { value },
                        span: Some(span.clone()),
                    });
                    self.diverged = true;
                }
            }
            Stmt::Let(n, t, e) => {
                if let Some(t) = t {
                    self.check_type(t)?;
                }
                let value = e
                    .as_ref()
                    .map(|initializer| {
                        let (e, copied) = match initializer {
                            Initializer::Shared(e) => (e, false),
                            Initializer::Copied(e) => (e, true),
                        };
                        let value = self.expr(e, t.as_ref().map(|n| n.text.as_str()))?;
                        if copied {
                            self.call("copy", vec![value], None, &e.span)
                        } else {
                            Ok(value)
                        }
                    })
                    .transpose()?;
                if self.diverged {
                    self.declare_dead_binding(
                        n,
                        t.as_ref().map_or(crate::hir::DIVERGED, |t| t.text.as_str()),
                    )?;
                    return Ok(());
                }
                let ty = t
                    .as_ref()
                    .map(|n| n.text.clone())
                    .or_else(|| value.as_ref().map(|v| v.ty.clone()))
                    .ok_or_else(|| n.span().error("uninitialized declaration requires a type"))?;
                if let Some(value) = value {
                    self.same(&ty, &value.ty, n.span())?;
                    self.declare_binding(n, value)?;
                } else if let Binding::Name(name) = n {
                    self.declare_annotated(name, ty, None)?;
                } else {
                    return Err(n.span().error("tuple binding requires an initializer"));
                }
            }
            Stmt::Input(items) => {
                for (n, t, args) in items {
                    self.check_type(t)?;
                    let mut values = vec![];
                    for arg in args {
                        values.push(self.expr(arg, Some("Int"))?);
                    }
                    if self.diverged {
                        return Ok(());
                    }
                    let v = self.call("__read__", values, Some(&t.text), &n.span)?;
                    self.declare(n, t.text.clone(), Some(v))?;
                }
            }
            Stmt::Output(items) => {
                let span = items.first().map(|e| e.span.clone()).unwrap_or(Span {
                    file: "<output>".into(),
                    line: 1,
                    column: 1,
                });
                for (i, e) in items.iter().enumerate() {
                    if i > 0 {
                        let space = self.literal("Char", "' '", &span)?;
                        self.call("__write__", vec![space], None, &span)?;
                    }
                    let v = self.expr(e, None)?;
                    self.call("__write__", vec![v], None, &e.span)?;
                }
                let newline = self.literal("Char", "'\\n'", &span)?;
                self.call("__write__", vec![newline], None, &span)?;
            }
            Stmt::If(cond, yes, no) => {
                let span = cond.span().clone();
                let (cond, bindings) = self.pattern_condition(cond)?;
                if self.diverged {
                    return Ok(());
                }
                let before = self.scopes.clone();
                let yes_body = self.capture(|c| c.pattern_statement(&bindings, yes, &span))?;
                let yes_state = self.scopes.clone();
                let yes_exit = self.diverged;
                self.scopes = before;
                self.diverged = false;
                let no_body = self.capture(|c| {
                    if let Some(no) = no {
                        c.statement(no)?;
                    }
                    Ok(())
                })?;
                let no_exit = self.diverged;
                self.body.push(Instruction::If {
                    condition: cond,
                    yes: yes_body,
                    no: no_body,
                });
                if yes_exit {
                } else if no_exit {
                    self.scopes = yes_state;
                } else {
                    self.intersect_initialization(&yes_state);
                }
                self.diverged = yes_exit && no_exit;
            }
            Stmt::While(cond, body, otherwise) => {
                let id = self.loop_id();
                let flag = if otherwise.is_some() {
                    Some(self.literal("Bool", "false", cond.span())?)
                } else {
                    None
                };
                self.loops.push(LoopContext {
                    id,
                    label,
                    broken: flag.clone(),
                    break_seen: false,
                    break_states: vec![],
                });
                let mut condition_exits = false;
                let outer = std::mem::take(&mut self.pending_temps);
                let mut before_body = self.scopes.clone();
                let loop_body = self.capture(|c| {
                    let (condition, bindings) = c.pattern_condition(cond)?;
                    before_body = c.scopes.clone();
                    condition_exits = c.diverged;
                    if c.diverged {
                        return Ok(());
                    }
                    c.body.push(Instruction::If {
                        condition,
                        yes: Body::default(),
                        no: Body {
                            instructions: vec![Instruction::Break(id)],
                        },
                    });
                    if matches!(cond, Condition::Expr(_)) {
                        c.release_temps();
                    }
                    c.pattern_statement(&bindings, body, cond.span())?;
                    c.release_temps();
                    Ok(())
                })?;
                let context = self.loops.pop().unwrap();
                self.diverged = false;
                self.pending_temps = outer;
                self.scopes = before_body;
                for state in &context.break_states {
                    self.intersect_initialization(state);
                }
                self.body.push(Instruction::Loop {
                    id,
                    body: loop_body,
                });
                if let Some(otherwise) = otherwise {
                    let before = self.scopes.clone();
                    let otherwise = self.capture(|c| c.statement(otherwise))?;
                    self.scopes = before;
                    self.body.push(Instruction::If {
                        condition: flag.unwrap(),
                        yes: Body::default(),
                        no: otherwise,
                    });
                }
                self.diverged = condition_exits && !context.break_seen;
            }
            Stmt::For(name, source, body, otherwise) => {
                let source = self.expr(source, None)?;
                if self.diverged {
                    return Ok(());
                }
                let iterator = self.call("__iter__", vec![source], None, name.span())?;
                let id = self.loop_id();
                let before = self.scopes.clone();
                let flag = if otherwise.is_some() {
                    Some(self.literal("Bool", "false", name.span())?)
                } else {
                    None
                };
                let outer = std::mem::take(&mut self.pending_temps);
                let loop_body = self.capture(|c| {
                    let has = c.call(
                        "__iter_has_next__",
                        vec![iterator.clone()],
                        None,
                        name.span(),
                    )?;
                    c.same("Bool", &has.ty, name.span())?;
                    c.body.push(Instruction::If {
                        condition: has,
                        yes: Body::default(),
                        no: Body {
                            instructions: vec![Instruction::Break(id)],
                        },
                    });
                    c.scopes.push(BTreeMap::new());
                    let value = c.call("__iter_next__", vec![iterator], None, name.span())?;
                    c.declare_binding(name, value)?;
                    c.release_temps();
                    c.loops.push(LoopContext {
                        id,
                        label: label.clone(),
                        broken: flag.clone(),
                        break_seen: false,
                        break_states: vec![],
                    });
                    let Stmt::Block(statements, tail) = body.as_ref() else {
                        unreachable!()
                    };
                    for statement in statements {
                        c.statement(statement)?;
                    }
                    if let Some(tail) = tail {
                        c.statement(&Stmt::Expr((**tail).clone()))?;
                    }
                    c.loops.pop();
                    c.scopes.pop();
                    Ok(())
                })?;
                self.diverged = false;
                self.pending_temps = outer;
                self.scopes = before;
                self.body.push(Instruction::Loop {
                    id,
                    body: loop_body,
                });
                if let Some(otherwise) = otherwise {
                    let before = self.scopes.clone();
                    let otherwise = self.capture(|c| c.statement(otherwise))?;
                    self.scopes = before;
                    self.body.push(Instruction::If {
                        condition: flag.unwrap(),
                        yes: Body::default(),
                        no: otherwise,
                    });
                }
            }
            Stmt::Loop(body) => {
                let before = self.scopes.clone();
                let id = self.loop_id();
                self.loops.push(LoopContext {
                    id,
                    label,
                    broken: None,
                    break_seen: false,
                    break_states: vec![],
                });
                let body = self.capture(|c| c.statement(body))?;
                let context = self.loops.pop().unwrap();
                self.scopes = before;
                self.diverged = !context.break_seen;
                self.body.push(Instruction::Loop { id, body });
            }
            Stmt::Break(span, label) => {
                let index = self.loop_target(label.as_ref(), span)?;
                let context = self.loops[index].clone();
                self.loops[index].break_seen = true;
                self.loops[index].break_states.push(self.scopes.clone());
                if let Some(flag) = context.broken {
                    let yes = self.literal("Bool", "true", span)?;
                    self.body.push(Instruction::StoreBool {
                        target: flag,
                        value: yes,
                    });
                }
                self.body.push(Instruction::Break(context.id));
                self.diverged = true;
            }
            Stmt::Continue(span, label) => {
                let index = self.loop_target(label.as_ref(), span)?;
                self.body.push(Instruction::Continue(self.loops[index].id));
                self.diverged = true;
            }
            Stmt::Block(statements, tail) => {
                self.scopes.push(BTreeMap::new());
                let body = self.capture(|c| {
                    for statement in statements {
                        c.statement(statement)?;
                    }
                    if let Some(tail) = tail {
                        c.statement(&Stmt::Expr((**tail).clone()))?;
                    }
                    Ok(())
                })?;
                self.scopes.pop();
                self.body.push(Instruction::Scope(body));
            }
            Stmt::Expr(e) | Stmt::Match(e) => {
                self.expr(e, None)?;
            }
        }
        Ok(())
    }
    fn condition(&mut self, expr: &Expr) -> Result<Value, String> {
        let outer = std::mem::take(&mut self.pending_temps);
        let value = self.expr(expr, None)?;
        if self.diverged {
            self.pending_temps = outer;
            return Ok(Value::never());
        }
        let result = self.call("__bool__", vec![value], None, &expr.span)?;
        self.same("Bool", &result.ty, &expr.span)?;
        self.pending_temps.retain(|name| name != &result.id);
        self.release_temps();
        self.pending_temps = outer;
        self.pending_temps.push(result.id.clone());
        Ok(result)
    }
    fn intersect_initialization(&mut self, other: &[BTreeMap<String, Variable>]) {
        for (scope, other_scope) in self.scopes.iter_mut().zip(other) {
            for (name, variable) in scope {
                variable.initialized &= other_scope.get(name).map_or_else(
                    || {
                        self.capture_contexts
                            .last()
                            .and_then(|c| c.locals.get(name))
                            .is_some_and(|v| v.id == variable.id)
                    },
                    |v| v.initialized,
                );
            }
        }
    }
    fn expr(&mut self, e: &Expr, expected: Option<&str>) -> Result<Value, String> {
        if self.diverged {
            return Ok(Value::never());
        }
        let result = self.expr_inner(e, expected);
        match result {
            Err(error) if error == crate::hir::DIVERGED => Ok(Value::never()),
            Err(error) => Err(error),
            Ok(_) if self.diverged => Ok(Value::never()),
            Ok(value) => Ok(value),
        }
    }
    fn expr_inner(&mut self, e: &Expr, expected: Option<&str>) -> Result<Value, String> {
        match &e.kind {
            ExprKind::Block(statements, tail) => {
                self.block_expression(statements, tail.as_deref(), expected, &e.span)
            }
            ExprKind::Match(value, arms) => self.match_expression(value, arms, expected, &e.span),
            ExprKind::If(a, b, c) => self.if_expression(a, b, c, expected, &e.span),
            ExprKind::Coalesce(a, b) => self.coalesce(a, b, expected, &e.span),
            ExprKind::Unwrap(value) => {
                let value = self.expr(value, None)?;
                if self.diverged {
                    return Ok(Value::never());
                }
                if self.option_element(&value.ty).is_none() {
                    return Err(e.span.error("?! requires Option<T>"));
                }
                self.call("value", vec![value], None, &e.span)
            }
            ExprKind::CompareChain(first, rest) => self.comparison_chain(first, rest, &e.span),
            ExprKind::Lambda(parameters, body) => self.lambda(parameters, body, &e.span),
            ExprKind::Invoke(callee, args) => self.invoke_expression(callee, args, &e.span),
            ExprKind::Tuple(items) => self.tuple_value(items, expected, &e.span),
            ExprKind::Record(name, fields) => self.record_value(name, fields),
            ExprKind::Field(receiver, field) => {
                let receiver = self.expr(receiver, None)?;
                if self.callables.contains_key(&receiver.ty) {
                    return Err(field.span.error("callable environment fields are private"));
                }
                let (index, ty) = self.field(&receiver.ty, field)?;
                Ok(self.emit(
                    &ty,
                    Rvalue::Field {
                        owner: receiver,
                        index,
                    },
                ))
            }
            ExprKind::Literal(ty, text) => self.literal(ty, text, &e.span),
            ExprKind::Array(items) => {
                let hint = expected.and_then(vector_element);
                let mut values = vec![];
                for item in items {
                    values.push(self.expr(item, hint)?);
                }
                let element = values
                    .first()
                    .map(|v| v.ty.as_str())
                    .or(hint)
                    .ok_or_else(|| e.span.error("empty array requires a vector type context"))?
                    .to_string();
                for value in &values {
                    self.same(&element, &value.ty, &e.span)?;
                }
                let ty = format!("Vec<{element}>");
                self.check_type(&Name {
                    text: ty.clone(),
                    span: e.span.clone(),
                })?;
                let array = self.call("__vec_new__", vec![], Some(&ty), &e.span)?;
                for value in values {
                    self.call("push_back", vec![array.clone(), value], None, &e.span)?;
                }
                Ok(array)
            }
            ExprKind::Repeat(initial, count) => {
                let initial = self.expr(initial, expected.and_then(vector_element))?;
                let ty = format!("Vec<{}>", initial.ty);
                self.check_type(&Name {
                    text: ty.clone(),
                    span: e.span.clone(),
                })?;
                let count = self.expr(count, Some("Int"))?;
                self.call("__vec_repeat__", vec![initial, count], Some(&ty), &e.span)
            }
            ExprKind::Index(array, index) => {
                let array = self.expr(array, None)?;
                if array.ty.starts_with("Tuple<") {
                    let index_value = tuples::literal_index(index)?;
                    return self.tuple_item(&array, index_value, &index.span);
                }
                let key = self.dictionary_types(&array.ty).map(|(k, _)| k);
                let index = self.expr(index, Some(key.as_deref().unwrap_or("Int")))?;
                self.call("__getitem__", vec![array, index], None, &e.span)
            }
            ExprKind::Range(start, end) => {
                let start = self.expr(start, Some("Int"))?;
                let end = self.expr(end, Some("Int"))?;
                self.call("__range__", vec![start, end], None, &e.span)
            }
            ExprKind::Logical(op, left, right) => {
                let left = self.condition(left)?;
                if self.diverged {
                    return Ok(Value::never());
                }
                let result = self.emit("Bool", Rvalue::BoolCopy(left.clone()));
                let before = self.scopes.clone();
                let outer = std::mem::take(&mut self.pending_temps);
                let branch = self.capture(|c| {
                    let right = c.condition(right)?;
                    c.body.push(Instruction::StoreBool {
                        target: result.clone(),
                        value: right,
                    });
                    c.release_temps();
                    Ok(())
                })?;
                self.diverged = false;
                self.pending_temps = outer;
                self.scopes = before;
                let (yes, no) = if op == "||" {
                    (Body::default(), branch)
                } else {
                    (branch, Body::default())
                };
                self.body.push(Instruction::If {
                    condition: left,
                    yes,
                    no,
                });
                Ok(result)
            }
            ExprKind::Var(n) => {
                let variable = self.lookup_variable(n, false)?;
                if (n.text == "None" || n.text.contains("::")) && variable.is_none() {
                    return self.call(&n.text, vec![], expected, &n.span);
                }
                let Some(v) = variable else {
                    return self.function_value(n);
                };
                if v.ty == crate::hir::DIVERGED {
                    self.diverged = true;
                    return Ok(Value::never());
                }
                if !v.initialized {
                    return Err(n.span.error("variable used before initialization"));
                }
                Ok(self.emit(
                    &v.ty,
                    Rvalue::Share(Value {
                        id: v.id,
                        ty: v.ty.clone(),
                    }),
                ))
            }
            ExprKind::Call(n, args) => {
                if self
                    .lookup_variable(n, true)?
                    .is_some_and(|v| self.callables.contains_key(&v.ty))
                {
                    let callee = self.expr(
                        &Expr {
                            kind: ExprKind::Var(n.clone()),
                            span: n.span.clone(),
                        },
                        None,
                    )?;
                    return self.invoke_arguments(callee, args, &e.span);
                }
                let mut values = vec![];
                for arg in args {
                    // Use a context only when every remaining overload agrees.
                    let hints = self.argument_hints(&n.text, args.len(), &values);
                    let builtin_hint =
                        self.enum_argument_hint(&n.text, &values, expected)
                            .or_else(|| {
                                self.container_argument_hint(&n.text, args.len(), &values, expected)
                            });
                    let hint = if hints.len() == 1 {
                        hints.first().map(String::as_str)
                    } else {
                        builtin_hint.as_deref()
                    };
                    values.push(self.expr(arg, hint)?);
                }
                self.call(&n.text, values, expected, &e.span)
            }
            ExprKind::Binary(op, a, b) => {
                let a = self.expr(a, None)?;
                let b = self.expr(b, None)?;
                self.call(binary_name(op), vec![a, b], None, &e.span)
            }
            ExprKind::Unary(op, x) => {
                // The minimum Int is spelled as a negated positive magnitude.
                if op == "-"
                    && let ExprKind::Literal(ty, text) = &x.kind
                    && ty == "Int"
                    && text == "9223372036854775808"
                {
                    return self.literal("Int", "-9223372036854775808", &e.span);
                }
                let x = self.expr(x, None)?;
                self.call(
                    match op.as_str() {
                        "-" => "__neg__",
                        "!" => "__not__",
                        _ => "__pos__",
                    },
                    vec![x],
                    None,
                    &e.span,
                )
            }
            ExprKind::Assign(op, left, right) => {
                if matches!(left.kind, ExprKind::Tuple(_)) {
                    if op != "=" && op != ":=" {
                        return Err(e.span.error("tuple assignment only supports = and :="));
                    }
                    let mut value = self.expr(right, None)?;
                    if op == "=" {
                        value = self.call("copy", vec![value], None, &right.span)?;
                    }
                    return self.assign_tuple_value(op, left, value);
                }
                if op == ":=" {
                    if let ExprKind::Field(receiver, field) = &left.kind {
                        let receiver = self.expr(receiver, None)?;
                        let (index, ty) = self.field(&receiver.ty, field)?;
                        let rhs = self.expr(right, Some(&ty))?;
                        self.same(&ty, &rhs.ty, &right.span)?;
                        self.body.push(Instruction::Rebind {
                            span: Some(e.span.clone()),
                            target: Place::Field {
                                owner: receiver,
                                index,
                            },
                            value: rhs.clone(),
                        });
                        return Ok(self.emit(&ty, Rvalue::Share(rhs)));
                    }
                    if let ExprKind::Index(array, index) = &left.kind {
                        let array = self.expr(array, None)?;
                        if array.ty.starts_with("Tuple<") {
                            let position = tuples::literal_index(index)?;
                            let item = self.tuple_item(&array, position, &index.span)?;
                            let rhs = self.expr(right, Some(&item.ty))?;
                            self.same(&item.ty, &rhs.ty, &right.span)?;
                            self.body.push(Instruction::Rebind {
                                span: Some(e.span.clone()),
                                target: Place::Field {
                                    owner: array,
                                    index: position,
                                },
                                value: rhs.clone(),
                            });
                            return Ok(self.emit(&item.ty, Rvalue::Share(rhs)));
                        }
                        let dict = self.dictionary_types(&array.ty);
                        let index = self.expr(
                            index,
                            Some(dict.as_ref().map_or("Int", |(k, _)| k.as_str())),
                        )?;
                        let element = dict
                            .map(|(_, v)| v)
                            .or_else(|| vector_element(&array.ty).map(str::to_string));
                        let rhs = self.expr(right, element.as_deref())?;
                        return self.call("__setitem__", vec![array, index, rhs], None, &e.span);
                    }
                    let ExprKind::Var(n) = &left.kind else {
                        return Err(e
                            .span
                            .error("rebinding requires a variable, field or index access"));
                    };
                    let v = self.find_var(n)?;
                    let r = self.expr(right, Some(&v.ty))?;
                    if self.diverged {
                        return Ok(Value::never());
                    }
                    self.same(&v.ty, &r.ty, &e.span)?;
                    self.check_fixed_rebind(&v, &e.span)?;
                    self.body.push(Instruction::Rebind {
                        span: Some(e.span.clone()),
                        target: Place::Local(v.id.clone()),
                        value: r,
                    });
                    for scope in self.scopes.iter_mut().rev() {
                        if let Some(v) = scope.get_mut(&n.text) {
                            v.initialized = true;
                            break;
                        }
                    }
                    return Ok(self.emit(
                        &v.ty,
                        Rvalue::Share(Value {
                            id: v.id,
                            ty: v.ty.clone(),
                        }),
                    ));
                }
                let target = self.expr(left, None)?;
                let rhs = self.expr(right, Some(&target.ty))?;
                if op == "=" {
                    self.call("__assign__", vec![target.clone(), rhs], None, &e.span)?;
                } else {
                    let binary = op.strip_suffix('=').expect("compound assignment operator");
                    let inplace = format!("__i{}", &binary_name(binary)[2..]);
                    if self
                        .resolve(&inplace, &[target.clone(), rhs.clone()], None, &e.span)?
                        .is_some()
                    {
                        self.call(&inplace, vec![target.clone(), rhs], None, &e.span)?;
                    } else {
                        let result = self.call(
                            binary_name(binary),
                            vec![target.clone(), rhs],
                            None,
                            &e.span,
                        )?;
                        self.call("__assign__", vec![target.clone(), result], None, &e.span)?;
                    }
                }
                Ok(target)
            }
        }
    }
}
fn binary_name(op: &str) -> &str {
    match op {
        "<?" => "__min__",
        ">?" => "__max__",
        "&" => "__bitand__",
        "|" => "__bitor__",
        "^" => "__bitxor__",
        "+" => "__add__",
        "-" => "__sub__",
        "*" => "__mul__",
        "/" => "__div__",
        "%" => "__mod__",
        "==" => "__eq__",
        "!=" => "__ne__",
        "<" => "__lt__",
        "<=" => "__le__",
        ">" => "__gt__",
        ">=" => "__ge__",
        _ => unreachable!(),
    }
}

fn vector_element(ty: &str) -> Option<&str> {
    ty.strip_prefix("Vec<")?.strip_suffix('>')
}
fn return_selected(name: &str) -> bool {
    matches!(name, "__read__" | "__vec_new__" | "__vec_repeat__")
}
