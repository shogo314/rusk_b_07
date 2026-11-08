use super::*;

fn specialize_row(
    row: &[TypedPattern],
    constructor: Option<&Constructor>,
    fields: &[String],
    out: &mut Vec<Vec<TypedPattern>>,
) {
    let first = &row[0];
    match &first.kind {
        Kind::Or(alternatives) => {
            for alternative in alternatives {
                let mut row = row.to_vec();
                row[0] = alternative.clone();
                specialize_row(&row, constructor, fields, out);
            }
        }
        Kind::Wild(_) => {
            let mut next = fields
                .iter()
                .map(|ty| TypedPattern::wild(ty, &first.span))
                .collect::<Vec<_>>();
            next.extend_from_slice(&row[1..]);
            out.push(next);
        }
        Kind::Constructor(c, children) if constructor == Some(c) => {
            let mut next = children.clone();
            next.extend_from_slice(&row[1..]);
            out.push(next);
        }
        _ => {}
    }
}
impl Compiler {
    pub(super) fn pattern_witness(
        &self,
        rows: &[Vec<TypedPattern>],
        candidate: &[TypedPattern],
    ) -> Result<Option<Vec<String>>, String> {
        self.useful_pattern(rows, candidate, &mut 100_000)
    }
    fn useful_pattern(
        &self,
        rows: &[Vec<TypedPattern>],
        candidate: &[TypedPattern],
        budget: &mut usize,
    ) -> Result<Option<Vec<String>>, String> {
        if candidate.is_empty() {
            return Ok(rows.is_empty().then(Vec::new));
        }
        if *budget == 0 {
            return Err(candidate[0].span.error("pattern analysis limit exceeded"));
        }
        *budget -= 1;
        if rows
            .iter()
            .any(|r| r.iter().all(|p| matches!(p.kind, Kind::Wild(_))))
        {
            return Ok(None);
        }
        let first = &candidate[0];
        if let Kind::Or(alternatives) = &first.kind {
            for a in alternatives {
                let mut next = vec![a.clone()];
                next.extend_from_slice(&candidate[1..]);
                if let Some(witness) = self.useful_pattern(rows, &next, budget)? {
                    return Ok(Some(witness));
                }
            }
            return Ok(None);
        }
        let constructors = match &first.kind {
            Kind::Constructor(c, _) => Some(vec![c.clone()]),
            _ => self.constructors(&first.ty),
        };
        if let Some(constructors) = constructors {
            for constructor in constructors {
                let fields = self.constructor_fields(&first.ty, &constructor);
                let mut specialized = vec![];
                for row in rows {
                    specialize_row(row, Some(&constructor), &fields, &mut specialized);
                }
                let mut next = if let Kind::Constructor(_, children) = &first.kind {
                    children.clone()
                } else {
                    fields
                        .iter()
                        .map(|ty| TypedPattern::wild(ty, &first.span))
                        .collect()
                };
                next.extend_from_slice(&candidate[1..]);
                if let Some(witness) = self.useful_pattern(&specialized, &next, budget)? {
                    let (children, tail) = witness.split_at(fields.len());
                    let mut result =
                        vec![self.format_constructor(&first.ty, &constructor, children)];
                    result.extend_from_slice(tail);
                    return Ok(Some(result));
                }
            }
            Ok(None)
        } else {
            let mut defaults = vec![];
            for row in rows {
                specialize_row(row, None, &[], &mut defaults);
            }
            let Some(tail) = self.useful_pattern(&defaults, &candidate[1..], budget)? else {
                return Ok(None);
            };
            let mut seen = BTreeSet::new();
            fn collect(p: &TypedPattern, out: &mut BTreeSet<Atom>) {
                match &p.kind {
                    Kind::Constructor(Constructor::Literal(a), _) => {
                        out.insert(a.clone());
                    }
                    Kind::Or(items) => {
                        for p in items {
                            collect(p, out);
                        }
                    }
                    _ => {}
                }
            }
            for row in rows {
                collect(&row[0], &mut seen);
            }
            let head = match first.ty.as_str() {
                "Int" => (0..=seen.len() as i64)
                    .map(Atom::Int)
                    .find(|a| !seen.contains(a))
                    .unwrap()
                    .text(),
                "Str" => (0..=seen.len())
                    .map(|n| Atom::Str("_".repeat(n)))
                    .find(|a| !seen.contains(a))
                    .unwrap()
                    .text(),
                _ => "_".into(),
            };
            let mut result = vec![head];
            result.extend(tail);
            Ok(Some(result))
        }
    }
}
