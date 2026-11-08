//! Target-specific representation selection and C++ emission from LIR.
mod cpp;
mod intrinsics;
mod program;
mod storage;
pub(crate) use program::{Context, emit as emit_program};
pub(crate) struct FunctionInfo {
    pub cpp: String,
    pub name: String,
    pub result: String,
    pub trusted: bool,
}
pub(crate) use cpp::emit as cpp;

#[derive(Clone, Copy)]
pub(crate) enum Emit {
    Cpp,
    Hir,
    Lir,
}
