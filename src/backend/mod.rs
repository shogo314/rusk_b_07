//! Target-specific representation selection and C++ emission from LIR.
mod cpp;
mod intrinsics;
mod owned_aliases;
mod program;
mod scalar_aliases;
mod storage;
pub(crate) use program::{Context, emit as emit_program};
pub(crate) struct FunctionInfo {
    pub cpp: String,
    pub name: String,
    pub result: String,
    pub trusted: bool,
}
pub(crate) use cpp::emit as cpp;

/// This must agree at function signatures and at callers borrowing aliases.
/// User annotations alone and native contracts do not select a borrowed ABI.
fn borrows_parameter(function: &crate::lir::ProgramFunction, index: usize) -> bool {
    use crate::effects::{Effect, SummarySource};
    let parameter = &function.effects.parameters[index];
    function.body.is_some()
        && function.effects.source == SummarySource::Inferred
        && function.effects.external_write == Effect::No
        && parameter.write == Effect::No
        && parameter.structural == Effect::No
        && parameter.escape == Effect::No
}

#[derive(Clone, Copy)]
pub(crate) enum Emit {
    Cpp,
    Hir,
    Lir,
}
