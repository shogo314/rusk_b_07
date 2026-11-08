//! Explicit semantic contracts for the embedded runtime. These describe Rusk
//! operations, never C++ names or source patterns. They are a trust boundary.
use crate::effects::{Effect, ReturnAlias, Summary};
use std::collections::BTreeMap;

pub(crate) fn contract(
    specification: &str,
    parameters: &[String],
    result: &str,
    types: &BTreeMap<String, crate::hir::Type>,
) -> Result<Summary, String> {
    fn builtin_value(ty: &str, types: &BTreeMap<String, crate::hir::Type>) -> bool {
        match types.get(ty) {
            Some(crate::hir::Type::Primitive) => {
                matches!(ty, "Int" | "Bool" | "Char" | "Str" | "Unit")
            }
            Some(crate::hir::Type::Vector(element)) => builtin_value(element, types),
            _ => false,
        }
    }
    let specification = match specification {
        "deep_read_fresh" | "deep_resize_first" | "deep_io_write" => {
            // Generated record copies can invoke user-defined copy functions;
            // foreign data can also supply arbitrary target-language helpers.
            if parameters.iter().any(|ty| !builtin_value(ty, types)) {
                return Ok(Summary::opaque(parameters.len()));
            }
            match specification {
                "deep_read_fresh" => "checked_read_fresh",
                "deep_resize_first" => "checked_resize_first",
                _ => "io_write",
            }
        }
        other => other,
    };
    let mut summary = Summary::native(parameters.len());
    for parameter in &mut summary.parameters {
        parameter.read = Effect::May;
    }
    summary.returned.fresh = result != "Unit";
    match specification {
        "read_fresh" => {}
        "checked_read_fresh" => summary.may_fail = Effect::May,
        "write_first" | "checked_write_first" | "checked_resize_first" | "checked_mutate_fresh" => {
            let first = summary
                .parameters
                .first_mut()
                .ok_or("native write contract requires an argument")?;
            first.write = Effect::May;
            if specification == "checked_resize_first" {
                first.structural = Effect::May;
            }
            if specification != "write_first" {
                summary.may_fail = Effect::May;
            }
            if specification != "checked_mutate_fresh" && result != "Unit" {
                return Err("native write contract requires Unit result".into());
            }
        }
        "io_read_fresh" | "io_write" => {
            summary.io = Effect::May;
            summary.may_diverge = Effect::May; // IO can block.
            if specification == "io_read_fresh" {
                summary.may_fail = Effect::May;
                if result == "Unit" {
                    return Err("native input contract requires a result".into());
                }
            } else if result != "Unit" {
                return Err("native output contract requires Unit result".into());
            }
        }
        "resize_retain_second" | "replace_element" => {
            let source = if specification == "replace_element" {
                2
            } else {
                1
            };
            if parameters.len() != source + 1 {
                return Err("native element store contract has invalid arity".into());
            }
            let Some(crate::hir::Type::Vector(element)) = types.get(&parameters[0]) else {
                return Err("native element store requires a vector".into());
            };
            if *element != parameters[source] || (source == 2 && parameters[1] != "Int") {
                return Err("native element store type mismatch".into());
            }
            summary.parameters[0].write = Effect::May;
            summary.parameters[0].structural = if source == 1 { Effect::May } else { Effect::No };
            summary.parameters[source].escape = Effect::May;
            summary.parameters[source].retained = Effect::May;
            if source == 1 {
                if result != "Unit" {
                    return Err("native append requires Unit result".into());
                }
            } else {
                if result != element {
                    return Err("native element store result mismatch".into());
                }
                summary.may_fail = Effect::May;
                summary.returned.fresh = false;
                summary.returned.aliases.insert(ReturnAlias {
                    result_path: vec![],
                    parameter: source,
                    parameter_path: vec![],
                });
            }
        }
        "option_some" => {
            if parameters.len() != 1 {
                return Err("native Some contract requires one value".into());
            }
            let Some(crate::hir::Type::Enum(variants)) = types.get(result) else {
                return Err("native Some contract requires an enum result".into());
            };
            if variants
                .first()
                .is_none_or(|(name, fields)| name != "Some" || fields != parameters)
            {
                return Err("native Some contract has an invalid payload".into());
            }
            summary.parameters[0].escape = Effect::May;
            summary.returned.aliases.insert(ReturnAlias {
                result_path: vec![crate::effects::Projection::VariantField {
                    variant: 0,
                    field: 0,
                }],
                parameter: 0,
                parameter_path: vec![],
            });
        }
        "option_value" => {
            if parameters.len() != 1 {
                return Err("native Option value contract requires one receiver".into());
            }
            let Some(crate::hir::Type::Enum(variants)) = types.get(&parameters[0]) else {
                return Err("native Option value contract requires an enum".into());
            };
            if variants
                .first()
                .is_none_or(|(name, fields)| name != "Some" || fields.as_slice() != [result])
            {
                return Err("native Option value contract has an invalid payload".into());
            }
            summary.returned.fresh = false;
            summary.returned.aliases.insert(ReturnAlias {
                result_path: vec![],
                parameter: 0,
                parameter_path: vec![crate::effects::Projection::VariantField {
                    variant: 0,
                    field: 0,
                }],
            });
            summary.parameters[0].escape = Effect::May;
            summary.may_fail = Effect::May;
        }
        "element" => {
            if parameters.len() != 2 || parameters[1] != "Int" {
                return Err("native element contract requires (container, Int)".into());
            }
            summary.returned.fresh = false;
            summary.returned.aliases.insert(ReturnAlias {
                result_path: vec![],
                parameter: 0,
                parameter_path: vec![crate::effects::Projection::Element],
            });
            summary.parameters[0].escape = Effect::May;
            summary.may_fail = Effect::May;
        }
        _ => return Err(format!("unknown native effect contract '{specification}'")),
    }
    summary.validate_signature(parameters, result, types, false)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        effects::{Projection, SummarySource},
        hir::Type,
    };
    fn types() -> BTreeMap<String, Type> {
        [
            ("Int".into(), Type::Primitive),
            ("Char".into(), Type::Primitive),
            ("Str".into(), Type::Primitive),
            ("Unit".into(), Type::Primitive),
            ("Vec<Int>".into(), Type::Vector("Int".into())),
        ]
        .into()
    }
    #[test]
    fn element_contract_returns_a_reference_without_retaining_it_elsewhere() {
        for (container, element) in [("Vec<Int>", "Int"), ("Str", "Char")] {
            let summary = contract(
                "element",
                &[container.into(), "Int".into()],
                element,
                &types(),
            )
            .unwrap();
            assert_eq!(summary.source, SummarySource::NativeContract);
            assert_eq!(summary.parameters[0].escape, Effect::May);
            assert_eq!(summary.parameters[0].retained, Effect::No);
            assert_eq!(summary.parameters[0].write, Effect::No);
            assert_eq!(
                summary
                    .returned
                    .aliases
                    .iter()
                    .next()
                    .unwrap()
                    .parameter_path,
                vec![Projection::Element]
            );
            assert_eq!(summary.may_fail, Effect::May);
        }
    }
    #[test]
    fn checked_reads_writes_and_io_have_distinct_effects() {
        let read = contract("checked_read_fresh", &["Vec<Int>".into()], "Int", &types()).unwrap();
        assert!(read.returned.fresh);
        assert_eq!(read.parameters[0].escape, Effect::No);
        assert_eq!(read.may_fail, Effect::May);
        assert_eq!(read.io, Effect::No);
        let write = contract(
            "checked_resize_first",
            &["Str".into(), "Char".into()],
            "Unit",
            &types(),
        )
        .unwrap();
        assert_eq!(write.parameters[0].write, Effect::May);
        assert_eq!(write.parameters[0].structural, Effect::May);
        assert_eq!(write.parameters[1].write, Effect::No);
        assert_eq!(write.parameters[1].retained, Effect::No);
        let input = contract("io_read_fresh", &[], "Int", &types()).unwrap();
        assert_eq!(input.io, Effect::May);
        assert_eq!(input.may_fail, Effect::May);
        assert_eq!(input.may_diverge, Effect::May);
        assert_eq!(input.external_write, Effect::No);
    }
    #[test]
    fn malformed_contracts_and_alias_types_are_rejected() {
        for (spec, args, result) in [
            ("unknown", vec![], "Int"),
            ("write_first", vec![], "Unit"),
            ("write_first", vec!["Int".into()], "Int"),
            ("io_write", vec!["Int".into()], "Int"),
            ("io_read_fresh", vec![], "Unit"),
            ("element", vec!["Int".into(), "Int".into()], "Int"),
            ("element", vec!["Str".into(), "Int".into()], "Int"),
        ] {
            assert!(contract(spec, &args, result, &types()).is_err(), "{spec}");
        }
    }
    #[test]
    fn vector_stores_retain_the_source_and_deep_operations_do_not_trust_user_helpers() {
        let mut types = types();
        let append = contract(
            "resize_retain_second",
            &["Vec<Int>".into(), "Int".into()],
            "Unit",
            &types,
        )
        .unwrap();
        assert_eq!(append.parameters[0].write, Effect::May);
        assert_eq!(append.parameters[1].write, Effect::No);
        assert_eq!(append.parameters[1].retained, Effect::May);
        let store = contract(
            "replace_element",
            &["Vec<Int>".into(), "Int".into(), "Int".into()],
            "Int",
            &types,
        )
        .unwrap();
        assert_eq!(store.parameters[0].structural, Effect::No);
        assert_eq!(store.returned.aliases.first().unwrap().parameter, 2);
        let copy = contract("deep_read_fresh", &["Vec<Int>".into()], "Vec<Int>", &types).unwrap();
        assert!(copy.returned.fresh);
        assert_eq!(copy.parameters[0].write, Effect::No);
        types.insert(
            "Custom".into(),
            Type::Record(vec![("x".into(), "Int".into())]),
        );
        types.insert("Vec<Custom>".into(), Type::Vector("Custom".into()));
        for spec in ["deep_read_fresh", "deep_resize_first", "deep_io_write"] {
            let opaque = contract(spec, &["Vec<Custom>".into()], "Vec<Custom>", &types).unwrap();
            assert_eq!(opaque.source, SummarySource::Opaque);
            assert_eq!(opaque.parameters[0].write, Effect::Unknown);
        }
        assert!(
            contract(
                "replace_element",
                &["Vec<Int>".into(), "Str".into(), "Int".into()],
                "Int",
                &types
            )
            .is_err()
        );
    }
    #[test]
    fn option_value_returns_a_checked_payload_alias_without_external_retention() {
        let mut types = types();
        types.insert(
            "Option<Int>".into(),
            Type::Enum(vec![
                ("Some".into(), vec!["Int".into()]),
                ("None".into(), vec![]),
            ]),
        );
        let summary = contract("option_value", &["Option<Int>".into()], "Int", &types).unwrap();
        assert_eq!(summary.parameters[0].escape, Effect::May);
        assert_eq!(summary.parameters[0].retained, Effect::No);
        assert_eq!(summary.parameters[0].write, Effect::No);
        assert_eq!(
            summary.returned.aliases.first().unwrap().parameter_path,
            vec![Projection::VariantField {
                variant: 0,
                field: 0
            }]
        );
        assert_eq!(summary.may_fail, Effect::May);
        let constructor = contract("option_some", &["Int".into()], "Option<Int>", &types).unwrap();
        assert!(constructor.returned.fresh);
        assert_eq!(constructor.parameters[0].escape, Effect::May);
        assert_eq!(constructor.parameters[0].retained, Effect::No);
        assert_eq!(
            constructor.returned.aliases.first().unwrap().result_path,
            vec![Projection::VariantField {
                variant: 0,
                field: 0
            }]
        );
        assert!(contract("option_some", &["Str".into()], "Option<Int>", &types).is_err());
        assert!(contract("option_value", &["Option<Int>".into()], "Str", &types).is_err());
        assert!(contract("option_value", &["Int".into()], "Int", &types).is_err());
    }
}
