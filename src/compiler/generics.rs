use super::tuples::type_parts;
use super::*;

#[derive(Clone)]
pub(super) struct GenericRecord {
    pub name: Name,
    pub parameters: Vec<Name>,
    pub fields: Vec<(Name, Name)>,
}
#[derive(Clone)]
pub(super) struct GenericFunction {
    pub name: Name,
    pub parameters: Vec<Name>,
    pub arguments: Vec<(DeclaredName, Name)>,
    pub result: Option<Name>,
    pub body: Box<Stmt>,
}

pub(super) fn substitute(text: &str, bindings: &BTreeMap<String, String>) -> String {
    let mut result = String::new();
    let mut word = String::new();
    for c in text.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            result.push_str(bindings.get(&word).unwrap_or(&word));
            word.clear();
            if c != '\0' {
                result.push(c);
            }
        }
    }
    result
}
pub(super) fn unify(
    pattern: &str,
    actual: &str,
    parameters: &[Name],
    bindings: &mut BTreeMap<String, String>,
) -> bool {
    if parameters.iter().any(|p| p.text == pattern) {
        if let Some(known) = bindings.get(pattern) {
            return known == actual;
        }
        bindings.insert(pattern.into(), actual.into());
        return true;
    }
    match (type_parts(pattern), type_parts(actual)) {
        (Some((a, pa)), Some((b, pb))) if a == b && pa.len() == pb.len() => pa
            .iter()
            .zip(pb)
            .all(|(p, a)| unify(p, a, parameters, bindings)),
        _ => pattern == actual,
    }
}
fn parameters_valid(parameters: &[Name]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for parameter in parameters {
        if !names.insert(&parameter.text) {
            return Err(parameter.span.error("duplicate type parameter"));
        }
    }
    Ok(())
}
fn explicit_bindings(name: &str, parameters: &[Name]) -> Option<BTreeMap<String, String>> {
    if let Some((_, arguments)) = type_parts(name) {
        if arguments.len() != parameters.len() {
            return None;
        }
        Some(
            parameters
                .iter()
                .zip(arguments)
                .map(|(p, a)| (p.text.clone(), a.into()))
                .collect(),
        )
    } else {
        Some(BTreeMap::new())
    }
}

impl Compiler {
    pub(super) fn register_generic_record(
        &mut self,
        definition: GenericRecord,
    ) -> Result<(), String> {
        parameters_valid(&definition.parameters)?;
        if self.types.contains_key(&definition.name.text)
            || self.generic_records.contains_key(&definition.name.text)
        {
            return Err(definition.name.span.error("duplicate type"));
        }
        let mut fields = BTreeSet::new();
        for (name, _) in &definition.fields {
            if !fields.insert(&name.text) {
                return Err(name.span.error("duplicate field"));
            }
        }
        self.generic_records
            .insert(definition.name.text.clone(), definition);
        Ok(())
    }
    pub(super) fn register_generic_function(
        &mut self,
        definition: GenericFunction,
    ) -> Result<(), String> {
        parameters_valid(&definition.parameters)?;
        let mut arguments = BTreeSet::new();
        for (name, _) in &definition.arguments {
            if !arguments.insert(&name.text) {
                return Err(name.span.error("duplicate parameter"));
            }
        }
        let normalized = |f: &GenericFunction| {
            let bindings = f
                .parameters
                .iter()
                .enumerate()
                .map(|(i, p)| (p.text.clone(), format!("${i}")))
                .collect();
            f.arguments
                .iter()
                .map(|(_, ty)| substitute(&ty.text, &bindings))
                .collect::<Vec<_>>()
        };
        if self.generic_functions.iter().any(|f| {
            f.name.text == definition.name.text
                && f.parameters.len() == definition.parameters.len()
                && normalized(f) == normalized(&definition)
        }) {
            return Err(definition.name.span.error("duplicate generic function"));
        }
        self.generic_functions.push(definition);
        Ok(())
    }
    pub(super) fn specialize_record(&mut self, name: &Name) -> Result<bool, String> {
        let Some((base, arguments)) = type_parts(&name.text) else {
            return Ok(false);
        };
        let Some(template) = self.generic_records.get(base).cloned() else {
            return Ok(false);
        };
        if arguments.len() != template.parameters.len() {
            return Err(name.span.error(format!(
                "type '{base}' expects {} type arguments",
                template.parameters.len()
            )));
        }
        if self.records.len() >= 256 {
            return Err(name.span.error("struct specialization limit exceeded"));
        }
        let bindings: BTreeMap<String, String> = template
            .parameters
            .iter()
            .zip(arguments)
            .map(|(p, a)| (p.text.clone(), a.into()))
            .collect();
        for ty in bindings.values() {
            self.check_type(&Name {
                text: ty.clone(),
                span: name.span.clone(),
            })?;
        }
        let fields = template
            .fields
            .iter()
            .map(|(field, ty)| {
                (
                    field.clone(),
                    Name {
                        text: substitute(&ty.text, &bindings),
                        span: ty.span.clone(),
                    },
                )
            })
            .collect();
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
        self.prepare_record(&name.text).map_err(|error| {
            format!(
                "{error}\nwhile specializing '{}' at {}:{}:{}",
                name.text, name.span.file, name.span.line, name.span.column
            )
        })?;
        Ok(true)
    }
    pub(super) fn argument_hints(
        &self,
        name: &str,
        count: usize,
        known: &[Value],
    ) -> BTreeSet<String> {
        let base = type_parts(name).map_or(name, |(base, _)| base);
        let mut hints = BTreeSet::new();
        if base == name {
            for (index, f) in self.functions.iter().enumerate() {
                if self.generic_instances.values().any(|&i| i == index) {
                    continue;
                }
                if f.name.text == name
                    && f.params.len() == count
                    && f.params
                        .iter()
                        .zip(known)
                        .all(|(ty, value)| ty == &value.ty)
                {
                    hints.insert(f.params[known.len()].clone());
                }
            }
        }
        for f in &self.generic_functions {
            if f.name.text != base || f.arguments.len() != count {
                continue;
            }
            let Some(mut bindings) = explicit_bindings(name, &f.parameters) else {
                continue;
            };
            if !f
                .arguments
                .iter()
                .zip(known)
                .all(|((_, ty), value)| unify(&ty.text, &value.ty, &f.parameters, &mut bindings))
            {
                continue;
            }
            let ty = &f.arguments[known.len()].1.text;
            let unbound = f.parameters.iter().any(|p| {
                !bindings.contains_key(&p.text)
                    && ty
                        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .any(|s| s == p.text)
            });
            if !unbound {
                hints.insert(substitute(ty, &bindings));
            }
        }
        hints
    }
    pub(super) fn resolve(
        &mut self,
        name: &str,
        args: &[Value],
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Option<usize>, String> {
        let base = type_parts(name).map_or(name, |(base, _)| base);
        if base == name {
            if let Some(index) = self.select(name, args, expected, span)? {
                return Ok(Some(index));
            }
        } else if let Some((_, arguments)) = type_parts(name) {
            for ty in arguments {
                self.check_type(&Name {
                    text: ty.into(),
                    span: span.clone(),
                })?;
            }
        }
        let mut candidates = vec![];
        for (index, f) in self.generic_functions.iter().enumerate() {
            if f.name.text != base || f.arguments.len() != args.len() {
                continue;
            }
            let Some(mut bindings) = explicit_bindings(name, &f.parameters) else {
                continue;
            };
            if f.arguments
                .iter()
                .zip(args)
                .all(|((_, ty), value)| unify(&ty.text, &value.ty, &f.parameters, &mut bindings))
                && f.parameters.iter().all(|p| bindings.contains_key(&p.text))
            {
                candidates.push((index, bindings));
            }
        }
        if candidates.len() > 1 {
            return Err(span.error(format!("ambiguous generic call to '{name}'")));
        }
        let Some((template, bindings)) = candidates.pop() else {
            return Ok(None);
        };
        let definition = self.generic_functions[template].clone();
        let type_arguments = definition
            .parameters
            .iter()
            .map(|p| bindings[&p.text].clone())
            .collect::<Vec<_>>();
        let key = (template, type_arguments);
        if let Some(index) = self.generic_instances.get(&key) {
            return Ok(Some(*index));
        }
        if self.generic_instances.len() >= 256 {
            return Err(span.error("generic function specialization limit exceeded"));
        }
        let result = definition
            .result
            .as_ref()
            .map_or("Unit".into(), |ty| substitute(&ty.text, &bindings));
        self.check_type(&Name {
            text: result.clone(),
            span: span.clone(),
        })?;
        let mut body = *definition.body.clone();
        substitute_statement(&mut body, &bindings);
        let index = self.functions.len();
        self.functions.push(Function {
            name: definition.name.clone(),
            params: args.iter().map(|v| v.ty.clone()).collect(),
            result,
            cpp: format!("rusk_generic_{index}"),
            code: String::new(),
            body: Some((
                definition
                    .arguments
                    .iter()
                    .map(|(n, _)| n.clone())
                    .collect(),
                Box::new(body),
            )),
            checked: None,
        });
        self.generic_instances.insert(key, index);
        self.generic_calls.insert(index, span.clone());
        Ok(Some(index))
    }
}

fn substitute_name(name: &mut Name, bindings: &BTreeMap<String, String>) {
    name.text = substitute(&name.text, bindings);
}
fn substitute_reference(name: &mut Name, bindings: &BTreeMap<String, String>) {
    if let Some((owner, variant)) = name.text.rsplit_once("::") {
        name.text = format!("{}::{variant}", substitute(owner, bindings));
    } else {
        substitute_name(name, bindings);
    }
}
fn substitute_expr(expr: &mut Expr, bindings: &BTreeMap<String, String>) {
    match &mut expr.kind {
        ExprKind::Block(items, tail) => {
            for item in items {
                substitute_statement(item, bindings);
            }
            if let Some(tail) = tail {
                substitute_expr(tail, bindings);
            }
        }
        ExprKind::Match(value, arms) => {
            substitute_expr(value, bindings);
            for arm in arms {
                substitute_pattern(&mut arm.pattern, bindings);
                if let Some(guard) = &mut arm.guard {
                    substitute_expr(guard, bindings);
                }
                substitute_expr(&mut arm.body, bindings);
            }
        }
        ExprKind::If(a, b, c) => {
            substitute_condition(a, bindings);
            substitute_expr(b, bindings);
            substitute_expr(c, bindings);
        }
        ExprKind::CompareChain(first, rest) => {
            substitute_expr(first, bindings);
            for (_, value, _) in rest {
                substitute_expr(value, bindings);
            }
        }
        ExprKind::Unwrap(value) => substitute_expr(value, bindings),
        ExprKind::Coalesce(a, b) => {
            substitute_expr(a, bindings);
            substitute_expr(b, bindings);
        }
        ExprKind::Lambda(parameters, body) => {
            for (_, ty) in parameters {
                substitute_name(ty, bindings);
            }
            substitute_expr(body, bindings);
        }
        ExprKind::Invoke(callee, args) => {
            substitute_expr(callee, bindings);
            for arg in args {
                substitute_expr(arg, bindings);
            }
        }
        ExprKind::Array(items) | ExprKind::Tuple(items) => {
            for item in items {
                substitute_expr(item, bindings);
            }
        }
        ExprKind::Repeat(a, b)
        | ExprKind::Range(a, b)
        | ExprKind::Index(a, b)
        | ExprKind::Logical(_, a, b)
        | ExprKind::Binary(_, a, b)
        | ExprKind::Assign(_, a, b) => {
            substitute_expr(a, bindings);
            substitute_expr(b, bindings);
        }
        ExprKind::Field(value, _) | ExprKind::Unary(_, value) => substitute_expr(value, bindings),
        ExprKind::Record(name, fields) => {
            substitute_name(name, bindings);
            for (_, value) in fields {
                substitute_expr(value, bindings);
            }
        }
        ExprKind::Call(name, args) => {
            substitute_reference(name, bindings);
            for arg in args {
                substitute_expr(arg, bindings);
            }
        }
        ExprKind::Var(n) if n.text.contains("::") => substitute_reference(n, bindings),
        ExprKind::Literal(..) | ExprKind::Var(_) => {}
    }
}
fn substitute_statement(stmt: &mut Stmt, bindings: &BTreeMap<String, String>) {
    match stmt {
        Stmt::Let(_, ty, initializer) => {
            if let Some(ty) = ty {
                substitute_name(ty, bindings);
            }
            if let Some(Initializer::Shared(expr) | Initializer::Copied(expr)) = initializer {
                substitute_expr(expr, bindings);
            }
        }
        Stmt::Return(_, Some(value)) => substitute_expr(value, bindings),
        Stmt::Input(items) => {
            for (_, ty, args) in items {
                substitute_name(ty, bindings);
                for arg in args {
                    substitute_expr(arg, bindings);
                }
            }
        }
        Stmt::Output(items) => {
            for expr in items {
                substitute_expr(expr, bindings);
            }
        }
        Stmt::Expr(expr) | Stmt::Match(expr) => substitute_expr(expr, bindings),
        Stmt::Block(items, tail) => {
            for item in items {
                substitute_statement(item, bindings);
            }
            if let Some(tail) = tail {
                substitute_expr(tail, bindings);
            }
        }
        Stmt::Labeled(_, body) => substitute_statement(body, bindings),
        Stmt::If(condition, body, other) | Stmt::While(condition, body, other) => {
            substitute_condition(condition, bindings);
            substitute_statement(body, bindings);
            if let Some(other) = other {
                substitute_statement(other, bindings);
            }
        }
        Stmt::For(_, source, body, other) => {
            substitute_expr(source, bindings);
            substitute_statement(body, bindings);
            if let Some(other) = other {
                substitute_statement(other, bindings);
            }
        }
        Stmt::Loop(body) => substitute_statement(body, bindings),
        _ => {}
    }
}

fn substitute_pattern(p: &mut Pattern, bindings: &BTreeMap<String, String>) {
    match &mut p.kind {
        PatternKind::Named(n, children) => {
            if n.text.contains("::") {
                substitute_reference(n, bindings);
            }
            if let Some(children) = children {
                for p in children {
                    substitute_pattern(p, bindings);
                }
            }
        }
        PatternKind::Tuple(items) | PatternKind::Or(items) => {
            for p in items {
                substitute_pattern(p, bindings);
            }
        }
        PatternKind::Record(n, fields, _) => {
            substitute_name(n, bindings);
            for (_, p) in fields {
                substitute_pattern(p, bindings);
            }
        }
        PatternKind::Literal(..) => {}
    }
}
fn substitute_condition(c: &mut Condition, bindings: &BTreeMap<String, String>) {
    match c {
        Condition::Expr(e) => substitute_expr(e, bindings),
        Condition::Let(p, e) => {
            substitute_pattern(p, bindings);
            substitute_expr(e, bindings);
        }
    }
}
