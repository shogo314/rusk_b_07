use super::*;
impl Compiler {
    pub(super) fn declare_annotated(
        &mut self,
        name: &DeclaredName,
        ty: String,
        value: Option<Value>,
    ) -> Result<(), String> {
        if let Some(annotation) = &name.escape {
            let Some(Value {
                id: ValueId::Parameter(parameter),
                ..
            }) = &value
            else {
                return Err(annotation
                    .span
                    .error("escape modifiers require a parameter identity"));
            };
            self.body.push(Instruction::AnnotateEscape {
                parameter: *parameter,
                info: crate::hir::EscapeInfo {
                    name: name.text.clone(),
                    declaration: name.span.clone(),
                    annotation: annotation.clone(),
                },
            });
        }
        self.declare(&name.name, ty, value)?;
        if let Some(annotation) = &name.binding {
            let info = crate::hir::BindingInfo {
                name: name.text.clone(),
                declaration: name.span.clone(),
                annotation: annotation.clone(),
            };
            let variable = self.scopes.last_mut().unwrap().get_mut(&name.text).unwrap();
            variable.binding = Some(info.clone());
            self.body.push(Instruction::AnnotateBinding {
                value: Value {
                    id: variable.id.clone(),
                    ty: variable.ty.clone(),
                },
                info,
            });
        }
        if let Some(annotation) = &name.access {
            let info = crate::hir::AccessInfo {
                name: name.text.clone(),
                declaration: name.span.clone(),
                annotation: annotation.clone(),
            };
            let variable = self.scopes.last_mut().unwrap().get_mut(&name.text).unwrap();
            variable.access = Some(info.clone());
            self.body.push(Instruction::AnnotateAccess {
                value: Value {
                    id: variable.id.clone(),
                    ty: variable.ty.clone(),
                },
                info,
            });
        }
        Ok(())
    }
    pub(super) fn check_fixed_rebind(
        &self,
        variable: &Variable,
        span: &Span,
    ) -> Result<(), String> {
        if variable.initialized
            && let Some(info) = &variable.binding
            && info.annotation.mode == BindingMode::Fixed
        {
            return Err(crate::bindings::rebinding_error(info, span));
        }
        Ok(())
    }
}
