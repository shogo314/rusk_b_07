use crate::rusk_grammar_trait as g;
use parol_runtime::Token;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub file: String,
    pub line: u32,
    pub column: u32,
}
impl Span {
    pub fn token(t: &Token<'_>) -> Self {
        Self {
            file: t.location.file_name.display().to_string(),
            line: t.location.start_line,
            column: t.location.start_column,
        }
    }
    pub fn error(&self, message: impl std::fmt::Display) -> String {
        format!("{}:{}:{}: {}", self.file, self.line, self.column, message)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingMode {
    Fixed,
    Rebindable,
}
#[derive(Clone, Debug)]
pub struct BindingAnnotation {
    pub mode: BindingMode,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessMode {
    Readonly,
    Mutable,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AccessAnnotation {
    pub mode: AccessMode,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeMode {
    Noescape,
    Escape,
}
#[derive(Clone, Debug)]
pub struct EscapeAnnotation {
    pub mode: EscapeMode,
    pub span: Span,
}
#[derive(Default)]
struct Modifiers {
    binding: Option<BindingAnnotation>,
    access: Option<AccessAnnotation>,
    escape: Option<EscapeAnnotation>,
}
#[derive(Clone, Debug)]
pub struct DeclaredName {
    pub name: Name,
    pub binding: Option<BindingAnnotation>,
    pub access: Option<AccessAnnotation>,
    pub escape: Option<EscapeAnnotation>,
}
impl DeclaredName {
    pub fn plain(name: Name) -> Self {
        Self {
            name,
            binding: None,
            access: None,
            escape: None,
        }
    }
}
impl std::ops::Deref for DeclaredName {
    type Target = Name;
    fn deref(&self) -> &Name {
        &self.name
    }
}
fn annotations(modifiers: &g::BindingModifiers<'_>) -> Result<Modifiers, String> {
    let mut binding: Option<BindingAnnotation> = None;
    let mut access: Option<AccessAnnotation> = None;
    let mut escape: Option<EscapeAnnotation> = None;
    for item in &modifiers.binding_modifiers_list {
        match item.binding_modifier.as_ref() {
            g::BindingModifier::Fixed(item) => {
                set_binding(&mut binding, BindingMode::Fixed, Span::token(&item.fixed))?
            }
            g::BindingModifier::Rebindable(item) => set_binding(
                &mut binding,
                BindingMode::Rebindable,
                Span::token(&item.rebindable),
            )?,
            g::BindingModifier::Readonly(item) => set_access(
                &mut access,
                AccessMode::Readonly,
                Span::token(&item.readonly),
            )?,
            g::BindingModifier::Mutable(item) => {
                set_access(&mut access, AccessMode::Mutable, Span::token(&item.mutable))?
            }
            g::BindingModifier::Noescape(item) => set_escape(
                &mut escape,
                EscapeMode::Noescape,
                Span::token(&item.noescape),
            )?,
            g::BindingModifier::Escape(item) => {
                set_escape(&mut escape, EscapeMode::Escape, Span::token(&item.escape))?
            }
        }
    }
    Ok(Modifiers {
        binding,
        access,
        escape,
    })
}
fn set_binding(
    previous: &mut Option<BindingAnnotation>,
    mode: BindingMode,
    span: Span,
) -> Result<(), String> {
    if let Some(old) = previous {
        return Err(format!(
            "{}\n{}",
            span.error(if old.mode == mode {
                "duplicate binding modifier"
            } else {
                "conflicting binding modifiers"
            }),
            old.span.error("previous binding modifier is here")
        ));
    }
    *previous = Some(BindingAnnotation { mode, span });
    Ok(())
}
fn set_access(
    previous: &mut Option<AccessAnnotation>,
    mode: AccessMode,
    span: Span,
) -> Result<(), String> {
    if let Some(old) = previous {
        return Err(format!(
            "{}\n{}",
            span.error(if old.mode == mode {
                "duplicate access modifier"
            } else {
                "conflicting access modifiers"
            }),
            old.span.error("previous access modifier is here")
        ));
    }
    *previous = Some(AccessAnnotation { mode, span });
    Ok(())
}
fn set_escape(
    previous: &mut Option<EscapeAnnotation>,
    mode: EscapeMode,
    span: Span,
) -> Result<(), String> {
    if let Some(old) = previous {
        return Err(format!(
            "{}\n{}",
            span.error(if old.mode == mode {
                "duplicate escape modifier"
            } else {
                "conflicting escape modifiers"
            }),
            old.span.error("previous escape modifier is here")
        ));
    }
    *previous = Some(EscapeAnnotation { mode, span });
    Ok(())
}
fn parameters(parameters: &g::Parameters<'_>) -> Result<Vec<(DeclaredName, Name)>, String> {
    let Some(first) = &parameters.parameters_opt else {
        return Ok(vec![]);
    };
    std::iter::once(first.parameter.as_ref())
        .chain(
            first
                .parameters_opt_list
                .iter()
                .map(|p| p.parameter.as_ref()),
        )
        .map(|p| {
            let Modifiers {
                binding,
                access,
                escape,
            } = annotations(&p.binding_modifiers)?;
            Ok((
                DeclaredName {
                    name: name(&p.identifier),
                    binding,
                    access,
                    escape,
                },
                type_name(&p.type_name),
            ))
        })
        .collect()
}
fn name(n: &g::Identifier<'_>) -> Name {
    Name {
        text: n.identifier.text().into(),
        span: Span::token(&n.identifier),
    }
}
fn type_parameters(t: &g::TypeParameters<'_>) -> Vec<Name> {
    std::iter::once(t.identifier.as_ref())
        .chain(t.type_parameters_list.iter().map(|t| t.identifier.as_ref()))
        .map(name)
        .collect()
}
fn reference(t: &g::Reference<'_>) -> Name {
    let mut n = name(&t.identifier);
    if let Some(arguments) = &t.reference_opt {
        n.text = format!(
            "{}<{}>",
            n.text,
            type_arguments(&arguments.type_arguments)
                .iter()
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    for part in &t.reference_list {
        n.text += "::";
        n.text += &name(&part.identifier).text;
    }
    n
}
fn type_arguments(t: &g::TypeArguments<'_>) -> Vec<Name> {
    std::iter::once(t.type_name.as_ref())
        .chain(t.type_arguments_list.iter().map(|t| t.type_name.as_ref()))
        .map(type_name)
        .collect()
}
fn type_name(t: &g::TypeName<'_>) -> Name {
    match t {
        g::TypeName::NamedType(t) => {
            let mut n = name(&t.named_type.identifier);
            if let Some(args) = &t.named_type.named_type_opt {
                n.text = format!(
                    "{}<{}>",
                    n.text,
                    type_arguments(&args.type_arguments)
                        .iter()
                        .map(|t| t.text.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
            n
        }
        g::TypeName::TupleType(t) => {
            let t = &t.tuple_type;
            let types = std::iter::once(t.type_name.as_ref())
                .chain(std::iter::once(t.type_name0.as_ref()))
                .chain(t.tuple_type_list.iter().map(|t| t.type_name.as_ref()))
                .map(type_name)
                .collect::<Vec<_>>();
            Name {
                text: format!(
                    "Tuple<{}>",
                    types
                        .iter()
                        .map(|t| t.text.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                span: types[0].span.clone(),
            }
        }
    }
}
#[derive(Clone, Debug)]
pub enum Binding {
    Name(DeclaredName),
    Tuple(Vec<Binding>, Span),
}
impl Binding {
    pub fn annotate(
        &mut self,
        annotation: &Option<BindingAnnotation>,
        access: &Option<AccessAnnotation>,
    ) {
        match self {
            Self::Name(name) => {
                name.binding = annotation.clone();
                name.access = access.clone();
            }
            Self::Tuple(items, _) => {
                for item in items {
                    item.annotate(annotation, access);
                }
            }
        }
    }
    pub fn span(&self) -> &Span {
        match self {
            Self::Name(n) => &n.span,
            Self::Tuple(_, span) => span,
        }
    }
}
fn binding(b: &g::Binding<'_>) -> Binding {
    match b {
        g::Binding::Identifier(n) => Binding::Name(DeclaredName::plain(name(&n.identifier))),
        g::Binding::LParenBindingCommaBindingBindingListRParen(t) => Binding::Tuple(
            std::iter::once(t.binding.as_ref())
                .chain(std::iter::once(t.binding0.as_ref()))
                .chain(t.binding_list.iter().map(|b| b.binding.as_ref()))
                .map(binding)
                .collect(),
            Span::token(&t.l_paren),
        ),
    }
}
#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub enum PatternKind {
    Named(Name, Option<Vec<Pattern>>),
    Literal(String, String),
    Tuple(Vec<Pattern>),
    Record(Name, Vec<(Name, Pattern)>, bool),
    Or(Vec<Pattern>),
}
#[derive(Clone, Debug)]
pub enum Condition {
    Expr(Expr),
    Let(Pattern, Expr),
}
impl Condition {
    pub fn span(&self) -> &Span {
        match self {
            Self::Expr(e) | Self::Let(_, e) => &e.span,
        }
    }
}
#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}
#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(String, String),
    Block(Vec<Stmt>, Option<Box<Expr>>),
    Array(Vec<Expr>),
    Tuple(Vec<Expr>),
    Repeat(Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Logical(String, Box<Expr>, Box<Expr>),
    If(Box<Condition>, Box<Expr>, Box<Expr>),
    Match(Box<Expr>, Vec<MatchArm>),
    Coalesce(Box<Expr>, Box<Expr>),
    Unwrap(Box<Expr>),
    CompareChain(Box<Expr>, Vec<(String, Expr, Span)>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, Name),
    Record(Name, Vec<(Name, Expr)>),
    Var(Name),
    Call(Name, Vec<Expr>),
    Invoke(Box<Expr>, Vec<Expr>),
    Lambda(Vec<(DeclaredName, Name)>, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Unary(String, Box<Expr>),
    Assign(String, Box<Expr>, Box<Expr>),
}
#[derive(Clone, Debug)]
pub enum Initializer {
    Shared(Expr),
    Copied(Expr),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Include(String, Span),
    Match(Expr),
    Enum(Name, Vec<Name>, Vec<(Name, Vec<Name>)>),
    Struct(Name, Vec<Name>, Vec<(Name, Name)>),
    Type(Name, BTreeMap<String, String>),
    Function(
        Name,
        Vec<(DeclaredName, Name)>,
        Option<Name>,
        BTreeMap<String, String>,
    ),
    UserFunction(
        Name,
        Vec<Name>,
        Vec<(DeclaredName, Name)>,
        Option<Name>,
        Box<Stmt>,
    ),
    Return(Span, Option<Expr>),
    Symbol(Name, BTreeMap<String, String>),
    Let(Binding, Option<Name>, Option<Initializer>),
    Input(Vec<(Name, Name, Vec<Expr>)>),
    Output(Vec<Expr>),
    Block(Vec<Stmt>, Option<Box<Expr>>),
    Labeled(Name, Box<Stmt>),
    If(Condition, Box<Stmt>, Option<Box<Stmt>>),
    While(Condition, Box<Stmt>, Option<Box<Stmt>>),
    For(Binding, Expr, Box<Stmt>, Option<Box<Stmt>>),
    Loop(Box<Stmt>),
    Break(Span, Option<Name>),
    Continue(Span, Option<Name>),
    Expr(Expr),
}
fn props(p: &g::Properties<'_>) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for entry in &p.properties_list {
        let p = &entry.property;
        let n = name(&p.identifier);
        let value = serde_json::from_str(p.string_literal.string_literal.text())
            .map_err(|e| n.span.error(e))?;
        if result.insert(n.text.clone(), value).is_some() {
            return Err(n.span.error(format!("duplicate property '{}'", n.text)));
        }
    }
    Ok(result)
}
pub fn lower(root: &g::Rusk<'_>) -> Result<Vec<Stmt>, String> {
    root.rusk_list.iter().map(|s| stmt(&s.statement)).collect()
}
fn stmt(s: &g::Statement<'_>) -> Result<Stmt, String> {
    match s {
        g::Statement::SimpleStatement(x) => simple_stmt(&x.simple_statement),
        g::Statement::BlockStatement(x) => block_statement(&x.block_statement),
        g::Statement::StatementExprSemicolon(x) => {
            Ok(Stmt::Expr(statement_expr(&x.statement_expr)?))
        }
    }
}
fn simple_stmt(s: &g::SimpleStatement<'_>) -> Result<Stmt, String> {
    Ok(match s {
        g::SimpleStatement::Include(x) => Stmt::Include(
            serde_json::from_str(x.include.string_literal.string_literal.text())
                .map_err(|e| Span::token(&x.include.string_literal.string_literal).error(e))?,
            Span::token(&x.include.string_literal.string_literal),
        ),
        g::SimpleStatement::EnumDef(x) => {
            let d = &x.enum_def;
            Stmt::Enum(
                name(&d.identifier),
                d.enum_def_opt
                    .as_ref()
                    .map(|p| type_parameters(&p.type_parameters))
                    .unwrap_or_default(),
                d.enum_def_opt0
                    .iter()
                    .flat_map(|v| {
                        std::iter::once(v.enum_variants.enum_variant.as_ref()).chain(
                            v.enum_variants
                                .enum_variants_list
                                .iter()
                                .map(|v| v.enum_variant.as_ref()),
                        )
                    })
                    .map(|v| {
                        (
                            name(&v.identifier),
                            v.enum_variant_opt
                                .as_ref()
                                .map(|f| {
                                    std::iter::once(f.variant_types.type_name.as_ref())
                                        .chain(
                                            f.variant_types
                                                .variant_types_list
                                                .iter()
                                                .map(|t| t.type_name.as_ref()),
                                        )
                                        .map(type_name)
                                        .collect()
                                })
                                .unwrap_or_default(),
                        )
                    })
                    .collect(),
            )
        }
        g::SimpleStatement::MatchStatement(x) => {
            let value = match_expr(&x.match_statement.match_expr)?;
            if x.match_statement.match_statement_opt.is_some() {
                Stmt::Expr(value)
            } else {
                Stmt::Match(value)
            }
        }
        g::SimpleStatement::StructDef(x) => Stmt::Struct(
            name(&x.struct_def.identifier),
            x.struct_def
                .struct_def_opt
                .as_ref()
                .map(|p| type_parameters(&p.type_parameters))
                .unwrap_or_default(),
            x.struct_def
                .struct_def_list
                .iter()
                .map(|field| {
                    (
                        name(&field.struct_field.identifier),
                        type_name(&field.struct_field.type_name),
                    )
                })
                .collect(),
        ),
        g::SimpleStatement::ExternType(x) => Stmt::Type(
            type_name(&x.extern_type.type_name),
            props(&x.extern_type.properties)?,
        ),
        g::SimpleStatement::ExternSymbol(x) => Stmt::Symbol(
            name(&x.extern_symbol.identifier),
            props(&x.extern_symbol.properties)?,
        ),
        g::SimpleStatement::ExternFn(x) => {
            let x = &x.extern_fn;
            let params = parameters(&x.parameters)?;
            Stmt::Function(
                name(&x.identifier),
                params,
                x.extern_fn_opt.as_ref().map(|r| type_name(&r.type_name)),
                props(&x.properties)?,
            )
        }
        g::SimpleStatement::Function(x) => {
            let x = &x.function;
            let params = parameters(&x.parameters)?;
            Stmt::UserFunction(
                name(&x.identifier),
                x.function_opt
                    .as_ref()
                    .map(|p| type_parameters(&p.type_parameters))
                    .unwrap_or_default(),
                params,
                x.function_opt0.as_ref().map(|r| type_name(&r.type_name)),
                Box::new(block(&x.block)?),
            )
        }
        g::SimpleStatement::Return(x) => Stmt::Return(
            Span::token(&x.r#return.r#return),
            x.r#return
                .return_opt
                .as_ref()
                .map(|x| expr(&x.expr))
                .transpose()?,
        ),
        g::SimpleStatement::Let(x) => {
            let x = &x.r#let;
            let mut binding = binding(&x.binding);
            let Modifiers {
                binding: mode,
                access,
                escape,
            } = annotations(&x.binding_modifiers)?;
            if let Some(annotation) = escape {
                return Err(annotation
                    .span
                    .error("escape modifiers are only supported on parameters"));
            }
            binding.annotate(&mode, &access);
            Stmt::Let(
                binding,
                x.let_opt.as_ref().map(|t| type_name(&t.type_name)),
                x.let_opt0
                    .as_ref()
                    .map(|e| -> Result<Initializer, String> {
                        let value = expr(&e.expr)?;
                        Ok(match e.let_init_op.as_ref() {
                            g::LetInitOp::ColonEqu(_) => Initializer::Shared(value),
                            g::LetInitOp::Equ(_) => Initializer::Copied(value),
                        })
                    })
                    .transpose()?,
            )
        }
        g::SimpleStatement::Input(x) => {
            let mut items = vec![];
            for item in std::iter::once(x.input.input_item.as_ref())
                .chain(x.input.input_list.iter().map(|i| i.input_item.as_ref()))
            {
                let ty = type_name(&item.type_name);
                for item in std::iter::once(item.input_name.as_ref())
                    .chain(item.input_item_list.iter().map(|i| i.input_name.as_ref()))
                {
                    let args = item
                        .input_name_opt
                        .as_ref()
                        .map(|x| list(&x.expr_list))
                        .transpose()?
                        .unwrap_or_default();
                    items.push((name(&item.identifier), ty.clone(), args));
                }
            }
            Stmt::Input(items)
        }
        g::SimpleStatement::Output(x) => Stmt::Output(list(&x.output.expr_list)?),
        g::SimpleStatement::If(x) => {
            let statement = Stmt::If(
                condition(&x.r#if.condition)?,
                Box::new(block(&x.r#if.block)?),
                x.r#if
                    .if_opt
                    .as_ref()
                    .map(|x| block(&x.block).map(Box::new))
                    .transpose()?,
            );
            if x.r#if.if_opt0.is_some() {
                Stmt::Expr(Expr {
                    span: condition(&x.r#if.condition)?.span().clone(),
                    kind: ExprKind::Block(vec![statement], None),
                })
            } else {
                statement
            }
        }
        g::SimpleStatement::For(x) => for_stmt(&x.r#for)?,
        g::SimpleStatement::While(x) => while_stmt(&x.r#while)?,
        g::SimpleStatement::Loop(x) => Stmt::Loop(Box::new(block(&x.r#loop.block)?)),
        g::SimpleStatement::Break(x) => Stmt::Break(
            Span::token(&x.r#break.r#break),
            x.r#break.break_opt.as_ref().map(|x| name(&x.identifier)),
        ),
        g::SimpleStatement::Continue(x) => Stmt::Continue(
            Span::token(&x.r#continue.r#continue),
            x.r#continue
                .continue_opt
                .as_ref()
                .map(|x| name(&x.identifier)),
        ),
        g::SimpleStatement::LabeledLoop(x) => Stmt::Labeled(
            loop_label(&x.labeled_loop.loop_label)?,
            Box::new(match x.labeled_loop.loop_statement.as_ref() {
                g::LoopStatement::For(x) => for_stmt(&x.r#for)?,
                g::LoopStatement::While(x) => while_stmt(&x.r#while)?,
                g::LoopStatement::Loop(x) => Stmt::Loop(Box::new(block(&x.r#loop.block)?)),
            }),
        ),
    })
}
fn loop_label(label: &g::LoopLabel<'_>) -> Result<Name, String> {
    let name = Name {
        text: label
            .loop_label
            .text()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect(),
        span: Span::token(&label.loop_label),
    };
    if matches!(
        name.text.as_str(),
        "include"
            | "struct"
            | "enum"
            | "match"
            | "fn"
            | "return"
            | "extern"
            | "type"
            | "symbol"
            | "let"
            | "in"
            | "wt"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "break"
            | "continue"
            | "true"
            | "false"
    ) {
        return Err(name.span.error("reserved keyword cannot be a loop label"));
    }
    Ok(name)
}
fn for_stmt(x: &g::For<'_>) -> Result<Stmt, String> {
    Ok(Stmt::For(
        binding(&x.binding),
        expr(&x.expr)?,
        Box::new(block(&x.block)?),
        x.for_opt
            .as_ref()
            .map(|x| block(&x.block).map(Box::new))
            .transpose()?,
    ))
}
fn while_stmt(x: &g::While<'_>) -> Result<Stmt, String> {
    Ok(Stmt::While(
        condition(&x.condition)?,
        Box::new(block(&x.block)?),
        x.while_opt
            .as_ref()
            .map(|x| block(&x.block).map(Box::new))
            .transpose()?,
    ))
}
fn block_statement(b: &g::BlockStatement<'_>) -> Result<Stmt, String> {
    if b.block_statement_opt.is_some() {
        Ok(Stmt::Expr(block_expr(&b.block)?))
    } else {
        block(&b.block)
    }
}
fn block_parts(
    items: &g::BlockItems<'_>,
    statements: &mut Vec<Stmt>,
) -> Result<Option<Box<Expr>>, String> {
    match items {
        g::BlockItems::SimpleStatementBlockItems(x) => {
            statements.push(simple_stmt(&x.simple_statement)?);
            block_parts(&x.block_items, statements)
        }
        g::BlockItems::BlockStatementBlockItems(x) => {
            statements.push(block_statement(&x.block_statement)?);
            block_parts(&x.block_items, statements)
        }
        g::BlockItems::StatementExprBlockTail(x) => {
            let value = statement_expr(&x.statement_expr)?;
            match x.block_tail.as_ref() {
                g::BlockTail::SemicolonBlockItems(tail) => {
                    statements.push(Stmt::Expr(value));
                    block_parts(&tail.block_items, statements)
                }
                g::BlockTail::BlockTailEmpty(_) => Ok(Some(Box::new(value))),
            }
        }
        g::BlockItems::BlockItemsEmpty(_) => Ok(None),
    }
}
fn block(b: &g::Block<'_>) -> Result<Stmt, String> {
    let mut statements = vec![];
    let tail = block_parts(&b.block_items, &mut statements)?;
    Ok(Stmt::Block(statements, tail))
}
fn block_value(statement: Stmt, span: Span) -> Expr {
    let Stmt::Block(mut statements, mut tail) = statement else {
        unreachable!()
    };
    if tail.is_none() {
        match statements.last() {
            Some(Stmt::If(_, _, Some(_))) => {
                let Stmt::If(condition, yes, Some(no)) = statements.pop().unwrap() else {
                    unreachable!()
                };
                let position = condition.span().clone();
                tail = Some(Box::new(Expr {
                    span: position.clone(),
                    kind: ExprKind::If(
                        Box::new(condition),
                        Box::new(block_value(*yes, position.clone())),
                        Box::new(block_value(*no, position)),
                    ),
                }));
            }
            Some(Stmt::Match(_)) => {
                let Stmt::Match(value) = statements.pop().unwrap() else {
                    unreachable!()
                };
                tail = Some(Box::new(value));
            }
            Some(Stmt::Block(..)) => {
                tail = Some(Box::new(block_value(
                    statements.pop().unwrap(),
                    span.clone(),
                )));
            }
            _ => {}
        }
    }
    Expr {
        kind: ExprKind::Block(statements, tail),
        span,
    }
}
fn block_expr(b: &g::Block<'_>) -> Result<Expr, String> {
    Ok(block_value(block(b)?, Span::token(&b.l_brace)))
}
fn list(l: &g::ExprList<'_>) -> Result<Vec<Expr>, String> {
    let Some(x) = &l.expr_list_opt else {
        return Ok(vec![]);
    };
    std::iter::once(x.expr.as_ref())
        .chain(x.expr_list_opt_list.iter().map(|i| i.expr.as_ref()))
        .map(expr)
        .collect()
}
fn match_expr(m: &g::MatchExpr<'_>) -> Result<Expr, String> {
    let mut arms = vec![];
    for item in &m.match_expr_list {
        let a = &item.match_arm;
        let body = match a.arm_body.as_ref() {
            g::ArmBody::BlockArmBodyOpt(x) => block_expr(&x.block)?,
            g::ArmBody::StatementExprComma(x) => statement_expr(&x.statement_expr)?,
            g::ArmBody::IfExprComma(x) => if_expr(&x.if_expr)?,
            g::ArmBody::MatchExprComma(x) => match_expr(&x.match_expr)?,
        };
        arms.push(MatchArm {
            pattern: pattern(&a.pattern)?,
            guard: a
                .match_arm_opt
                .as_ref()
                .map(|g| expr(&g.guard.expr))
                .transpose()?,
            body,
        });
    }
    Ok(Expr {
        span: Span::token(&m.r#match),
        kind: ExprKind::Match(Box::new(expr(&m.expr)?), arms),
    })
}
fn expr(e: &g::Expr<'_>) -> Result<Expr, String> {
    match e {
        g::Expr::MatchExpr(x) => match_expr(&x.match_expr),
        g::Expr::PlainExpr(x) => plain_expr(&x.plain_expr),
        g::Expr::IfExpr(x) => if_expr(&x.if_expr),
    }
}
fn assignment(left: Expr, op: &g::AssignOp<'_>, right: &g::Expr<'_>) -> Result<Expr, String> {
    let (op, t) = match op {
        g::AssignOp::ColonEqu(x) => (":=", &x.colon_equ),
        g::AssignOp::Equ(x) => ("=", &x.equ),
        g::AssignOp::PlusEqu(x) => ("+=", &x.plus_equ),
        g::AssignOp::MinusEqu(x) => ("-=", &x.minus_equ),
        g::AssignOp::StarEqu(x) => ("*=", &x.star_equ),
        g::AssignOp::SlashEqu(x) => ("/=", &x.slash_equ),
        g::AssignOp::PercentEqu(x) => ("%=", &x.percent_equ),
        g::AssignOp::LTQuestEqu(x) => ("<?=", &x.l_t_quest_equ),
        g::AssignOp::GTQuestEqu(x) => (">?=", &x.g_t_quest_equ),
    };
    Ok(Expr {
        span: Span::token(t),
        kind: ExprKind::Assign(op.into(), Box::new(left), Box::new(expr(right)?)),
    })
}
fn plain_expr(e: &g::PlainExpr<'_>) -> Result<Expr, String> {
    match e {
        g::PlainExpr::Lambda(x) => lambda(&x.lambda),
        g::PlainExpr::BinaryPlainExprOpt(x) => {
            let first = unary(&x.binary.unary)?;
            let rest = x
                .binary
                .binary_list
                .iter()
                .map(|x| Ok((binary_op(&x.binary_op), unary(&x.unary)?)))
                .collect::<Result<Vec<_>, String>>()?;
            let value = binary(first, rest)?;
            if let Some(tail) = &x.plain_expr_opt {
                assignment(value, &tail.assign_op, &tail.expr)
            } else {
                Ok(value)
            }
        }
    }
}
fn statement_expr(e: &g::StatementExpr<'_>) -> Result<Expr, String> {
    match e {
        g::StatementExpr::Lambda(x) => lambda(&x.lambda),
        g::StatementExpr::StatementBinaryStatementExprOpt(x) => {
            let first = match x.statement_binary.statement_unary.as_ref() {
                g::StatementUnary::UnaryOpUnary(x) => unary_value(&x.unary_op, unary(&x.unary)?),
                g::StatementUnary::StatementPostfix(x) => {
                    let mut value = non_block_primary(&x.statement_postfix.non_block_primary)?;
                    for item in &x.statement_postfix.statement_postfix_list {
                        value = suffix(value, &item.suffix)?;
                    }
                    value
                }
            };
            let rest = x
                .statement_binary
                .statement_binary_list
                .iter()
                .map(|x| Ok((binary_op(&x.binary_op), unary(&x.unary)?)))
                .collect::<Result<Vec<_>, String>>()?;
            let value = binary(first, rest)?;
            if let Some(tail) = &x.statement_expr_opt {
                assignment(value, &tail.assign_op, &tail.expr)
            } else {
                Ok(value)
            }
        }
    }
}
fn unary_value(op: &g::UnaryOp<'_>, value: Expr) -> Expr {
    let (op, t) = match op {
        g::UnaryOp::Plus(x) => ("+", &x.plus),
        g::UnaryOp::Minus(x) => ("-", &x.minus),
        g::UnaryOp::Bang(x) => ("!", &x.bang),
    };
    Expr {
        span: Span::token(t),
        kind: ExprKind::Unary(op.into(), Box::new(value)),
    }
}
fn binary_op(op: &g::BinaryOp<'_>) -> (String, Span) {
    let token = match op {
        g::BinaryOp::QuestQuest(x) => &x.quest_quest,
        g::BinaryOp::DotDot(x) => &x.dot_dot,
        g::BinaryOp::OrOr(x) => &x.or_or,
        g::BinaryOp::AmpAmp(x) => &x.amp_amp,
        g::BinaryOp::Or(x) => &x.or,
        g::BinaryOp::Circumflex(x) => &x.circumflex,
        g::BinaryOp::Amp(x) => &x.amp,
        g::BinaryOp::EquEqu(x) => &x.equ_equ,
        g::BinaryOp::BangEqu(x) => &x.bang_equ,
        g::BinaryOp::LT(x) => &x.l_t,
        g::BinaryOp::LTEqu(x) => &x.l_t_equ,
        g::BinaryOp::GT(x) => &x.g_t,
        g::BinaryOp::GTEqu(x) => &x.g_t_equ,
        g::BinaryOp::LTQuest(x) => &x.l_t_quest,
        g::BinaryOp::GTQuest(x) => &x.g_t_quest,
        g::BinaryOp::Plus(x) => &x.plus,
        g::BinaryOp::Minus(x) => &x.minus,
        g::BinaryOp::Star(x) => &x.star,
        g::BinaryOp::Slash(x) => &x.slash,
        g::BinaryOp::Percent(x) => &x.percent,
    };
    (token.text().into(), Span::token(token))
}
fn precedence(op: &str) -> u8 {
    match op {
        "??" => 1,
        ".." => 2,
        "||" => 3,
        "&&" => 4,
        "|" => 5,
        "^" => 6,
        "&" => 7,
        "==" | "!=" | "<" | "<=" | ">" | ">=" => 8,
        "<?" | ">?" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" => 11,
        _ => unreachable!(),
    }
}
fn binary(first: Expr, rest: Vec<((String, Span), Expr)>) -> Result<Expr, String> {
    let mut values = std::iter::once(first).chain(rest.iter().map(|(_, v)| v.clone()));
    let first = values.next().unwrap();
    let mut ops = rest
        .iter()
        .map(|((op, span), _)| (op.clone(), span.clone()))
        .peekable();
    fn parse(
        left: Expr,
        minimum: u8,
        values: &mut impl Iterator<Item = Expr>,
        ops: &mut std::iter::Peekable<impl Iterator<Item = (String, Span)>>,
    ) -> Result<Expr, String> {
        let mut left = left;
        while let Some((op, _)) = ops.peek() {
            let p = precedence(op);
            if p < minimum {
                break;
            }
            let (op, span) = ops.next().unwrap();
            let right = parse(
                values.next().unwrap(),
                if op == "??" { p } else { p + 1 },
                values,
                ops,
            )?;
            if p == 8 {
                let mut rest = vec![(op, right, span.clone())];
                while ops.peek().is_some_and(|(op, _)| precedence(op) == 8) {
                    let (op, span) = ops.next().unwrap();
                    let right = parse(values.next().unwrap(), 9, values, ops)?;
                    rest.push((op, right, span));
                }
                let kind = if rest.len() == 1 {
                    let (op, right, _) = rest.pop().unwrap();
                    ExprKind::Binary(op, Box::new(left), Box::new(right))
                } else {
                    ExprKind::CompareChain(Box::new(left), rest)
                };
                left = Expr { span, kind };
                continue;
            }
            let kind = match op.as_str() {
                "??" => ExprKind::Coalesce(Box::new(left), Box::new(right)),
                ".." => {
                    if matches!(left.kind, ExprKind::Range(..)) {
                        return Err(span.error("range operators cannot be chained"));
                    }
                    ExprKind::Range(Box::new(left), Box::new(right))
                }
                "&&" | "||" => ExprKind::Logical(op, Box::new(left), Box::new(right)),
                _ => ExprKind::Binary(op, Box::new(left), Box::new(right)),
            };
            left = Expr { span, kind };
        }
        Ok(left)
    }
    parse(first, 1, &mut values, &mut ops)
}
fn unary(e: &g::Unary<'_>) -> Result<Expr, String> {
    match e {
        g::Unary::UnaryOpUnary(x) => {
            let (op, t) = match x.unary_op.as_ref() {
                g::UnaryOp::Plus(x) => ("+", &x.plus),
                g::UnaryOp::Minus(x) => ("-", &x.minus),
                g::UnaryOp::Bang(x) => ("!", &x.bang),
            };
            Ok(Expr {
                kind: ExprKind::Unary(op.into(), Box::new(unary(&x.unary)?)),
                span: Span::token(t),
            })
        }
        g::Unary::Postfix(x) => postfix(&x.postfix),
    }
}
fn postfix(p: &g::Postfix<'_>) -> Result<Expr, String> {
    let mut value = primary(&p.primary)?;
    for item in &p.postfix_list {
        value = suffix(value, &item.suffix)?;
    }
    Ok(value)
}
fn suffix(value: Expr, suffix: &g::Suffix<'_>) -> Result<Expr, String> {
    Ok(match suffix {
        g::Suffix::Unwrap(x) => Expr {
            kind: ExprKind::Unwrap(Box::new(value)),
            span: Span::token(&x.unwrap.unwrap),
        },
        g::Suffix::Index(x) => Expr {
            kind: ExprKind::Index(Box::new(value), Box::new(expr(&x.index.expr)?)),
            span: Span::token(&x.index.l_bracket),
        },
        g::Suffix::Member(x) => {
            let n = name(&x.member.identifier);
            Expr {
                span: n.span.clone(),
                kind: ExprKind::Field(Box::new(value), n),
            }
        }
        g::Suffix::Invoke(x) => {
            let span = value.span.clone();
            let args = list(&x.invoke.expr_list)?;
            let kind = match value.kind {
                ExprKind::Var(n) => ExprKind::Call(n, args),
                _ => ExprKind::Invoke(Box::new(value), args),
            };
            Expr { span, kind }
        }
    })
}
fn primary(e: &g::Primary<'_>) -> Result<Expr, String> {
    match e {
        g::Primary::Block(x) => block_expr(&x.block),
        g::Primary::NonBlockPrimary(x) => non_block_primary(&x.non_block_primary),
    }
}
fn non_block_primary(e: &g::NonBlockPrimary<'_>) -> Result<Expr, String> {
    Ok(match e {
        g::NonBlockPrimary::Array(x) => {
            let kind = if let Some(items) = &x.array.array_opt {
                let first = expr(&items.expr)?;
                match items.array_tail.as_ref() {
                    g::ArrayTail::SemicolonExpr(x) => {
                        ExprKind::Repeat(Box::new(first), Box::new(expr(&x.expr)?))
                    }
                    g::ArrayTail::ArrayTailList(x) => {
                        let mut values = vec![first];
                        values.extend(
                            x.array_tail_list
                                .iter()
                                .map(|x| expr(&x.expr))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                        ExprKind::Array(values)
                    }
                }
            } else {
                ExprKind::Array(vec![])
            };
            Expr {
                kind,
                span: Span::token(&x.array.l_bracket),
            }
        }
        g::NonBlockPrimary::LParenExprNonBlockPrimaryOpt0RParen(x) => {
            let first = expr(&x.expr)?;
            if let Some(tail) = &x.non_block_primary_opt0 {
                let mut values = vec![first];
                values.push(expr(&tail.tuple_tail.expr)?);
                for item in &tail.tuple_tail.tuple_tail_list {
                    values.push(expr(&item.expr)?);
                }
                Expr {
                    span: Span::token(&x.l_paren),
                    kind: ExprKind::Tuple(values),
                }
            } else {
                return Ok(first);
            }
        }
        g::NonBlockPrimary::ReferenceNonBlockPrimaryOpt(x) => {
            let n = reference(&x.reference);
            Expr {
                span: n.span.clone(),
                kind: if let Some(record) = &x.non_block_primary_opt {
                    let fields = &record.record_fields;
                    let values = std::iter::once(fields.field_value.as_ref())
                        .chain(
                            fields
                                .record_fields_list
                                .iter()
                                .map(|f| f.field_value.as_ref()),
                        )
                        .map(|f| Ok((name(&f.identifier), expr(&f.expr)?)))
                        .collect::<Result<_, String>>()?;
                    ExprKind::Record(n, values)
                } else {
                    ExprKind::Var(n)
                },
            }
        }
        g::NonBlockPrimary::Literal(x) => literal_expr(&x.literal),
    })
}

fn lambda(e: &g::Lambda<'_>) -> Result<Expr, String> {
    let parameters = parameters(&e.parameters)?;
    Ok(Expr {
        kind: ExprKind::Lambda(parameters, Box::new(expr(&e.expr)?)),
        span: Span::token(&e.l_paren),
    })
}

fn literal_expr(literal: &g::Literal<'_>) -> Expr {
    let (ty, t) = match literal {
        g::Literal::IntLiteral(x) => ("Int", &x.int_literal.int_literal),
        g::Literal::StringLiteral(x) => ("Str", &x.string_literal.string_literal),
        g::Literal::CharLiteral(x) => ("Char", &x.char_literal.char_literal),
        g::Literal::BoolLiteral(x) => (
            "Bool",
            match x.bool_literal.as_ref() {
                g::BoolLiteral::True(x) => &x.r#true,
                g::BoolLiteral::False(x) => &x.r#false,
            },
        ),
    };
    Expr {
        kind: ExprKind::Literal(ty.into(), t.text().into()),
        span: Span::token(t),
    }
}

fn condition(c: &g::Condition<'_>) -> Result<Condition, String> {
    Ok(match c {
        g::Condition::Expr(x) => Condition::Expr(expr(&x.expr)?),
        g::Condition::LetCondition(x) => Condition::Let(
            pattern(&x.let_condition.pattern)?,
            expr(&x.let_condition.expr)?,
        ),
    })
}
fn pattern(p: &g::Pattern<'_>) -> Result<Pattern, String> {
    let first = pattern_atom(&p.pattern_atom)?;
    if p.pattern_list0.is_empty() {
        return Ok(first);
    }
    let span = first.span.clone();
    let mut items = vec![first];
    for x in &p.pattern_list0 {
        items.push(pattern_atom(&x.pattern_atom)?);
    }
    Ok(Pattern {
        kind: PatternKind::Or(items),
        span,
    })
}
fn pattern_list(p: &g::PatternList<'_>) -> Result<Vec<Pattern>, String> {
    std::iter::once(p.pattern.as_ref())
        .chain(p.pattern_list_list.iter().map(|p| p.pattern.as_ref()))
        .map(pattern)
        .collect()
}
fn pattern_atom(p: &g::PatternAtom<'_>) -> Result<Pattern, String> {
    Ok(match p {
        g::PatternAtom::Literal(x) => {
            let e = literal_expr(&x.literal);
            let ExprKind::Literal(ty, text) = e.kind else {
                unreachable!()
            };
            Pattern {
                kind: PatternKind::Literal(ty, text),
                span: e.span,
            }
        }
        g::PatternAtom::NegativePattern(x) => {
            let t = &x.negative_pattern.int_literal.int_literal;
            Pattern {
                kind: PatternKind::Literal("Int".into(), format!("-{}", t.text())),
                span: Span::token(t),
            }
        }
        g::PatternAtom::ParenPattern(x) => {
            let p = &x.paren_pattern;
            let mut items = vec![];
            if let Some(v) = &p.paren_pattern_opt {
                let first = pattern(&v.pattern)?;
                match v.paren_pattern_tail.as_ref() {
                    g::ParenPatternTail::ParenPatternTailEmpty(_) => return Ok(first),
                    g::ParenPatternTail::CommaPatternParenPatternTailListParenPatternTailOpt(t) => {
                        items.push(first);
                        items.push(pattern(&t.pattern)?);
                        for t in &t.paren_pattern_tail_list {
                            items.push(pattern(&t.pattern)?);
                        }
                    }
                }
            }
            Pattern {
                kind: PatternKind::Tuple(items),
                span: Span::token(&p.l_paren),
            }
        }
        g::PatternAtom::NamedPattern(x) => {
            let p = &x.named_pattern;
            let n = reference(&p.reference);
            let span = n.span.clone();
            let kind = match &p.named_pattern_opt {
                None => PatternKind::Named(n, None),
                Some(t) => match t.named_pattern_tail.as_ref() {
                    g::NamedPatternTail::LParenNamedPatternTailOptRParen(x) => PatternKind::Named(
                        n,
                        Some(
                            x.named_pattern_tail_opt
                                .as_ref()
                                .map(|p| pattern_list(&p.pattern_list))
                                .transpose()?
                                .unwrap_or_default(),
                        ),
                    ),
                    g::NamedPatternTail::PatternRecord(x) => {
                        let mut fields = vec![];
                        let mut rest = false;
                        if let Some(p) = &x.pattern_record.pattern_record_opt {
                            for f in std::iter::once(p.pattern_fields.pattern_field.as_ref()).chain(
                                p.pattern_fields
                                    .pattern_fields_list
                                    .iter()
                                    .map(|f| f.pattern_field.as_ref()),
                            ) {
                                if rest {
                                    return Err(
                                        span.error(".. must be the last struct pattern field")
                                    );
                                }
                                match f {
                                    g::PatternField::DotDot(_) => rest = true,
                                    g::PatternField::IdentifierPatternFieldOpt(x) => {
                                        let f = name(&x.identifier);
                                        let value = if let Some(p) = &x.pattern_field_opt {
                                            pattern(&p.pattern)?
                                        } else {
                                            Pattern {
                                                kind: PatternKind::Named(f.clone(), None),
                                                span: f.span.clone(),
                                            }
                                        };
                                        fields.push((f, value));
                                    }
                                }
                            }
                        }
                        PatternKind::Record(n, fields, rest)
                    }
                },
            };
            Pattern { kind, span }
        }
    })
}

fn if_expr(x: &g::IfExpr<'_>) -> Result<Expr, String> {
    Ok(Expr {
        span: Span::token(&x.r#if),
        kind: ExprKind::If(
            Box::new(condition(&x.condition)?),
            Box::new(block_expr(&x.block)?),
            Box::new(block_expr(&x.block0)?),
        ),
    })
}
