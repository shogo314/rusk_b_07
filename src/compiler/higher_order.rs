use super::*;

#[derive(Clone)]
pub(super) struct IteratorType {
    pub native: String,
    pub element: String,
    pub definition: String,
    pub copy_code: String,
}

impl Compiler {
    pub(super) fn native_function(
        &mut self,
        name: &str,
        params: Vec<String>,
        result: &str,
        code: impl FnOnce(&str) -> String,
        span: &Span,
    ) -> Result<usize, String> {
        self.check_type(&Name {
            text: result.into(),
            span: span.clone(),
        })?;
        let index = self.functions.len();
        let cpp = format!("rusk_algorithm_{index}");
        let code = code(&cpp);
        self.functions.push(Function {
            name: Name {
                text: name.into(),
                span: span.clone(),
            },
            params,
            result: result.into(),
            cpp,
            code,
            body: None,
            checked: None,
        });
        Ok(index)
    }
    fn callback(
        &self,
        value: &Value,
        parameters: &[String],
        result: Option<&str>,
        span: &Span,
    ) -> Result<callables::Callable, String> {
        let f = self
            .callables
            .get(&value.ty)
            .cloned()
            .ok_or_else(|| span.error("expected a statically known callable"))?;
        if f.parameters != parameters {
            return Err(span.error(format!(
                "callback requires parameters ({}), found ({})",
                parameters.join(","),
                f.parameters.join(",")
            )));
        }
        if let Some(result) = result {
            self.same(result, &f.result, span)?;
        }
        Ok(f)
    }
    pub(super) fn copy_function(&mut self, ty: &str, span: &Span) -> Result<String, String> {
        let id = self
            .resolve(
                "copy",
                &[Value {
                    ty: ty.into(),
                    id: ValueId::Probe,
                }],
                None,
                span,
            )?
            .ok_or_else(|| span.error(format!("type '{ty}' has no copy function")))?;
        self.same(ty, &self.functions[id].result, span)?;
        self.used.insert(id);
        Ok(self.functions[id].cpp.clone())
    }
    fn iteration(
        &mut self,
        source: &Value,
        span: &Span,
    ) -> Result<(String, String, String, String, String), String> {
        let element = if let Some(element) = vector_element(&source.ty) {
            element.to_string()
        } else if source.ty == "Range" {
            "Int".into()
        } else if let Some(it) = self.iterator_types.get(&source.ty) {
            it.element.clone()
        } else if let Some(element) = self.collection_element(&source.ty) {
            element
        } else {
            return Err(span.error("expected an array, integer range, or lazy iterator"));
        };
        let create = self
            .resolve("__iter__", std::slice::from_ref(source), None, span)?
            .ok_or_else(|| span.error("source is not iterable"))?;
        let iterator = self.functions[create].result.clone();
        let dummy = Value {
            ty: iterator.clone(),
            id: ValueId::Probe,
        };
        let has = self
            .resolve(
                "__iter_has_next__",
                std::slice::from_ref(&dummy),
                None,
                span,
            )?
            .ok_or_else(|| span.error("iterator has no availability check"))?;
        let next = self
            .resolve("__iter_next__", &[dummy], None, span)?
            .ok_or_else(|| span.error("iterator has no next operation"))?;
        self.used.extend([create, has, next]);
        Ok((
            element,
            iterator,
            self.functions[create].cpp.clone(),
            self.functions[has].cpp.clone(),
            self.functions[next].cpp.clone(),
        ))
    }

    pub(super) fn higher_order(
        &mut self,
        name: &str,
        args: &[Value],
        span: &Span,
    ) -> Result<Option<usize>, String> {
        let types = args.iter().map(|v| v.ty.clone()).collect::<Vec<_>>();
        if matches!(name, "map" | "filter") && args.len() == 2 {
            let (element, source, create, has, next) = self.iteration(&args[0], span)?;
            let cb = self.callback(
                &args[1],
                std::slice::from_ref(&element),
                if name == "filter" { Some("Bool") } else { None },
                span,
            )?;
            let output = if name == "filter" {
                element
            } else {
                cb.result.clone()
            };
            if output == "Unit" {
                return Err(span.error("map callback must return a value"));
            }
            let invoke = self.functions[cb.function].cpp.clone();
            let ty = format!("__RuskIterator{}", self.iterator_types.len());
            let native = format!("RuskLazyIterator{}", self.iterator_types.len());
            let input_native = self.types[&source].clone();
            let callback_native = self.types[&args[1].ty].clone();
            let output_native = self.types[&output].clone();
            let definition = format!(
                "struct {native} {{ Obj<{input_native}> source; Obj<{callback_native}> callback; Obj<{output_native}> cached; bool ready=false; bool finished=false; }};\n"
            );
            let callback_copy = self.copy_function(&args[1].ty, span)?;
            let element_copy = self.copy_function(&output, span)?;
            let copy_code = format!(
                "Obj<{native}> deep_copy(const Obj<{native}>& it,Loc loc) {{ return make_obj<{native}>({native}{{deep_copy(it->source,loc),{callback_copy}(it->callback,loc),it->cached?{element_copy}(it->cached,loc):Obj<{output_native}>{{}},it->ready,it->finished}}); }}\n"
            );
            self.types.insert(ty.clone(), native.clone());
            self.iterator_types.insert(
                ty.clone(),
                IteratorType {
                    native: native.clone(),
                    element: output.clone(),
                    definition,
                    copy_code,
                },
            );
            let has_name = format!("rusk_lazy_has_{}", self.iterator_types.len());
            let next_name = format!("rusk_lazy_next_{}", self.iterator_types.len());
            let body = if name == "filter" {
                format!(
                    "if(it->finished) return make_obj<bool>(false); rusk_guard_iter(it->source,loc); if(it->ready) return make_obj<bool>(true); while(*{has}(it->source,loc)){{auto value={next}(it->source,loc);auto accepted={invoke}(it->callback,value,loc);rusk_guard_iter(it->source,loc);if(*accepted){{it->cached=value;it->ready=true;return make_obj<bool>(true);}}}}it->finished=true;return make_obj<bool>(false);"
                )
            } else {
                format!(
                    "if(it->finished)return make_obj<bool>(false); bool result=*{has}(it->source,loc);if(!result)it->finished=true;return make_obj<bool>(result);"
                )
            };
            let take = if name == "filter" {
                "auto value=it->cached;it->cached.reset();it->ready=false;return value;".to_string()
            } else {
                format!(
                    "auto value={next}(it->source,loc);auto result={invoke}(it->callback,value,loc);rusk_guard_iter(it->source,loc);return result;"
                )
            };
            let code = format!(
                "Obj<bool> {has_name}(Obj<{native}> it,Loc loc){{{body}}}\nObj<{output_native}> {next_name}(Obj<{native}> it,Loc loc){{if(!*{has_name}(it,loc))fail(loc,\"iterator exhausted\");{take}}}\n"
            );
            self.functions.push(Function {
                name: Name {
                    text: "__iter_has_next__".into(),
                    span: span.clone(),
                },
                params: vec![ty.clone()],
                result: "Bool".into(),
                cpp: has_name,
                code,
                body: None,
                checked: None,
            });
            let has_id = self.functions.len() - 1;
            self.used.insert(has_id);
            self.functions.push(Function {
                name: Name {
                    text: "__iter_next__".into(),
                    span: span.clone(),
                },
                params: vec![ty.clone()],
                result: output,
                cpp: next_name,
                code: String::new(),
                body: None,
                checked: None,
            });
            self.native_function(
                "__iter__",
                vec![ty.clone()],
                &ty,
                |cpp| format!("Obj<{native}> {cpp}(Obj<{native}> it,Loc){{return it;}}"),
                span,
            )?;
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
            self.native_function("__assign__",vec![ty.clone(),ty.clone()],"Unit",|cpp|format!("void {cpp}(Obj<{native}> a,Obj<{native}> b,Loc loc){{auto snapshot=deep_copy(b,loc);*a=std::move(*snapshot);}}"),span)?;
            let source_native = self.types[&args[0].ty].clone();
            return self.native_function(name,types,&ty,|cpp|format!("Obj<{native}> {cpp}(Obj<{source_native}> source,Obj<{callback_native}> callback,Loc loc){{return make_obj<{native}>({native}{{{create}(source,loc),callback,{{}},false,false}});}}"),span).map(Some);
        }
        if (name == "to_vec" && args.len() == 1)
            || (matches!(name, "all" | "any" | "fold" | "scan")
                && args.len()
                    == if matches!(name, "fold" | "scan") {
                        3
                    } else {
                        2
                    })
        {
            let (element, iterator, create, has, next) = self.iteration(&args[0], span)?;
            let source_native = self.types[&args[0].ty].clone();
            let mut params = format!("Obj<{source_native}> source");
            let mut result = format!("Vec<{element}>");
            let mut initial = String::new();
            let mut process = String::new();
            let mut finish = String::new();
            match name {
                "to_vec" => {
                    process = "result->push_back(value);".into();
                    finish = "return result;".into();
                }
                "all" | "any" => {
                    let cb = self.callback(
                        &args[1],
                        std::slice::from_ref(&element),
                        Some("Bool"),
                        span,
                    )?;
                    let invoke = &self.functions[cb.function].cpp;
                    let callback_native = &self.types[&args[1].ty];
                    params.push_str(&format!(",Obj<{callback_native}> callback"));
                    let wanted = if name == "all" { "false" } else { "true" };
                    process = format!(
                        "auto accepted={invoke}(callback,value,loc);rusk_guard_iter(it,loc);if(*accepted=={wanted})return make_obj<bool>({wanted});"
                    );
                    result = "Bool".into();
                    finish = format!("return make_obj<bool>({});", name == "all");
                }
                "fold" | "scan" => {
                    let acc = args[1].ty.clone();
                    let cb = self.callback(&args[2], &[acc.clone(), element], Some(&acc), span)?;
                    let invoke = &self.functions[cb.function].cpp.clone();
                    let copy = self.copy_function(&acc, span)?;
                    let acc_native = &self.types[&acc];
                    let callback_native = &self.types[&args[2].ty];
                    params.push_str(&format!(
                        ",Obj<{acc_native}> seed,Obj<{callback_native}> callback"
                    ));
                    initial = format!("auto accumulator={copy}(seed,loc);");
                    process = format!(
                        "accumulator={invoke}(callback,accumulator,value,loc);rusk_guard_iter(it,loc);"
                    );
                    if name == "scan" {
                        result = format!("Vec<{acc}>");
                        initial.push_str(&format!("result->push_back({copy}(accumulator,loc));"));
                        process.push_str(&format!("result->push_back({copy}(accumulator,loc));"));
                        finish = "return result;".into();
                    } else {
                        result = acc;
                        finish = "return accumulator;".into();
                    }
                }
                _ => {}
            }
            self.check_type(&Name {
                text: result.clone(),
                span: span.clone(),
            })?;
            let result_native = self.types[&result].clone();
            let allocate = if result.starts_with("Vec<") {
                format!("auto result=make_obj<{result_native}>({result_native}{{}});")
            } else {
                String::new()
            };
            let _ = iterator;
            return self.native_function(name,types,&result,|cpp|format!("Obj<{result_native}> {cpp}({params},Loc loc){{auto it={create}(source,loc);{allocate}{initial}while(*{has}(it,loc)){{auto value={next}(it,loc);{process}}}{finish}}}"),span).map(Some);
        }
        if name == "first_true" && args.len() == 3 {
            self.same("Int", &args[0].ty, span)?;
            self.same("Int", &args[1].ty, span)?;
            let cb = self.callback(&args[2], &["Int".into()], Some("Bool"), span)?;
            let invoke = self.functions[cb.function].cpp.clone();
            let callback_native = self.types[&args[2].ty].clone();
            return self.native_function(name,types,"Int",|cpp|format!("Obj<std::int64_t> {cpp}(Obj<std::int64_t> left,Obj<std::int64_t> right,Obj<{callback_native}> predicate,Loc loc){{auto l=*left,r=*right;if(l>r)fail(loc,\"invalid binary search interval\");while(l<r){{auto mid=static_cast<std::int64_t>(static_cast<__int128>(l)+(static_cast<__int128>(r)-l)/2);bool yes=*{invoke}(predicate,make_obj<std::int64_t>(mid),loc);if(yes)r=mid;else l=mid+1;}}return make_obj<std::int64_t>(l);}}"),span).map(Some);
        }
        if matches!(name, "sort_by_key" | "sort_by_comp") && args.len() == 2 {
            let element = vector_element(&args[0].ty)
                .ok_or_else(|| span.error("sorting requires an array"))?
                .to_string();
            let parameters = if name == "sort_by_key" {
                vec![element]
            } else {
                vec![element.clone(), element]
            };
            let cb = self.callback(
                &args[1],
                &parameters,
                if name == "sort_by_comp" {
                    Some("Bool")
                } else {
                    None
                },
                span,
            )?;
            let invoke = self.functions[cb.function].cpp.clone();
            let source_native = self.types[&args[0].ty].clone();
            let callback_native = self.types[&args[1].ty].clone();
            let guard =
                "if(source->version!=version)fail(loc,\"container changed during callback\");";
            let action = if name == "sort_by_comp" {
                format!(
                    "auto values=*source;std::stable_sort(values.begin(),values.end(),[&](const auto& a,const auto& b){{{guard}auto result={invoke}(callback,a,b,loc);{guard}return *result;}});source->assign(values.begin(),values.end());"
                )
            } else {
                if !self.automatic_value(&cb.result) {
                    return Err(span.error(
                        "sort keys require automatically ordered basic, array, or tuple values",
                    ));
                }
                let copy = self.copy_function(&cb.result, span)?;
                let key_native = &self.types[&cb.result];
                let element_native = &self.types[&parameters[0]];
                format!(
                    "std::vector<std::pair<Obj<{key_native}>,Obj<{element_native}>>> values;values.reserve(source->size());for(std::size_t i=0,n=source->size();i<n;++i){{auto value=(*source)[i];auto key={invoke}(callback,value,loc);{guard}values.push_back({{{copy}(key,loc),value}});}}std::stable_sort(values.begin(),values.end(),[](const auto& a,const auto& b){{return value_less(a.first,b.first);}});for(std::size_t i=0;i<values.size();++i)(*source)[i]=values[i].second;"
                )
            };
            return self.native_function(name,types,"Unit",|cpp|format!("void {cpp}(Obj<{source_native}> source,Obj<{callback_native}> callback,Loc loc){{auto version=source->version;{action}++source->version;}}"),span).map(Some);
        }
        Ok(None)
    }

    pub(super) fn lazy_cpp(&self) -> String {
        let mut code = String::new();
        for native in self.types.values().filter(|n| n.starts_with("RuskRecord")) {
            code.push_str(&format!("struct {native};\n"));
        }
        for it in self.iterator_types.values() {
            code.push_str(&format!("struct {};\n", it.native));
        }
        for it in self.iterator_types.values() {
            code.push_str(&it.definition);
        }
        code.push_str("template<class T> void rusk_guard_iter(const Obj<T>&,Loc){}\ntemplate<class T> void rusk_guard_iter(const Obj<RuskIter<T>>& it,Loc loc){check_iter(it,loc);}\n");
        code.push_str(&self.container_guards());
        for it in self.iterator_types.values() {
            let native = &it.native;
            code.push_str(&format!(
                "void rusk_guard_iter(const Obj<{native}>& it,Loc loc);\n"
            ));
        }
        for it in self.iterator_types.values() {
            let native = &it.native;
            code.push_str(&format!("void rusk_guard_iter(const Obj<{native}>& it,Loc loc){{if(!it->finished)rusk_guard_iter(it->source,loc);}}\n"));
        }
        code
    }
    pub(super) fn lazy_copy_declarations(&self) -> String {
        self.iterator_types
            .values()
            .map(|it| {
                format!(
                    "Obj<{}> deep_copy(const Obj<{}>& it,Loc loc=Loc{{\"<copy>\",0,0}});\n",
                    it.native, it.native
                )
            })
            .collect()
    }
    pub(super) fn lazy_copy_cpp(&self) -> String {
        self.iterator_types
            .values()
            .map(|it| it.copy_code.as_str())
            .collect()
    }
}
