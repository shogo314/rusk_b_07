use super::tuples::type_parts;
use super::*;

#[derive(Clone)]
pub(super) enum Shape {
    Dict,
    Set,
    Iterator { owner: String, element: String },
}
#[derive(Clone)]
pub(super) struct Container {
    pub shape: Shape,
    pub arguments: Vec<String>,
    pub native: String,
}

impl Compiler {
    pub(super) fn container_argument_hint(
        &self,
        name: &str,
        count: usize,
        values: &[Value],
        expected: Option<&str>,
    ) -> Option<String> {
        let (base, args) = type_parts(name).unwrap_or((name, vec![]));
        if base == "Some" && count == 1 && values.is_empty() {
            return if args.len() == 1 {
                Some(args[0].into())
            } else {
                expected.and_then(|ty| self.option_element(ty))
            };
        }
        let first = values.first()?;
        let c = self.containers.get(&first.ty)?;
        if matches!(c.shape, Shape::Dict | Shape::Set) {
            if values.len() == 1
                && matches!(
                    name,
                    "get" | "set" | "insert" | "contains" | "erase" | "__getitem__" | "__setitem__"
                )
            {
                return Some(c.arguments[0].clone());
            }
            if values.len() == 2
                && matches!(c.shape, Shape::Dict)
                && matches!(name, "set" | "__setitem__")
            {
                return Some(c.arguments[1].clone());
            }
        }
        None
    }
    pub(super) fn dictionary_types(&self, ty: &str) -> Option<(String, String)> {
        let c = self.containers.get(ty)?;
        matches!(c.shape, Shape::Dict).then(|| (c.arguments[0].clone(), c.arguments[1].clone()))
    }
    fn key_type(&self, ty: &str) -> bool {
        matches!(ty, "Int" | "Bool" | "Char" | "Str")
            || (ty.starts_with("Tuple<")
                && self
                    .records
                    .get(ty)
                    .is_some_and(|r| r.fields.iter().all(|(_, t)| self.key_type(&t.text))))
    }
    pub(super) fn collection_element(&self, ty: &str) -> Option<String> {
        let c = self.containers.get(ty)?;
        match &c.shape {
            Shape::Dict => Some(format!("Tuple<{},{}>", c.arguments[0], c.arguments[1])),
            Shape::Set => Some(c.arguments[0].clone()),
            Shape::Iterator { element, .. } => Some(element.clone()),
        }
    }
    pub(super) fn specialize_container(&mut self, name: &Name) -> Result<bool, String> {
        let Some((base, args)) = type_parts(&name.text) else {
            return Ok(false);
        };
        if !matches!(base, "Dict" | "Set") {
            return Ok(false);
        }
        let count = if base == "Dict" { 2 } else { 1 };
        if args.len() != count {
            return Err(name
                .span
                .error(format!("{base} expects {count} type arguments")));
        }
        let arguments = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for arg in &arguments {
            self.check_type(&Name {
                text: arg.clone(),
                span: name.span.clone(),
            })?;
        }
        if !self.key_type(&arguments[0]) {
            return Err(name
                .span
                .error("hash keys require basic types or tuples of basic types"));
        }
        if self.containers.len() >= 512 {
            return Err(name.span.error("container specialization limit exceeded"));
        }
        let native = format!("RuskContainer{}", self.containers.len());
        let shape = match base {
            "Dict" => Shape::Dict,
            _ => Shape::Set,
        };
        self.types.insert(name.text.clone(), native.clone());
        self.containers.insert(
            name.text.clone(),
            Container {
                shape: shape.clone(),
                arguments,
                native: native.clone(),
            },
        );
        let ty = &name.text;
        self.native_function(
            "copy",
            vec![ty.clone()],
            ty,
            |cpp| {
                format!("Obj<{native}> {cpp}(Obj<{native}> x,Loc loc){{return deep_copy(x,loc);}}")
            },
            &name.span,
        )?;
        let assign = "x->entries=std::move(snapshot->entries);++x->version;";
        self.native_function("__assign__",vec![ty.clone(),ty.clone()],"Unit",|cpp|format!("void {cpp}(Obj<{native}> x,Obj<{native}> y,Loc loc){{auto snapshot=deep_copy(y,loc);{assign}}}"),&name.span)?;
        if matches!(shape, Shape::Dict | Shape::Set) {
            self.register_collection_iterator(ty, &name.span)?;
        }
        Ok(true)
    }
    fn register_collection_iterator(&mut self, owner: &str, span: &Span) -> Result<(), String> {
        let element = self.collection_element(owner).unwrap();
        self.check_type(&Name {
            text: element.clone(),
            span: span.clone(),
        })?;
        let c = self.containers[owner].clone();
        let source = c.native.clone();
        let ty = format!("__RuskCollectionIterator{}", self.containers.len());
        let native = format!("RuskContainer{}", self.containers.len());
        self.types.insert(ty.clone(), native.clone());
        self.containers.insert(
            ty.clone(),
            Container {
                shape: Shape::Iterator {
                    owner: owner.into(),
                    element: element.clone(),
                },
                arguments: vec![],
                native: native.clone(),
            },
        );
        self.native_function("__iter__",vec![owner.into()],&ty,|cpp|format!("Obj<{native}> {cpp}(Obj<{source}> owner,Loc){{return make_obj<{native}>({native}(owner));}}"),span)?;
        self.native_function(
            "__iter__",
            vec![ty.clone()],
            &ty,
            |cpp| format!("Obj<{native}> {cpp}(Obj<{native}> it,Loc){{return it;}}"),
            span,
        )?;
        let has=self.native_function("__iter_has_next__",vec![ty.clone()],"Bool",|cpp|format!("Obj<bool> {cpp}(Obj<{native}> it,Loc loc){{if(it->finished)return make_obj<bool>(false);if(it->version!=it->owner->version)fail(loc,\"container changed during iteration\");if(it->position==it->end){{it->finished=true;return make_obj<bool>(false);}}return make_obj<bool>(true);}}"),span)?;
        let has_cpp = self.functions[has].cpp.clone();
        self.used.insert(has);
        let key_copy = self.copy_function(&c.arguments[0], span)?;
        let result_native = self.types[&element].clone();
        let take = if matches!(c.shape, Shape::Dict) {
            format!(
                "auto key={key_copy}(it->position->first,loc);auto value=it->position->second;++it->position;return make_obj<{result_native}>({result_native}{{key,value}});"
            )
        } else {
            format!("auto value={key_copy}(*it->position,loc);++it->position;return value;")
        };
        self.native_function("__iter_next__",vec![ty.clone()],&element,|cpp|format!("Obj<{result_native}> {cpp}(Obj<{native}> it,Loc loc){{if(!*{has_cpp}(it,loc))fail(loc,\"iterator exhausted\");{take}}}"),span)?;
        self.native_function(
            "copy",
            vec![ty.clone()],
            &ty,
            |cpp| {
                format!(
                    "Obj<{native}> {cpp}(Obj<{native}> it,Loc loc){{return deep_copy(it,loc);}}"
                )
            },
            span,
        )?;
        Ok(())
    }
    pub(super) fn cached_native(
        &mut self,
        name: &str,
        params: Vec<String>,
        result: &str,
        code: impl FnOnce(&str) -> String,
        span: &Span,
    ) -> Result<usize, String> {
        let key = format!("{name}|{}|{result}", params.join(","));
        if let Some(&index) = self.container_functions.get(&key) {
            return Ok(index);
        }
        let index = self.native_function(name, params, result, code, span)?;
        self.container_functions.insert(key, index);
        Ok(index)
    }
    fn default_function(&mut self, ty: &str, span: &Span) -> Result<String, String> {
        let name = if vector_element(ty).is_some() {
            "__vec_new__"
        } else {
            ty
        };
        let index = if let Some(i) = self.resolve(name, &[], Some(ty), span)? {
            i
        } else if let Some(i) = self.container_constructor(name, &[], Some(ty), span)? {
            i
        } else {
            return Err(span.error(format!("type '{ty}' has no zero-argument constructor")));
        };
        self.same(ty, &self.functions[index].result, span)?;
        self.used.insert(index);
        Ok(self.functions[index].cpp.clone())
    }
    pub(super) fn container_constructor(
        &mut self,
        name: &str,
        args: &[Value],
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Option<usize>, String> {
        let (base, explicit) = type_parts(name).map_or((name, vec![]), |(n, a)| {
            (n, a.iter().map(|s| s.to_string()).collect())
        });
        if !matches!(
            base,
            "Some" | "None" | "Option" | "Dict" | "Set" | "try_read"
        ) {
            return Ok(None);
        }
        let result = if base == "Some" {
            if args.len() != 1 {
                return Err(span.error("Some requires one value"));
            }
            if !explicit.is_empty() && explicit != [args[0].ty.clone()] {
                return Err(span.error("Some type argument mismatch"));
            }
            format!("Option<{}>", args[0].ty)
        } else if !explicit.is_empty() {
            let constructor = if matches!(base, "None" | "try_read") {
                "Option"
            } else {
                base
            };
            format!("{constructor}<{}>", explicit.join(","))
        } else {
            let ty = expected.ok_or_else(|| {
                span.error(format!(
                    "{base} requires type arguments or a type annotation"
                ))
            })?;
            let wanted = if matches!(base, "None" | "try_read") {
                "Option"
            } else {
                base
            };
            if type_parts(ty).is_none_or(|(b, _)| b != wanted) {
                return Err(span.error(format!("{base} requires a {wanted} type context")));
            }
            ty.into()
        };
        if base != "Some" && !args.is_empty() {
            return Err(span.error("constructor requires no arguments"));
        }
        self.check_type(&Name {
            text: result.clone(),
            span: span.clone(),
        })?;
        let native = self.types[&result].clone();
        let params = args.iter().map(|v| v.ty.clone()).collect::<Vec<_>>();
        let code = if base == "Some" {
            let value = &self.types[&args[0].ty];
            format!(
                "Obj<{native}> $fn(Obj<{value}> value,Loc){{return rusk_enum_make<{native},0>(value);}}"
            )
        } else if base == "try_read" {
            let element = self.option_element(&result).unwrap();
            if !matches!(element.as_str(), "Int" | "Bool" | "Char" | "Str") {
                return Err(span.error("try_read currently supports basic input types"));
            }
            let read = self.read_function(&element, 0, span)?;
            self.used.insert(read);
            let read = &self.functions[read].cpp;
            format!(
                "Obj<{native}> $fn(Loc loc){{while(ascii_space(rusk_peek()))rusk_get();if(rusk_peek()==std::char_traits<char>::eof())return rusk_enum_make<{native},1>();auto value={read}(loc);return rusk_enum_make<{native},0>(value);}}"
            )
        } else if self.option_element(&result).is_some() {
            format!("Obj<{native}> $fn(Loc){{return rusk_enum_make<{native},1>();}}")
        } else {
            format!("Obj<{native}> $fn(Loc){{return make_obj<{native}>({native}{{}});}}")
        };
        let function = self.cached_native(
            &format!("$constructor_{base}"),
            params,
            &result,
            |cpp| code.replace("$fn", cpp),
            span,
        )?;
        let contract = crate::native_effects::contract(
            match base {
                "Some" => "option_some",
                "try_read" => "io_read_fresh",
                _ => "read_fresh",
            },
            &self.functions[function].params,
            &result,
            &self.ir_types(),
        )
        .map_err(|error| span.error(error))?;
        self.native_contracts.insert(function, contract);
        Ok(Some(function))
    }
    pub(super) fn collection_operation(
        &mut self,
        name: &str,
        args: &[Value],
        expected: Option<&str>,
        span: &Span,
    ) -> Result<Option<usize>, String> {
        if name == "__read__"
            && let Some(ty) = expected
        {
            return self.read_function(ty, args.len(), span).map(Some);
        }
        let Some(first) = args.first() else {
            return Ok(None);
        };
        if let Some(index) = self.option_operation(name, args, span)? {
            return Ok(Some(index));
        }
        let Some(c) = self.containers.get(&first.ty).cloned() else {
            return Ok(None);
        };
        let types = args.iter().map(|v| v.ty.clone()).collect::<Vec<_>>();
        let native = c.native.clone();
        if !matches!(c.shape, Shape::Dict | Shape::Set) {
            return Ok(None);
        }
        let key = c.arguments[0].clone();
        let key_native = self.types[&key].clone();
        let is_dict = matches!(c.shape, Shape::Dict);
        let value = if is_dict {
            c.arguments[1].clone()
        } else {
            key.clone()
        };
        let value_native = self.types[&value].clone();
        let (result, params, body) = match name {
            "len" if args.len() == 1 => (
                "Int".into(),
                format!("Obj<{native}> x"),
                "return make_obj<std::int64_t>(checked_int(x->entries.size(),loc));".into(),
            ),
            "is_empty" | "__bool__" | "__not__" if args.len() == 1 => (
                "Bool".into(),
                format!("Obj<{native}> x"),
                format!(
                    "return make_obj<bool>({}x->entries.empty());",
                    if name == "__bool__" { "!" } else { "" }
                ),
            ),
            "clear" if args.len() == 1 => (
                "Unit".into(),
                format!("Obj<{native}> x"),
                "if(!x->entries.empty()){x->entries.clear();++x->version;}".into(),
            ),
            "contains" | "erase" | "get" | "insert" | "set" | "__getitem__" | "__setitem__" => {
                if args.len() < 2 {
                    return Err(span.error("missing key"));
                }
                self.same(&key, &args[1].ty, span)?;
                let mut params = format!("Obj<{native}> x,Obj<{key_native}> key");
                let body=match name{
                    "contains" if args.len()==2=> ("Bool".into(),"return make_obj<bool>(x->entries.find(key)!=x->entries.end());".into()),
                    "erase" if args.len()==2=>("Bool".into(),"auto count=x->entries.erase(key);if(count)++x->version;return make_obj<bool>(count!=0);".into()),
                    "get" if args.len()==2=>{
                        let result=format!("Option<{value}>");self.check_type(&Name{text:result.clone(),span:span.clone()})?;
                        let found=if is_dict{"it->second".into()}else{let copy=self.copy_function(&key,span)?;format!("{copy}(*it,loc)")};
                        let option=self.types[&result].clone();
                        (result,format!("auto it=x->entries.find(key);if(it==x->entries.end())return rusk_enum_make<{option},1>();auto value={found};return rusk_enum_make<{option},0>(value);"))
                    }
                    "insert" if !is_dict && args.len()==2=>{let copy=self.copy_function(&key,span)?;("Bool".into(),format!("if(x->entries.find(key)!=x->entries.end())return make_obj<bool>(false);auto copied={copy}(key,loc);x->entries.insert(copied);++x->version;return make_obj<bool>(true);"))}
                    "set"|"__setitem__" if is_dict && args.len()==3=>{
                        self.same(&value,&args[2].ty,span)?;params.push_str(&format!(",Obj<{value_native}> value"));let copy=self.copy_function(&key,span)?;
                        let tail=if name=="set"{""}else{"return value;"};
                        (if name=="set"{"Unit".into()}else{value.clone()},format!("auto it=x->entries.find(key);if(it!=x->entries.end())it->second=value;else{{auto copied={copy}(key,loc);x->entries.emplace(copied,value);++x->version;}}{tail}"))
                    }
                    "__getitem__" if is_dict && args.len()==2=>{
                        let default=self.default_function(&value,span)?;let copy=self.copy_function(&key,span)?;
                        (value.clone(),format!("auto it=x->entries.find(key);if(it!=x->entries.end())return it->second;auto copied={copy}(key,loc);auto value={default}(loc);auto result=x->entries.emplace(copied,value);if(result.second)++x->version;return result.first->second;"))
                    }
                    _=>return Err(span.error("invalid dictionary/set operation or argument count")),
                };
                (body.0, params, body.1)
            }
            "keys" | "values" if args.len() == 1 => {
                let element = if name == "keys" || !is_dict {
                    key.clone()
                } else {
                    value.clone()
                };
                let result = format!("Vec<{element}>");
                self.check_type(&Name {
                    text: result.clone(),
                    span: span.clone(),
                })?;
                let out_native = self.types[&result].clone();
                let get = if name == "keys" || !is_dict {
                    let copy = self.copy_function(&key, span)?;
                    format!(
                        "{copy}({},loc)",
                        if is_dict { "entry.first" } else { "entry" }
                    )
                } else {
                    "entry.second".into()
                };
                (
                    result,
                    format!("Obj<{native}> x"),
                    format!(
                        "auto result=make_obj<{out_native}>({out_native}{{}});result->reserve(x->entries.size());for(const auto& entry:x->entries)result->push_back({get});return result;"
                    ),
                )
            }
            _ => return Ok(None),
        };
        let return_native = if result == "Unit" {
            "void".into()
        } else {
            format!("Obj<{}>", self.types[&result])
        };
        self.cached_native(
            name,
            types,
            &result,
            |cpp| format!("{return_native} {cpp}({params},[[maybe_unused]] Loc loc){{{body}}}"),
            span,
        )
        .map(Some)
    }

    pub(super) fn read_function(
        &mut self,
        ty: &str,
        dimensions: usize,
        span: &Span,
    ) -> Result<usize, String> {
        let args = (0..dimensions)
            .map(|_| Value {
                ty: "Int".into(),
                id: ValueId::Probe,
            })
            .collect::<Vec<_>>();
        if let Some(index) = self.resolve("__read__", &args, Some(ty), span)? {
            return Ok(index);
        }
        self.check_type(&Name {
            text: ty.into(),
            span: span.clone(),
        })?;
        let native = self.types[ty].clone();
        let code = if let Some(element) = vector_element(ty).map(str::to_string) {
            if dimensions == 0 {
                return Err(span.error("array input requires one length per array dimension"));
            }
            let child = self.read_function(&element, dimensions - 1, span)?;
            self.used.insert(child);
            let read = &self.functions[child].cpp;
            let params = (0..dimensions)
                .map(|i| format!("Obj<std::int64_t> d{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let validate = (0..dimensions)
                .map(|i| format!("if(*d{i}<0)fail(loc,\"negative array length\");"))
                .collect::<String>();
            let child_args = (1..dimensions)
                .map(|i| format!("d{i},"))
                .collect::<String>();
            format!(
                "Obj<{native}> $fn({params},Loc loc){{{validate}auto n=*d0;if(n<0)fail(loc,\"negative array length\");auto result=make_obj<{native}>({native}{{}});if(static_cast<__int128>(n)>result->max_size())fail(loc,\"array length out of range\");result->reserve(static_cast<std::size_t>(n));for(std::int64_t i=0;i<n;++i)result->push_back({read}({child_args}loc));return result;}}"
            )
        } else if ty.starts_with("Tuple<") && dimensions == 0 {
            let fields = self.records[ty].fields.clone();
            let mut body = String::new();
            let mut values = vec![];
            for (index, (_, field)) in fields.iter().enumerate() {
                let read = self.read_function(&field.text, 0, span)?;
                self.used.insert(read);
                body += &format!("auto value{index}={}(loc);", self.functions[read].cpp);
                values.push(format!("value{index}"));
            }
            format!(
                "Obj<{native}> $fn(Loc loc){{{body}return make_obj<{native}>({native}{{{}}});}}",
                values.join(",")
            )
        } else {
            return Err(span.error(format!(
                "no input reader for '{ty}' with {dimensions} dimensions"
            )));
        };
        self.cached_native(
            "__read__",
            vec!["Int".into(); dimensions],
            ty,
            |cpp| code.replace("$fn", cpp),
            span,
        )
    }

    pub(super) fn prepare_container_copies(&mut self) -> Result<(), String> {
        let todo = self
            .containers
            .iter()
            .filter(|(ty, _)| !self.container_copies.contains_key(*ty))
            .map(|(ty, c)| (ty.clone(), c.clone()))
            .collect::<Vec<_>>();
        for (ty, c) in todo {
            let mut copies = vec![];
            for argument in &c.arguments {
                let span = Span {
                    file: "<stdlib/containers>".into(),
                    line: 1,
                    column: 1,
                };
                let f = self
                    .resolve(
                        "copy",
                        &[Value {
                            ty: argument.clone(),
                            id: ValueId::Probe,
                        }],
                        None,
                        &span,
                    )?
                    .ok_or_else(|| {
                        span.error(format!("container element '{argument}' requires copy"))
                    })?;
                self.same(argument, &self.functions[f].result, &span)?;
                self.used.insert(f);
                copies.push(f);
            }
            self.container_copies.insert(ty, copies);
        }
        Ok(())
    }
    pub(super) fn container_cpp(&self) -> String {
        let mut code = String::new();
        for native in self.types.values().filter(|n| n.starts_with("RuskRecord")) {
            code += &format!("struct {native};\n");
        }
        for c in self.containers.values() {
            code += &format!("struct {};\n", c.native);
        }
        for c in self.containers.values() {
            let native = &c.native;
            let args = c
                .arguments
                .iter()
                .map(|t| self.types[t].clone())
                .collect::<Vec<_>>()
                .join(",");
            let base = match c.shape {
                Shape::Dict => "RuskDict",
                Shape::Set => "RuskSet",
                Shape::Iterator { .. } => continue,
            };
            code += &format!("struct {native}:{base}<{args}>{{}};\n");
        }
        for c in self.containers.values() {
            if let Shape::Iterator { owner, .. } = &c.shape {
                let native = &c.native;
                let owner = &self.types[owner];
                code += &format!(
                    "struct {native}{{Obj<{owner}> owner;{owner}::Entries::iterator position,end;std::uint64_t version;bool finished=false;explicit {native}(Obj<{owner}> value):owner(value),position(value->entries.begin()),end(value->entries.end()),version(value->version){{}}}};\n"
                );
            }
        }
        code
    }
    pub(super) fn container_declarations(&self) -> String {
        self.containers
            .values()
            .map(|c| {
                format!(
                    "Obj<{}> deep_copy(const Obj<{}>& value,Loc loc=Loc{{\"<copy>\",0,0}});\n",
                    c.native, c.native
                )
            })
            .collect()
    }
    pub(super) fn container_guards(&self) -> String {
        self.containers.values().filter(|c|matches!(c.shape,Shape::Iterator{..})).map(|c|format!("void rusk_guard_iter(const Obj<{}>& it,Loc loc){{if(!it->finished && it->version!=it->owner->version)fail(loc,\"container changed during iteration\");}}\n",c.native)).collect()
    }
    pub(super) fn key_hash_cpp(&self) -> String {
        let keys = self
            .records
            .iter()
            .filter(|(ty, _)| self.key_type(ty))
            .collect::<Vec<_>>();
        let mut code = String::new();
        for (ty, _) in &keys {
            code += &format!(
                "std::size_t rusk_hash_value(const Obj<{}>& x);\n",
                self.types[*ty]
            );
        }
        for (ty, record) in keys {
            code += &format!(
                "std::size_t rusk_hash_value(const Obj<{}>& x){{std::size_t hash=0;",
                self.types[ty]
            );
            for i in 0..record.fields.len() {
                code += &format!("rusk_hash_combine(hash,rusk_hash_value(x->field_{i}));");
            }
            code += "return hash;}\n";
        }
        code
    }
    pub(super) fn container_copy_cpp(&self) -> String {
        let mut code = String::new();
        for (ty, c) in &self.containers {
            let native = &c.native;
            let copies = self.container_copies[ty]
                .iter()
                .map(|&i| self.functions[i].cpp.clone())
                .collect::<Vec<_>>();
            let body = match &c.shape {
                Shape::Dict => format!(
                    "auto result=make_obj<{native}>({native}{{}});for(const auto& entry:value->entries){{auto key={}(entry.first,loc);auto item={}(entry.second,loc);result->entries.emplace(key,item);}}return result;",
                    copies[0], copies[1]
                ),
                Shape::Set => format!(
                    "auto result=make_obj<{native}>({native}{{}});for(const auto& entry:value->entries)result->entries.insert({}(entry,loc));return result;",
                    copies[0]
                ),
                Shape::Iterator { .. } => format!(
                    "rusk_guard_iter(value,loc);auto result=make_obj<{native}>({native}(value->owner));if(value->finished)result->finished=true;else result->position=value->position;return result;"
                ),
            };
            code +=
                &format!("Obj<{native}> deep_copy(const Obj<{native}>& value,Loc loc){{{body}}}\n");
        }
        code
    }
}
