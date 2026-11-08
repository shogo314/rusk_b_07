use super::*;
impl Compiler {
    pub(super) fn ir_types(&self) -> BTreeMap<String, crate::hir::Type> {
        use crate::hir::Type;
        self.types
            .keys()
            .map(|name| {
                let fields = || {
                    self.records[name]
                        .fields
                        .iter()
                        .map(|(n, t)| (n.text.clone(), t.text.clone()))
                        .collect()
                };
                let kind = if let Some(c) = self.callables.get(name) {
                    Type::Callable {
                        fields: fields(),
                        function: c.function,
                        params: c.parameters.clone(),
                        result: c.result.clone(),
                    }
                } else if let Some(e) = self.enums.get(name) {
                    Type::Enum(
                        e.variants
                            .iter()
                            .map(|(n, f)| {
                                (n.text.clone(), f.iter().map(|t| t.text.clone()).collect())
                            })
                            .collect(),
                    )
                } else if self.records.contains_key(name) {
                    Type::Record(fields())
                } else if let Some(element) = vector_element(name) {
                    Type::Vector(element.into())
                } else if let Some(c) = self.containers.get(name) {
                    match &c.shape {
                        containers::Shape::Dict => Type::Dict {
                            key: c.arguments[0].clone(),
                            value: c.arguments[1].clone(),
                        },
                        containers::Shape::Set => Type::Set(c.arguments[0].clone()),
                        containers::Shape::Iterator { element, .. } => Type::Iterator {
                            element: element.clone(),
                        },
                    }
                } else if let Some(it) = self.iterator_types.get(name) {
                    Type::Iterator {
                        element: it.element.clone(),
                    }
                } else if let Some(("Iter", args)) = tuples::type_parts(name)
                    && let Some(element) = args.first().and_then(|arg| vector_element(arg))
                {
                    Type::Iterator {
                        element: element.into(),
                    }
                } else if name == "RangeIter" {
                    Type::Iterator {
                        element: "Int".into(),
                    }
                } else if matches!(name.as_str(), "Int" | "Bool" | "Char" | "Str" | "Unit") {
                    Type::Primitive
                } else {
                    Type::Opaque
                };
                (name.clone(), kind)
            })
            .collect()
    }
}
