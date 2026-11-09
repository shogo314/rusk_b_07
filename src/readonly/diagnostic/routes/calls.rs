use super::*;
use crate::readonly::Reference;

impl Context<'_> {
    fn seed(&self, body: &lir::Function, arguments: &[Paths]) -> (State, Routes) {
        let mut state = State::default();
        let mut routes = Routes::new(body);
        for (index, paths) in arguments.iter().enumerate() {
            if paths.0.is_empty() {
                continue;
            }
            let id = ValueId::Parameter(index);
            let mut reference = Reference::unknown();
            reference.access = paths.access(self.source);
            state.values.insert(id.clone(), reference);
            routes.values.insert(id, paths.clone());
        }
        (state, routes)
    }
    pub(super) fn returns(&mut self, function: usize, arguments: &[Paths]) -> Option<Found> {
        let program = self.program;
        if arguments.iter().all(|paths| paths.0.is_empty())
            && !has(&self.summaries[function].returned, self.source)
            && !has(&self.summaries[function].retained, self.source)
        {
            return None;
        }
        let body = program.functions[function].body.as_ref()?;
        if self.active.len() >= 16 || !self.active.insert(function) {
            return None;
        }
        let mut arguments = arguments.to_vec();
        for (index, paths) in arguments.iter_mut().enumerate() {
            paths.step(
                None,
                format!(
                    "readonly access enters parameter {} of '{}'",
                    index + 1,
                    program.functions[function].name
                ),
            );
        }
        let (state, routes) = self.seed(body, &arguments);
        let found = self.run(body, state, routes, Goal::Returns);
        self.active.remove(&function);
        found
    }
    pub(super) fn mutation(
        &mut self,
        call: &crate::hir::Call,
        index: usize,
        paths: Paths,
    ) -> Trail {
        let program = self.program;
        let callee = &program.functions[call.function];
        let mut fallback = paths.best().unwrap_or_else(|| Trail::origin(self.source));
        fallback.push(
            Some(call.span.clone()),
            format!(
                "argument {} of '{}' may modify this access",
                index + 1,
                callee.name
            ),
        );
        let effect = &callee.effects.parameters[index];
        fallback.uncertain |=
            effect.write == Effect::Unknown || effect.structural == Effect::Unknown;
        let Some(body) = &callee.body else {
            return fallback;
        };
        if self.active.len() >= 16 || !self.active.insert(call.function) {
            fallback.truncated = true;
            return fallback;
        }
        let mut arguments = vec![Paths::default(); call.args.len()];
        arguments[index] = paths;
        let (state, mut routes) = self.seed(body, &arguments);
        if let Some(paths) = routes.values.get_mut(&ValueId::Parameter(index)) {
            paths.step(
                Some(call.span.clone()),
                format!(
                    "readonly argument enters parameter {} of '{}'",
                    index + 1,
                    callee.name
                ),
            );
        }
        let found = self.run(body, state, routes, Goal::Write);
        let result = if let Some(found) = found {
            let (block, position) = found.site.unwrap();
            let op = &body.blocks[block].instructions[position];
            let mut trail = found.paths.best().unwrap_or(fallback.clone());
            if let Operation::Call(nested) = op {
                if let Some(arg) = nested
                    .args
                    .iter()
                    .enumerate()
                    .find(|(i, v)| {
                        let e = &program.functions[nested.function].effects.parameters[*i];
                        found.value.as_ref().is_some_and(|target| target.id == v.id)
                            && (e.write != Effect::No || e.structural != Effect::No)
                    })
                    .map(|(i, _)| i)
                {
                    trail = self.mutation(nested, arg, found.paths);
                }
            } else if let Operation::Rebind { span, .. } = op {
                trail.push(
                    span.clone(),
                    "readonly access reaches field or tuple element rebinding".into(),
                );
            }
            trail
        } else {
            fallback.uncertain = true;
            fallback.push(
                None,
                "precise operation for this contained readonly access is not reconstructed".into(),
            );
            fallback
        };
        self.active.remove(&call.function);
        result
    }
}
