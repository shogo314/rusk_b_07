use super::FunctionInfo;
use crate::hir::Call;

/// Only operations resolved to the embedded standard library are lowered.
/// The generated scalar result is a fresh value, never a snapshot of an alias.
pub(super) fn lower(call: &Call, info: &FunctionInfo) -> Option<String> {
    if !info.trusted {
        return None;
    }
    let args = &call.args;
    let a = args
        .first()
        .map(|v| format!("*{}", v.id))
        .unwrap_or_default();
    let b = args
        .get(1)
        .map(|v| format!("*{}", v.id))
        .unwrap_or_default();
    let first = args.first().map(|v| v.id.to_string()).unwrap_or_default();
    let ty = args.first().map(|v| v.ty.as_str()).unwrap_or_default();
    let scalar = matches!(ty, "Int" | "Bool" | "Char");
    let integers = args.len() == 2 && args.iter().all(|v| v.ty == "Int");
    let loc = super::cpp::call_location(call);
    match info.name.as_str() {
        "__add__" | "__sub__" | "__mul__" if integers => {
            let op = match info.name.as_str() {
                "__add__" => "+",
                "__sub__" => "-",
                _ => "*",
            };
            Some(format!(
                "checked_int(static_cast<__int128>({a}) {op} {b}, {loc})"
            ))
        }
        "__div__" | "__mod__" if integers => Some(format!(
            "rusk_{}_int({a}, {b}, {loc})",
            if info.name == "__div__" { "div" } else { "mod" }
        )),
        "__bitand__" | "__bitor__" | "__bitxor__" if integers => {
            let op = match info.name.as_str() {
                "__bitand__" => "&",
                "__bitor__" => "|",
                _ => "^",
            };
            Some(format!("({a} {op} {b})"))
        }
        "__eq__" | "__ne__" | "__lt__" | "__le__" | "__gt__" | "__ge__"
            if scalar || ty == "Str" =>
        {
            let op = match info.name.as_str() {
                "__eq__" => "==",
                "__ne__" => "!=",
                "__lt__" => "<",
                "__le__" => "<=",
                "__gt__" => ">",
                _ => ">=",
            };
            Some(format!("({a} {op} {b})"))
        }
        "__pos__" if ty == "Int" => Some(a),
        "__neg__" if ty == "Int" => {
            Some(format!("checked_int(-static_cast<__int128>({a}), {loc})"))
        }
        "copy" if scalar => Some(a),
        "__assign__" if scalar => Some(format!("{a} = {b}")),
        "__write__" if scalar || ty == "Str" => Some(format!("rusk_write({a})")),
        "__bool__" | "__not__" if scalar || ty == "Str" || ty.starts_with("Vec<") => {
            let value = if scalar {
                format!("({a} != 0)")
            } else {
                format!("!{first}->empty()")
            };
            Some(if info.name == "__not__" {
                format!("!({value})")
            } else {
                value
            })
        }
        "__read__" if args.is_empty() && call.result.ty == "Int" => {
            Some(format!("rusk_read_int({loc})"))
        }
        "len" if ty == "Str" || ty.starts_with("Vec<") => Some(format!(
            "checked_int(static_cast<__int128>({first}->size()), {loc})"
        )),
        "__iter_has_next__" if ty == "RangeIter" => Some(format!("({first}->next < {first}->end)")),
        "__iter_next__" if ty == "RangeIter" => Some(format!("rusk_range_next({first}, {loc})")),
        "__iter_has_next__" if ty.starts_with("Iter<") => Some(format!(
            "(check_iter({first},{loc}), {first}->index < {first}->owner->size())"
        )),
        _ => None,
    }
}
