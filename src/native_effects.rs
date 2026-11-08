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
}
