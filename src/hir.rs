//! Resolved, typed Rusk operations. Bodies contain no target-language source.
use crate::ast::Span;

/// A binding identity, independent of its source spelling. Parameter identities
/// are local to a function; all other identities come from the compiler counter.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ValueId {
    Local(usize),
    Parameter(usize),
    Probe,
}
pub(crate) type FunctionId = usize;
pub(crate) type LoopId = usize;
#[derive(Clone, Debug)]
pub(crate) struct Value {
    pub id: ValueId,
    pub ty: String,
}
#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub function: FunctionId,
    pub args: Vec<Value>,
    pub span: Span,
    /// Standard-library wrappers report failures at their caller.
    pub inherit_location: bool,
    pub result: Value,
}
#[derive(Clone, Debug)]
pub(crate) enum Literal {
    Unit,
    Int(i64),
    Bool(bool),
    Char(u8),
    Str(String),
}
#[derive(Clone, Debug)]
pub(crate) enum Rvalue {
    Share(Value),
    Field {
        owner: Value,
        index: usize,
    },
    Aggregate(Vec<Value>),
    BoolCopy(Value),
    Variant {
        index: usize,
        fields: Vec<Value>,
    },
    VariantTest {
        owner: Value,
        index: usize,
    },
    VariantField {
        owner: Value,
        variant: usize,
        index: usize,
    },
}
#[derive(Clone, Debug)]
pub(crate) enum Place {
    Local(ValueId),
    Field { owner: Value, index: usize },
}
#[derive(Clone, Debug)]
pub(crate) struct BindingInfo {
    pub name: String,
    pub declaration: Span,
    pub annotation: crate::ast::BindingAnnotation,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AccessInfo {
    pub name: String,
    pub declaration: Span,
    pub annotation: crate::ast::AccessAnnotation,
}
#[derive(Clone, Debug)]
pub(crate) struct EscapeInfo {
    pub name: String,
    pub declaration: Span,
    pub annotation: crate::ast::EscapeAnnotation,
}
#[derive(Clone, Debug)]
pub(crate) enum Instruction {
    /// Source names for diagnostics; independent of declared contracts.
    DescribeBinding {
        value: Value,
        name: crate::ast::Name,
        captured: bool,
    },
    AnnotateEscape {
        parameter: usize,
        info: EscapeInfo,
    },
    AnnotateAccess {
        value: Value,
        info: AccessInfo,
    },
    AnnotateBinding {
        value: Value,
        info: BindingInfo,
    },
    Object {
        result: Value,
        expression: Rvalue,
    },
    Literal {
        result: Value,
        value: Literal,
    },
    Call(Call),
    Declare {
        result: Value,
        initial: Option<Value>,
    },
    Rebind {
        target: Place,
        value: Value,
        span: Option<Span>,
    },
    StoreBool {
        target: Value,
        value: Value,
    },
    Release(Vec<ValueId>),
    Scope(Body),
    If {
        condition: Value,
        yes: Body,
        no: Body,
    },
    Loop {
        id: LoopId,
        body: Body,
    },
    Break(LoopId),
    Continue(LoopId),
    Return {
        value: Option<Value>,
        span: Option<Span>,
    },
    Unreachable,
}
#[derive(Clone, Default, Debug)]
pub(crate) struct Body {
    pub instructions: Vec<Instruction>,
}
impl Body {
    pub fn clear(&mut self) {
        self.instructions.clear();
    }
    pub fn push(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Type {
    Primitive,
    Record(Vec<(String, String)>),
    Callable {
        fields: Vec<(String, String)>,
        function: FunctionId,
        params: Vec<String>,
        result: String,
    },
    Vector(String),
    Enum(Vec<(String, Vec<String>)>),
    Dict {
        key: String,
        value: String,
    },
    Set(String),
    Iterator {
        element: String,
    },
    Opaque,
}
impl Type {
    pub fn fields(&self) -> Option<&[(String, String)]> {
        match self {
            Self::Record(fields) | Self::Callable { fields, .. } => Some(fields),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Function {
    pub id: FunctionId,
    pub name: String,
    pub params: Vec<String>,
    pub result: String,
    pub body: Option<Body>,
    pub effects: crate::effects::Summary,
    pub access: crate::readonly::Summary,
}
#[derive(Debug)]
pub(crate) struct Program {
    pub types: std::collections::BTreeMap<String, Type>,
    pub functions: Vec<Function>,
    pub main: Body,
}

pub(crate) const DIVERGED: &str = "<diverged>";
impl Value {
    pub fn never() -> Self {
        Self {
            id: ValueId::Probe,
            ty: DIVERGED.into(),
        }
    }
    pub fn is_never(&self) -> bool {
        self.ty == DIVERGED
    }
}
