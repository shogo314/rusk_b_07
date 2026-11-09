use super::{FunctionInfo, intrinsics};
use crate::{
    hir::{Literal, Place, Rvalue, ValueId},
    lir::{Function, Operation as Instruction, Terminator},
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn emit(
    function: &Function,
    types: &BTreeMap<String, String>,
    functions: &[FunctionInfo],
    summaries: &[crate::lir::ProgramFunction],
    optimize: bool,
    current: Option<&crate::lir::ProgramFunction>,
) -> String {
    let mut scalar = BTreeSet::new();
    if optimize {
        for block in &function.blocks {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::Literal { result, .. }
                        if matches!(result.ty.as_str(), "Int" | "Bool" | "Char" | "Unit") =>
                    {
                        scalar.insert(result.id.clone());
                    }
                    Instruction::Object {
                        result,
                        expression: Rvalue::BoolCopy(_) | Rvalue::VariantTest { .. },
                    } => {
                        scalar.insert(result.id.clone());
                    }
                    Instruction::Call(call)
                        if functions[call.function].result == "Unit"
                            || intrinsics::lower(call, &functions[call.function]).is_some() =>
                    {
                        scalar.insert(call.result.id.clone());
                    }
                    _ => {}
                }
            }
        }
    }
    let scalar_aliases = if optimize {
        super::scalar_aliases::plan(function, functions, summaries)
    } else {
        BTreeMap::new()
    };
    let parameter_aliases = if optimize {
        current.map_or_else(BTreeMap::new, |current| {
            super::scalar_aliases::parameter_plan(function, functions, summaries, current)
        })
    } else {
        BTreeMap::new()
    };
    let owned = if optimize {
        super::owned_aliases::plan(function, functions, summaries, &scalar_aliases)
    } else {
        super::owned_aliases::Plan::default()
    };
    let across_owner = |id: &ValueId| {
        owned
            .aliases
            .get(id)
            .filter(|owner| owned.across_blocks.contains(*owner))
    };
    let inline_storage = |id: &ValueId| {
        if scalar_aliases.contains_key(id) {
            "RuskInlineScalar"
        } else {
            "RuskTemporary"
        }
    };
    let mut persistent = super::storage::persistent(function);
    persistent.extend(scalar_aliases.values().cloned());
    persistent.extend(owned.across_blocks.iter().cloned());
    let declaration = |value: &crate::hir::Value| {
        if persistent.contains(&value.id) {
            String::new()
        } else {
            let storage = if scalar.contains(&value.id) {
                inline_storage(&value.id)
            } else {
                "Obj"
            };
            format!("{storage}<{}> ", types[&value.ty])
        }
    };
    let mut out = String::new();
    for slot in &function.slots {
        if parameter_aliases.contains_key(&slot.id)
            || across_owner(&slot.id).is_some_and(|owner| *owner != slot.id)
        {
            continue;
        }
        if scalar_aliases
            .get(&slot.id)
            .is_some_and(|owner| *owner != slot.id)
        {
            continue;
        }
        if persistent.contains(&slot.id) {
            let storage = if scalar.contains(&slot.id) {
                inline_storage(&slot.id)
            } else {
                "Obj"
            };
            out += &format!("{storage}<{}> {};\n", types[&slot.ty], slot.id);
        }
    }
    for slot in &function.slots {
        if let Some(owner) = across_owner(&slot.id)
            && *owner != slot.id
        {
            out += &format!(
                "[[maybe_unused]] const Obj<{}>& {}={owner};\n",
                types[&slot.ty], slot.id
            );
        }
        if let Some(parameter) = parameter_aliases.get(&slot.id) {
            out += &format!(
                "[[maybe_unused]] const Obj<{}>& {}={parameter};\n",
                types[&slot.ty], slot.id
            );
        }
        if let Some(owner) = scalar_aliases.get(&slot.id)
            && slot.id != *owner
        {
            out += &format!(
                "RuskInlineScalar<{}>& {}={owner};\n",
                types[&slot.ty], slot.id
            );
        }
    }
    let release = |block: usize, position: usize, values: &[ValueId]| {
        values
            .iter()
            .enumerate()
            .filter(|(_, v)| !parameter_aliases.contains_key(*v))
            .filter(|(_, v)| !scalar_aliases.get(*v).is_some_and(|owner| owner != *v))
            .filter_map(|(event, v)| {
                if across_owner(v).is_some() {
                    return owned
                        .release_sites
                        .get(&(block, position, event))
                        .map(|owner| format!("{owner}.reset();\n"));
                }
                match owned.releases.get(v) {
                    Some(Some(owner)) if !owned.elements.contains(owner) => {
                        Some(format!("{owner}.reset();\n"))
                    }
                    Some(Some(_)) => None,
                    Some(None) => None,
                    None => Some(format!("{v}.reset();\n")),
                }
            })
            .collect::<String>()
    };
    for (index, block) in function.blocks.iter().enumerate() {
        out += &format!("rusk_block_{index}:{{\n");
        for (position, instruction) in block.instructions.iter().enumerate() {
            if let Instruction::Declare { result, .. }
            | Instruction::Object {
                result,
                expression: Rvalue::Share(_),
            } = instruction
                && (parameter_aliases.contains_key(&result.id)
                    || across_owner(&result.id).is_some_and(|owner| *owner != result.id)
                    || scalar_aliases
                        .get(&result.id)
                        .is_some_and(|owner| *owner != result.id))
            {
                continue;
            }
            if let Instruction::Declare { result, .. }
            | Instruction::Object {
                result,
                expression: Rvalue::Share(_),
            } = instruction
                && let Some(owner) = owned.aliases.get(&result.id)
                && *owner != result.id
            {
                out += &format!("const Obj<{}>& {}={owner};\n", types[&result.ty], result.id);
                continue;
            }
            match instruction {
                Instruction::Literal { result, value } => {
                    let expression = match value {
                        Literal::Unit => "Unit{}".into(),
                        Literal::Int(n) if *n == i64::MIN => {
                            "(-INT64_C(9223372036854775807)-1)".into()
                        }
                        Literal::Int(n) => format!("INT64_C({n})"),
                        Literal::Bool(v) => v.to_string(),
                        Literal::Char(v) => format!("static_cast<char>({v})"),
                        Literal::Str(v) => {
                            format!("std::string({}, {})", quoted(v), v.len())
                        }
                    };
                    let constructor = if scalar.contains(&result.id) {
                        inline_storage(&result.id)
                    } else {
                        "make_obj"
                    };
                    out += &format!(
                        "{}{}={constructor}<{}>({expression});\n",
                        declaration(result),
                        result.id,
                        types[&result.ty]
                    );
                }
                Instruction::Object { result, expression } => {
                    let native = &types[&result.ty];
                    let rhs = match expression {
                        Rvalue::Share(value) => value.id.to_string(),
                        Rvalue::Field { owner, index } => format!("{}->field_{index}", owner.id),
                        Rvalue::Aggregate(fields) => format!(
                            "make_obj<{native}>({native}{{{}}})",
                            fields
                                .iter()
                                .map(|f| f.id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        ),
                        Rvalue::BoolCopy(value) => format!(
                            "{}<bool>(*{})",
                            if scalar.contains(&result.id) {
                                inline_storage(&result.id)
                            } else {
                                "make_obj"
                            },
                            value.id
                        ),
                        Rvalue::Variant { index, fields } => format!(
                            "rusk_enum_make<{native},{index}>({})",
                            fields
                                .iter()
                                .map(|v| v.id.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        ),
                        Rvalue::VariantTest { owner, index } => {
                            format!(
                                "{}<bool>(rusk_enum_index<{}>({}->value)=={index})",
                                if scalar.contains(&result.id) {
                                    inline_storage(&result.id)
                                } else {
                                    "make_obj"
                                },
                                types[&owner.ty],
                                owner.id
                            )
                        }
                        Rvalue::VariantField {
                            owner,
                            variant,
                            index,
                        } => format!(
                            "rusk_enum_payload<{}, {variant}>({}->value).field_{index}",
                            types[&owner.ty], owner.id
                        ),
                    };
                    out += &format!("{}{}={rhs};\n", declaration(result), result.id);
                }
                Instruction::Declare { result, initial } => {
                    if let Some(initial) = initial {
                        out += &format!("{}{}={};\n", declaration(result), result.id, initial.id);
                    } else if !persistent.contains(&result.id) {
                        out += &format!("{}{};\n", declaration(result), result.id);
                    }
                }
                Instruction::Rebind { target, value, .. } => {
                    let target = match target {
                        Place::Local(id) => id.to_string(),
                        Place::Field { owner, index } => format!("{}->field_{index}", owner.id),
                    };
                    out += &format!("{target}={};\n", value.id);
                }
                Instruction::StoreBool { target, value } => {
                    out += &format!("*{}=*{};\n", target.id, value.id)
                }
                Instruction::DescribeBinding { .. }
                | Instruction::AnnotateBinding { .. }
                | Instruction::AnnotateAccess { .. }
                | Instruction::AnnotateEscape { .. } => {}
                Instruction::Release(values) => out += &release(index, position, values),
                Instruction::Call(call) => {
                    let info = &functions[call.function];
                    let direct = if optimize {
                        if owned.elements.contains(&call.result.id) {
                            intrinsics::lower_borrowed(call, info)
                        } else {
                            intrinsics::lower(call, info)
                                .or_else(|| intrinsics::lower_owned(call, info))
                        }
                    } else {
                        None
                    };
                    let expression = if let Some(direct) = direct {
                        direct
                    } else {
                        let mut args = call
                            .args
                            .iter()
                            .map(|v| v.id.to_string())
                            .collect::<Vec<_>>();
                        args.push(call_location(call));
                        format!("{}({})", info.cpp, args.join(","))
                    };
                    if call.result.ty == "Unit" {
                        let constructor = if scalar.contains(&call.result.id) {
                            "RuskTemporary"
                        } else {
                            "make_obj"
                        };
                        out += &format!(
                            "{expression};\n{}{}={constructor}<Unit>(Unit{{}});\n",
                            declaration(&call.result),
                            call.result.id
                        );
                    } else if scalar.contains(&call.result.id) {
                        out += &format!(
                            "{}{}={}<{}>({expression});\n",
                            declaration(&call.result),
                            call.result.id,
                            inline_storage(&call.result.id),
                            types[&call.result.ty]
                        );
                    } else {
                        let storage = if owned.elements.contains(&call.result.id) {
                            format!("const Obj<{}>& ", types[&call.result.ty])
                        } else {
                            declaration(&call.result)
                        };
                        out += &format!("{}{}={expression};\n", storage, call.result.id);
                    }
                }
            }
        }
        match &block.terminator {
            Terminator::Jump(target) => out += &format!("goto rusk_block_{};\n", target.0),
            Terminator::Branch { condition, yes, no } => {
                out += &format!(
                    "if(*{})goto rusk_block_{};else goto rusk_block_{};\n",
                    condition.id, yes.0, no.0
                )
            }
            Terminator::Return { value, cleanup, .. } => {
                if let Some(value) = value {
                    out += &format!(
                        "{{Obj<{}> rusk_return={};\n{}return rusk_return;}}\n",
                        types[&value.ty],
                        value.id,
                        release(index, block.instructions.len(), cleanup)
                    );
                } else {
                    out += &release(index, block.instructions.len(), cleanup);
                    out += if current.is_none() {
                        "return 0;\n"
                    } else {
                        "return;\n"
                    };
                }
            }
            Terminator::Unreachable => out += "std::abort();\n",
        }
        out += "}\n";
    }
    out
}

impl std::fmt::Display for ValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(id) => write!(f, "rusk_v{id}"),
            Self::Parameter(id) => write!(f, "rusk_p{id}"),
            Self::Probe => panic!("type-resolution probe cannot reach code generation"),
        }
    }
}

pub(super) fn quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            32..=126 => out.push(char::from(b)),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out.push('"');
    out
}
pub(super) fn location(s: &crate::ast::Span) -> String {
    format!("Loc{{{}, {}, {}}}", quoted(&s.file), s.line, s.column)
}

pub(super) fn call_location(call: &crate::hir::Call) -> String {
    if call.inherit_location {
        "rusk_call_loc".into()
    } else {
        location(&call.span)
    }
}
