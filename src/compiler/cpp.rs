//! C++ runtime provider. Rusk HIR/LIR bodies never contain these native sources.
use super::*;
impl Compiler {
    pub(super) fn cpp_context(&self) -> crate::backend::Context {
        let functions = self
            .functions
            .iter()
            .map(|f| crate::backend::FunctionInfo {
                cpp: f.cpp.clone(),
                name: f.name.text.clone(),
                result: f.result.clone(),
                trusted: f.body.is_none()
                    && matches!(
                        f.name.span.file.as_str(),
                        "<stdlib/core.rusk>" | "<stdlib/vec.rusk>"
                    ),
            })
            .collect();
        let mut prelude = String::new();
        for code in self.symbols.values() {
            prelude += code;
            prelude += "\n";
        }
        prelude += &self.enum_forwards();
        prelude += &self.container_cpp();
        prelude += &self.enum_cpp();
        prelude += &self.lazy_cpp();
        prelude += &self.record_cpp();
        prelude += &self.key_hash_cpp();
        prelude += &self.container_declarations();
        prelude += &self.lazy_copy_declarations();
        let foreign_definitions = self
            .used
            .iter()
            .filter(|&&id| self.functions[id].checked.is_none())
            .map(|&id| self.functions[id].code.clone())
            .collect();
        let epilogue = self.enum_copy_cpp()
            + &self.record_copy_cpp()
            + &self.container_copy_cpp()
            + &self.lazy_copy_cpp();
        crate::backend::Context {
            types: self.types.clone(),
            functions,
            prelude,
            foreign_definitions,
            epilogue,
        }
    }
}
