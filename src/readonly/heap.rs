//! Allocation-site may-alias graph. Capabilities belong to references; only
//! references stored in members enter the graph. Weak updates preserve older
//! instances of an allocation site on loop backedges.
use super::{AccessPaths, Projection, ValueId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Object {
    site: ValueId,
    path: Vec<Projection>,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Reference {
    pub access: AccessPaths,
    objects: BTreeSet<Object>,
    pub unknown: bool,
}
impl Reference {
    pub fn diagnostic_cost(&self) -> usize {
        1 + self.objects.len() + self.access.diagnostic_cost()
    }
    pub fn fresh(site: ValueId) -> Self {
        Self {
            objects: [Object { site, path: vec![] }].into(),
            ..Self::default()
        }
    }
    pub fn unknown() -> Self {
        Self {
            unknown: true,
            ..Self::default()
        }
    }
    pub fn join(&mut self, other: &Self) {
        self.access.join(&other.access);
        self.objects.extend(other.objects.iter().cloned());
        self.unknown |= other.unknown;
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct Heap {
    members: BTreeMap<Object, BTreeMap<Projection, Reference>>,
    exposed: BTreeSet<Object>,
}
impl Heap {
    pub fn diagnostic_cost(&self) -> usize {
        1 + self.exposed.len()
            + self
                .members
                .values()
                .map(|fields| {
                    1 + fields
                        .values()
                        .map(Reference::diagnostic_cost)
                        .sum::<usize>()
                })
                .sum::<usize>()
    }
    pub fn join(&mut self, other: &Self) {
        for (object, members) in &other.members {
            for (projection, value) in members {
                self.members
                    .entry(object.clone())
                    .or_default()
                    .entry(projection.clone())
                    .or_default()
                    .join(value);
            }
        }
        self.exposed.extend(other.exposed.iter().cloned());
        self.expose_objects(self.exposed.iter().cloned().collect());
    }
    fn expose_objects(&mut self, mut pending: VecDeque<Object>) {
        let mut visited = BTreeSet::new();
        while let Some(object) = pending.pop_front() {
            if !visited.insert(object.clone()) {
                continue;
            }
            self.exposed.insert(object.clone());
            if let Some(members) = self.members.get(&object) {
                for value in members.values() {
                    pending.extend(value.objects.iter().cloned());
                }
            }
        }
    }
    pub fn expose(&mut self, value: &Reference) {
        self.expose_objects(value.objects.iter().cloned().collect());
    }
    pub fn read(&self, value: &Reference) -> AccessPaths {
        self.read_inner(value, &mut BTreeSet::new(), 0)
    }
    fn read_inner(
        &self,
        value: &Reference,
        active: &mut BTreeSet<Object>,
        depth: usize,
    ) -> AccessPaths {
        let mut result = value.access.clone();
        if depth >= 16 || value.objects.iter().any(|object| active.contains(object)) {
            // Widen to all stored restrictions rather than dropping a cycle or
            // an arbitrarily deep path. Ordinary nonrecursive types stay local.
            for members in self.members.values() {
                for value in members.values() {
                    result.join(&value.access);
                }
            }
            return result.collapse();
        }
        for object in &value.objects {
            active.insert(object.clone());
            if let Some(members) = self.members.get(object) {
                for (projection, field) in members {
                    result.join(
                        &self
                            .read_inner(field, active, depth + 1)
                            .contain(projection.clone()),
                    );
                }
            }
            active.remove(object);
        }
        result
    }
    pub fn project(&self, owner: &Reference, projection: Projection) -> Reference {
        let mut result = Reference {
            access: owner.access.project(projection.clone()),
            unknown: owner.unknown,
            ..Reference::default()
        };
        for object in &owner.objects {
            if let Some(field) = self
                .members
                .get(object)
                .and_then(|fields| fields.get(&projection))
            {
                result.join(field);
            } else {
                result.unknown = true;
            }
        }
        result
    }
    /// Diagnostic-only object reachability. This never adds capabilities or
    /// participates in the access proof. Unknown links have no concrete path.
    pub fn paths_to(
        &self,
        from: &Reference,
        target: &Reference,
        budget: &mut usize,
    ) -> Vec<Vec<Projection>> {
        let mut pending = VecDeque::from([(from.objects.clone(), vec![], BTreeSet::new())]);
        let mut paths = BTreeSet::new();
        while let Some((objects, path, active)) = pending.pop_front() {
            if *budget == 0 || paths.len() >= 64 {
                break;
            }
            *budget -= 1;
            if !objects.is_disjoint(&target.objects) {
                paths.insert(path.clone());
            }
            if path.len() >= 16 {
                continue;
            }
            for object in objects {
                if active.contains(&object) {
                    continue;
                }
                let mut active = active.clone();
                active.insert(object.clone());
                if let Some(fields) = self.members.get(&object) {
                    for (projection, value) in fields {
                        if *budget == 0 {
                            break;
                        }
                        *budget -= 1;
                        let mut path = path.clone();
                        path.push(projection.clone());
                        pending.push_back((value.objects.clone(), path, active.clone()));
                    }
                }
            }
        }
        paths.into_iter().collect()
    }
    /// True means a destination can have untracked aliases outside this graph.
    pub fn store(&mut self, owner: &Reference, projection: Projection, value: &Reference) -> bool {
        let uncertain = owner.unknown
            || owner.objects.is_empty()
            || owner
                .objects
                .iter()
                .any(|object| self.exposed.contains(object));
        for object in &owner.objects {
            self.members
                .entry(object.clone())
                .or_default()
                .entry(projection.clone())
                .or_default()
                .join(value);
        }
        if uncertain {
            self.expose(value);
        }
        uncertain
    }
    pub fn store_path(
        &mut self,
        owner: &Reference,
        path: &[Projection],
        value: &Reference,
    ) -> bool {
        if path.is_empty() || path.len() > 16 {
            return true;
        }
        let mut owner = owner.clone();
        let mut uncertain = false;
        for projection in &path[..path.len() - 1] {
            let mut next = Reference {
                unknown: owner.unknown,
                ..Reference::default()
            };
            for object in &owner.objects {
                let fields = self.members.entry(object.clone()).or_default();
                let field = fields.entry(projection.clone()).or_insert_with(|| {
                    let mut path = object.path.clone();
                    path.push(projection.clone());
                    Reference {
                        objects: [Object {
                            site: object.site.clone(),
                            path,
                        }]
                        .into(),
                        ..Reference::default()
                    }
                });
                next.join(field);
            }
            uncertain |= owner.unknown
                || owner
                    .objects
                    .iter()
                    .any(|object| self.exposed.contains(object));
            owner = next;
        }
        self.store(&owner, path.last().unwrap().clone(), value) || uncertain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn restricted() -> Reference {
        let span = crate::ast::Span {
            file: "heap-test".into(),
            line: 1,
            column: 1,
        };
        Reference {
            access: AccessPaths::root(crate::hir::AccessInfo {
                name: "r".into(),
                declaration: span.clone(),
                annotation: crate::ast::AccessAnnotation {
                    mode: crate::ast::AccessMode::Readonly,
                    span,
                },
            }),
            ..Reference::default()
        }
    }
    #[test]
    fn exposure_reaches_children_added_on_another_branch() {
        let parent = Reference::fresh(ValueId::Local(0));
        let child = Reference::fresh(ValueId::Local(1));
        let mut exposed = Heap::default();
        exposed.expose(&parent);
        let mut stored = Heap::default();
        assert!(!stored.store(&parent, Projection::Field(0), &child));
        exposed.join(&stored);
        assert!(exposed.store(&child, Projection::Field(0), &restricted()));
    }
    #[test]
    fn cycles_widen_without_losing_stored_restrictions() {
        let value = Reference::fresh(ValueId::Local(0));
        let mut heap = Heap::default();
        heap.store(&value, Projection::Field(0), &value);
        heap.store(&value, Projection::Field(1), &restricted());
        let access = heap.read(&value);
        assert!(!access.0.is_empty());
        assert!(access.1);
    }
}
