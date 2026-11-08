use super::*;
mod coverage;
mod lower;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Atom {
    Int(i64),
    Bool(bool),
    Char(u8),
    Str(String),
}
impl Atom {
    fn parse(ty: &str, text: &str) -> Self {
        match ty {
            "Int" => Self::Int(text.parse().expect("validated integer")),
            "Bool" => Self::Bool(text == "true"),
            "Str" => Self::Str(serde_json::from_str(text).expect("validated string")),
            "Char" => {
                let inner = &text[1..text.len() - 1];
                Self::Char(if let Some(escape) = inner.strip_prefix('\\') {
                    match escape {
                        "n" => b'\n',
                        "r" => b'\r',
                        "t" => b'\t',
                        "0" => 0,
                        "\\" => b'\\',
                        "'" => b'\'',
                        "\"" => b'"',
                        _ => unreachable!(),
                    }
                } else {
                    inner.as_bytes()[0]
                })
            }
            _ => unreachable!(),
        }
    }
    fn text(&self) -> String {
        match self {
            Self::Int(n) => n.to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Str(s) => serde_json::to_string(s).unwrap(),
            Self::Char(c) => match c {
                0 => "'\\0'".into(),
                b'\n' => "'\\n'".into(),
                b'\r' => "'\\r'".into(),
                b'\t' => "'\\t'".into(),
                b'\'' => "'\\\''".into(),
                b'\\' => "'\\\\'".into(),
                32..=126 => format!("'{}'", *c as char),
                _ => format!("Char({c})"),
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Constructor {
    Variant(usize),
    Product,
    Literal(Atom),
}
#[derive(Clone)]
struct TypedPattern {
    ty: String,
    kind: Kind,
    span: Span,
    literal: Option<Value>,
}
#[derive(Clone)]
enum Kind {
    Wild(Option<Name>),
    Constructor(Constructor, Vec<TypedPattern>),
    Or(Vec<TypedPattern>),
}
impl TypedPattern {
    fn wild(ty: &str, span: &Span) -> Self {
        Self {
            ty: ty.into(),
            kind: Kind::Wild(None),
            span: span.clone(),
            literal: None,
        }
    }
    fn bindings(&self) -> Result<BTreeMap<String, (Name, String)>, String> {
        let mut out = BTreeMap::new();
        match &self.kind {
            Kind::Wild(Some(n)) => {
                out.insert(n.text.clone(), (n.clone(), self.ty.clone()));
            }
            Kind::Wild(None) => {}
            Kind::Constructor(_, fields) => {
                for field in fields {
                    for (name, binding) in field.bindings()? {
                        if out.insert(name, binding.clone()).is_some() {
                            return Err(binding.0.span.error("duplicate variable in pattern"));
                        }
                    }
                }
            }
            Kind::Or(alternatives) => {
                out = alternatives[0].bindings()?;
                for alternative in &alternatives[1..] {
                    let other = alternative.bindings()?;
                    if out
                        .iter()
                        .map(|(n, (_, t))| (n, t))
                        .ne(other.iter().map(|(n, (_, t))| (n, t)))
                    {
                        return Err(alternative.span.error(
                            "or-pattern alternatives must bind the same names with the same types",
                        ));
                    }
                }
            }
        }
        Ok(out)
    }
}
impl Compiler {
    fn resolve_pattern(&mut self, pattern: &Pattern, ty: &str) -> Result<TypedPattern, String> {
        let mut result = TypedPattern::wild(ty, &pattern.span);
        result.kind = match &pattern.kind {
            PatternKind::Literal(wanted, text) => {
                self.same(wanted, ty, &pattern.span)?;
                result.literal = Some(self.literal(wanted, text, &pattern.span)?);
                Kind::Constructor(Constructor::Literal(Atom::parse(wanted, text)), vec![])
            }
            PatternKind::Or(items) => {
                let alternatives = items
                    .iter()
                    .map(|p| self.resolve_pattern(p, ty))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut rows = vec![];
                for p in &alternatives {
                    if self
                        .pattern_witness(&rows, std::slice::from_ref(p))?
                        .is_none()
                    {
                        return Err(p.span.error("unreachable or-pattern alternative"));
                    }
                    rows.push(vec![p.clone()]);
                }
                Kind::Or(alternatives)
            }
            PatternKind::Named(name, args) => {
                if name.text == "_" {
                    if args.is_some() {
                        return Err(name.span.error("wildcard has no payload"));
                    }
                    Kind::Wild(None)
                } else {
                    let e = self.enums.get(ty).cloned();
                    let (owner, variant) = name
                        .text
                        .rsplit_once("::")
                        .map_or((None, name.text.as_str()), |(o, v)| (Some(o), v));
                    let index = e
                        .as_ref()
                        .and_then(|e| e.variants.iter().position(|(n, _)| n.text == variant));
                    if let (Some(e), Some(index)) = (e, index) {
                        if owner.is_some_and(|o| o != ty && o != e.definition) {
                            return Err(name.span.error("pattern belongs to another enum type"));
                        }
                        let args = args.as_deref().unwrap_or_default();
                        let fields = &e.variants[index].1;
                        if args.len() != fields.len() {
                            return Err(name.span.error("enum pattern payload count mismatch"));
                        }
                        Kind::Constructor(
                            Constructor::Variant(index),
                            args.iter()
                                .zip(fields)
                                .map(|(p, t)| self.resolve_pattern(p, &t.text))
                                .collect::<Result<Vec<_>, _>>()?,
                        )
                    } else if owner.is_some() || args.is_some() {
                        return Err(name.span.error("unknown enum variant in pattern"));
                    } else {
                        Kind::Wild(Some(name.clone()))
                    }
                }
            }
            PatternKind::Tuple(items) => {
                let types = if ty == "Unit" {
                    vec![]
                } else if ty.starts_with("Tuple<") {
                    self.records[ty]
                        .fields
                        .iter()
                        .map(|(_, t)| t.text.clone())
                        .collect()
                } else {
                    return Err(pattern
                        .span
                        .error(format!("expected a tuple, found '{ty}'")));
                };
                if items.len() != types.len() {
                    return Err(pattern.span.error("tuple pattern arity mismatch"));
                }
                Kind::Constructor(
                    Constructor::Product,
                    items
                        .iter()
                        .zip(types)
                        .map(|(p, t)| self.resolve_pattern(p, &t))
                        .collect::<Result<Vec<_>, _>>()?,
                )
            }
            PatternKind::Record(name, items, rest) => {
                if self.callables.contains_key(ty) {
                    return Err(name.span.error("callable environment fields are private"));
                }
                let record = self
                    .records
                    .get(ty)
                    .cloned()
                    .ok_or_else(|| name.span.error("struct pattern requires a struct value"))?;
                let base = tuples::type_parts(ty).map_or(ty, |(b, _)| b);
                if name.text != ty && name.text != base {
                    return Err(name.span.error("struct pattern type mismatch"));
                }
                let mut fields = record
                    .fields
                    .iter()
                    .map(|(_, t)| TypedPattern::wild(&t.text, &pattern.span))
                    .collect::<Vec<_>>();
                let mut seen = BTreeSet::new();
                for (field, p) in items {
                    let (index, t) = self.field(ty, field)?;
                    if !seen.insert(index) {
                        return Err(field.span.error("duplicate struct pattern field"));
                    }
                    fields[index] = self.resolve_pattern(p, &t)?;
                }
                if !rest && seen.len() != fields.len() {
                    return Err(name.span.error(
                        "missing struct pattern fields; use .. to ignore the remaining fields",
                    ));
                }
                Kind::Constructor(Constructor::Product, fields)
            }
        };
        result.bindings()?;
        Ok(result)
    }
    fn constructor_fields(&self, ty: &str, constructor: &Constructor) -> Vec<String> {
        match constructor {
            Constructor::Variant(index) => self.enums[ty].variants[*index]
                .1
                .iter()
                .map(|t| t.text.clone())
                .collect(),
            Constructor::Product => self
                .records
                .get(ty)
                .map(|r| r.fields.iter().map(|(_, t)| t.text.clone()).collect())
                .unwrap_or_default(),
            Constructor::Literal(_) => vec![],
        }
    }
    fn constructors(&self, ty: &str) -> Option<Vec<Constructor>> {
        if let Some(e) = self.enums.get(ty) {
            Some((0..e.variants.len()).map(Constructor::Variant).collect())
        } else if ty == "Bool" {
            Some(vec![
                Constructor::Literal(Atom::Bool(false)),
                Constructor::Literal(Atom::Bool(true)),
            ])
        } else if ty == "Char" {
            Some(
                (0..=127)
                    .map(|c| Constructor::Literal(Atom::Char(c)))
                    .collect(),
            )
        } else if ty == "Unit" || self.records.contains_key(ty) {
            Some(vec![Constructor::Product])
        } else {
            None
        }
    }
    fn format_constructor(&self, ty: &str, constructor: &Constructor, fields: &[String]) -> String {
        match constructor {
            Constructor::Variant(i) => {
                let name = &self.enums[ty].variants[*i].0.text;
                if fields.is_empty() {
                    name.clone()
                } else {
                    format!("{name}({})", fields.join(", "))
                }
            }
            Constructor::Literal(atom) => atom.text(),
            Constructor::Product => {
                if ty == "Unit" || ty.starts_with("Tuple<") {
                    format!("({})", fields.join(", "))
                } else {
                    format!(
                        "{ty} {{ {} }}",
                        self.records[ty]
                            .fields
                            .iter()
                            .zip(fields)
                            .map(|((n, _), v)| format!("{}: {v}", n.text))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
        }
    }
}
