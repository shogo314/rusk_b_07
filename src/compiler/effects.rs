use super::*;

impl Compiler {
    pub(super) fn declared_native_effects(
        &self,
        name: &Name,
        parameters: &[String],
        result: &str,
        properties: &BTreeMap<String, String>,
    ) -> Result<Option<crate::effects::Summary>, String> {
        let Some(specification) = properties.get("effects") else {
            return Ok(None);
        };
        if !matches!(
            name.span.file.as_str(),
            "<stdlib/core.rusk>" | "<stdlib/vec.rusk>"
        ) {
            return Err(name
                .span
                .error("native effect contracts are reserved for the embedded standard library"));
        }
        crate::native_effects::contract(specification, parameters, result, &self.ir_types())
            .map(Some)
            .map_err(|error| name.span.error(error))
    }
}
