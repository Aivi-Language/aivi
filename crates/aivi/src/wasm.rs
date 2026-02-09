use std::collections::HashMap;

use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection,
    Function, FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction, MemorySection,
    MemoryType, Module, RefType, TypeSection, ValType,
};

use crate::hir::{HirBlockItem, HirExpr, HirPattern, HirProgram};
use crate::AiviError;

#[derive(Clone, Copy)]
enum DefKind {
    Function,
    Value,
}

struct DefInfo {
    name: String,
    params: Vec<String>,
    body: HirExpr,
    kind: DefKind,
}

#[derive(Clone, Copy)]
struct DefMeta {
    func_index: u32,
    param_len: usize,
    kind: DefKind,
}

struct DataBuilder {
    next_offset: u32,
    data: Vec<(u32, Vec<u8>)>,
    strings: HashMap<String, (u32, u32)>,
}

impl DataBuilder {
    fn new() -> Self {
        Self {
            next_offset: 16,
            data: Vec::new(),
            strings: HashMap::new(),
        }
    }

    fn intern_string(&mut self, text: &str) -> (u32, u32) {
        if let Some(&info) = self.strings.get(text) {
            return info;
        }
        let bytes = text.as_bytes().to_vec();
        let offset = self.add_bytes(bytes.clone(), 4);
        let len = bytes.len() as u32;
        self.strings
            .insert(text.to_string(), (offset, len));
        (offset, len)
    }

    fn intern_list(&mut self, values: &[i64]) -> (u32, u32) {
        let mut bytes = Vec::with_capacity(values.len() * 8);
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let offset = self.add_bytes(bytes, 8);
        (offset, values.len() as u32)
    }

    fn add_bytes(&mut self, bytes: Vec<u8>, align: u32) -> u32 {
        let offset = align_up(self.next_offset, align);
        self.data.push((offset, bytes));
        self.next_offset = offset + self.data.last().unwrap().1.len() as u32;
        offset
    }
}

struct TypePool {
    types: TypeSection,
    map: HashMap<String, u32>,
}

impl TypePool {
    fn new() -> Self {
        Self {
            types: TypeSection::new(),
            map: HashMap::new(),
        }
    }

    fn type_index(&mut self, params: &[ValType], results: &[ValType]) -> u32 {
        let key = signature_key(params, results);
        if let Some(&index) = self.map.get(&key) {
            return index;
        }
        let index = self.map.len() as u32;
        self.types.function(params.iter().copied(), results.iter().copied());
        self.map.insert(key, index);
        index
    }
}

struct CodegenEnv {
    defs: HashMap<String, DefMeta>,
    print_index: u32,
    alloc_index: u32,
    data: DataBuilder,
}

pub fn compile_wasm(program: HirProgram) -> Result<Vec<u8>, AiviError> {
    if program.modules.len() != 1 {
        return Err(AiviError::Codegen(
            "WASM build currently supports a single module".to_string(),
        ));
    }
    let mut modules = program.modules;
    let module = modules.pop().unwrap();
    let mut defs = collect_defs(module.defs)?;
    let main_name = defs
        .iter()
        .find(|def| def.name == "main")
        .map(|def| def.name.clone())
        .ok_or_else(|| {
            AiviError::Codegen("WASM build expects a main definition".to_string())
        })?;
    if let Some(main_def) = defs.iter().find(|def| def.name == "main") {
        if !main_def.params.is_empty() {
            return Err(AiviError::Codegen(
                "main must not take parameters for WASM build".to_string(),
            ));
        }
    } else {
        return Err(AiviError::Codegen(
            "WASM build expects a main definition".to_string(),
        ));
    }

    let mut type_pool = TypePool::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut globals = GlobalSection::new();
    let mut memory = MemorySection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut data_section = DataSection::new();

    let fd_write_type = type_pool.type_index(
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        &[ValType::I32],
    );
    imports.import(
        "wasi_snapshot_preview1",
        "fd_write",
        EntityType::Function(fd_write_type),
    );
    let import_count = 1u32;

    let print_type = type_pool.type_index(&[ValType::I64], &[ValType::I64]);
    let alloc_type = type_pool.type_index(&[ValType::I32], &[ValType::I32]);
    let start_type = type_pool.type_index(&[], &[]);

    let mut next_func_index = import_count;
    let print_index = next_func_index;
    next_func_index += 1;
    functions.function(print_type);

    let alloc_index = next_func_index;
    next_func_index += 1;
    functions.function(alloc_type);

    let mut def_meta = HashMap::new();
    for def in &defs {
        let func_index = next_func_index;
        next_func_index += 1;
        let param_types = vec![ValType::I64; def.params.len()];
        let func_type = type_pool.type_index(&param_types, &[ValType::I64]);
        functions.function(func_type);
        def_meta.insert(
            def.name.clone(),
            DefMeta {
                func_index,
                param_len: def.params.len(),
                kind: def.kind,
            },
        );
    }

    let start_index = next_func_index;
    functions.function(start_type);

    let mut env = CodegenEnv {
        defs: def_meta,
        print_index,
        alloc_index,
        data: DataBuilder::new(),
    };

    let mut def_funcs = Vec::with_capacity(defs.len());
    for def in defs.drain(..) {
        def_funcs.push(compile_def(def, &mut env)?);
    }

    let iovec_offset = align_up(env.data.next_offset, 4);
    let heap_base = iovec_offset + 12;

    let print_func = build_print_function(0, iovec_offset, 0);
    let alloc_func = build_alloc_function();

    let main_index = env
        .defs
        .get(&main_name)
        .ok_or_else(|| AiviError::Codegen("main definition missing".to_string()))?
        .func_index;
    let start_func = build_start_function(main_index);

    let memory_pages = ((heap_base as u64 + 0xFFFF) / 0x10000).max(1);
    memory.memory(MemoryType {
        minimum: memory_pages,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(heap_base as i32),
    );

    for (offset, bytes) in env.data.data {
        data_section.active(0, &ConstExpr::i32_const(offset as i32), bytes);
    }

    exports.export("_start", ExportKind::Func, start_index);

    code.function(&print_func);
    code.function(&alloc_func);
    for func in def_funcs {
        code.function(&func);
    }
    code.function(&start_func);

    let mut module = Module::new();
    module.section(&type_pool.types);
    module.section(&imports);
    module.section(&functions);
    module.section(&memory);
    module.section(&globals);
    module.section(&exports);
    module.section(&code);
    module.section(&data_section);

    Ok(module.finish())
}

fn collect_defs(defs: Vec<crate::hir::HirDef>) -> Result<Vec<DefInfo>, AiviError> {
    let mut seen = HashMap::new();
    let mut results = Vec::new();
    for def in defs {
        if seen.contains_key(&def.name) {
            return Err(AiviError::Codegen(format!(
                "duplicate definition {name}",
                name = def.name
            )));
        }
        let (params, body, kind) = split_lambdas(def.expr);
        let info = DefInfo {
            name: def.name.clone(),
            params,
            body,
            kind,
        };
        seen.insert(def.name, ());
        results.push(info);
    }
    Ok(results)
}

fn split_lambdas(expr: HirExpr) -> (Vec<String>, HirExpr, DefKind) {
    let mut params = Vec::new();
    let mut current = expr;
    loop {
        match current {
            HirExpr::Lambda { param, body, .. } => {
                params.push(param);
                current = *body;
            }
            other => {
                let kind = if params.is_empty() {
                    DefKind::Value
                } else {
                    DefKind::Function
                };
                return (params, other, kind);
            }
        }
    }
}

fn build_print_function(fd_write_index: u32, iovec_offset: u32, memory_index: u32) -> Function {
    let mut func = Function::new(Vec::new());
    let memarg = wasm_encoder::MemArg {
        offset: 0,
        align: 2,
        memory_index,
    };
    func.instruction(&Instruction::I32Const(iovec_offset as i32));
    func.instruction(&Instruction::LocalGet(0));
    func.instruction(&Instruction::I32WrapI64);
    func.instruction(&Instruction::I32Store(memarg));

    func.instruction(&Instruction::I32Const((iovec_offset + 4) as i32));
    func.instruction(&Instruction::LocalGet(0));
    func.instruction(&Instruction::I64Const(32));
    func.instruction(&Instruction::I64ShrU);
    func.instruction(&Instruction::I32WrapI64);
    func.instruction(&Instruction::I32Store(memarg));

    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Const(iovec_offset as i32));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Const((iovec_offset + 8) as i32));
    func.instruction(&Instruction::Call(fd_write_index));
    func.instruction(&Instruction::Drop);
    func.instruction(&Instruction::I64Const(0));
    func.instruction(&Instruction::End);
    func
}

fn build_alloc_function() -> Function {
    let locals = vec![(1, ValType::I32)];
    let mut func = Function::new(locals);
    func.instruction(&Instruction::GlobalGet(0));
    func.instruction(&Instruction::LocalTee(1));
    func.instruction(&Instruction::LocalGet(0));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::GlobalSet(0));
    func.instruction(&Instruction::LocalGet(1));
    func.instruction(&Instruction::End);
    func
}

fn build_start_function(main_index: u32) -> Function {
    let mut func = Function::new(Vec::new());
    func.instruction(&Instruction::Call(main_index));
    func.instruction(&Instruction::Drop);
    func.instruction(&Instruction::End);
    func
}

fn compile_def(def: DefInfo, env: &mut CodegenEnv) -> Result<Function, AiviError> {
    let mut compiler = Compiler::new(&def, env);
    compiler.compile_expr(def.body)?;
    compiler.emit(Instruction::End);
    Ok(compiler.finish())
}

struct Compiler<'a> {
    env: &'a mut CodegenEnv,
    locals: HashMap<String, u32>,
    param_count: u32,
    local_count: u32,
    instrs: Vec<Instruction<'a>>,
    def_name: String,
}

impl<'a> Compiler<'a> {
    fn new(def: &DefInfo, env: &'a mut CodegenEnv) -> Self {
        let mut locals = HashMap::new();
        for (index, param) in def.params.iter().enumerate() {
            locals.insert(param.clone(), index as u32);
        }
        Self {
            env,
            locals,
            param_count: def.params.len() as u32,
            local_count: 0,
            instrs: Vec::new(),
            def_name: def.name.clone(),
        }
    }

    fn finish(self) -> Function {
        let locals = if self.local_count == 0 {
            Vec::new()
        } else {
            vec![(self.local_count, ValType::I64)]
        };
        let mut func = Function::new(locals);
        for instr in self.instrs {
            func.instruction(&instr);
        }
        func
    }

    fn emit(&mut self, instr: Instruction<'a>) {
        self.instrs.push(instr);
    }

    fn add_local(&mut self, name: String) -> u32 {
        let index = self.param_count + self.local_count;
        self.local_count += 1;
        self.locals.insert(name, index);
        index
    }

    fn compile_expr(&mut self, expr: HirExpr) -> Result<(), AiviError> {
        match expr {
            HirExpr::Var { name, .. } => self.compile_var(&name),
            HirExpr::LitNumber { text, .. } => {
                let value = parse_int(&text)?;
                self.emit(Instruction::I64Const(value));
                Ok(())
            }
            HirExpr::LitBool { value, .. } => {
                self.emit(Instruction::I64Const(if value { 1 } else { 0 }));
                Ok(())
            }
            HirExpr::LitString { text, .. } => {
                let (offset, len) = self.env.data.intern_string(&text);
                self.emit(Instruction::I64Const(pack_value(offset, len)));
                Ok(())
            }
            HirExpr::List { items, .. } => self.compile_list(items),
            HirExpr::Binary { op, left, right, .. } => self.compile_binary(&op, *left, *right),
            HirExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.compile_expr(*cond)?;
                self.emit(Instruction::I32WrapI64);
                self.emit(Instruction::If(BlockType::Result(ValType::I64)));
                self.compile_expr(*then_branch)?;
                self.emit(Instruction::Else);
                self.compile_expr(*else_branch)?;
                self.emit(Instruction::End);
                Ok(())
            }
            HirExpr::Call { func, args, .. } => self.compile_call(*func, args),
            HirExpr::App { func, arg, .. } => self.compile_call(*func, vec![*arg]),
            HirExpr::Block { items, .. } => self.compile_block(items),
            HirExpr::LitDateTime { .. }
            | HirExpr::Tuple { .. }
            | HirExpr::Record { .. }
            | HirExpr::Patch { .. }
            | HirExpr::FieldAccess { .. }
            | HirExpr::Index { .. }
            | HirExpr::Match { .. }
            | HirExpr::Lambda { .. }
            | HirExpr::JsxElement { .. }
            | HirExpr::Raw { .. } => Err(self.unsupported("expression")),
        }
    }

    fn compile_var(&mut self, name: &str) -> Result<(), AiviError> {
        if let Some(&index) = self.locals.get(name) {
            self.emit(Instruction::LocalGet(index));
            return Ok(());
        }
        match name {
            "True" => {
                self.emit(Instruction::I64Const(1));
                return Ok(());
            }
            "False" => {
                self.emit(Instruction::I64Const(0));
                return Ok(());
            }
            "Unit" => {
                self.emit(Instruction::I64Const(0));
                return Ok(());
            }
            _ => {}
        }
        let Some(meta) = self.env.defs.get(name) else {
            return Err(AiviError::Codegen(format!(
                "unknown name {name} in {def}",
                def = self.def_name
            )));
        };
        match meta.kind {
            DefKind::Value => {
                self.emit(Instruction::Call(meta.func_index));
                Ok(())
            }
            DefKind::Function => Err(AiviError::Codegen(format!(
                "function value {name} used without call in {def}",
                def = self.def_name
            ))),
        }
    }

    fn compile_list(&mut self, items: Vec<crate::hir::HirListItem>) -> Result<(), AiviError> {
        if items.iter().any(|item| item.spread) {
            return Err(self.unsupported("list spread"));
        }
        let mut values = Vec::with_capacity(items.len());
        for item in items {
            let value = self.const_value(item.expr)?;
            values.push(value);
        }
        let (offset, len) = self.env.data.intern_list(&values);
        self.emit(Instruction::I64Const(pack_value(offset, len)));
        Ok(())
    }

    fn const_value(&mut self, expr: HirExpr) -> Result<i64, AiviError> {
        match expr {
            HirExpr::LitNumber { text, .. } => parse_int(&text),
            HirExpr::LitBool { value, .. } => Ok(if value { 1 } else { 0 }),
            HirExpr::LitString { text, .. } => {
                let (offset, len) = self.env.data.intern_string(&text);
                Ok(pack_value(offset, len))
            }
            HirExpr::Var { name, .. } => match name.as_str() {
                "True" => Ok(1),
                "False" => Ok(0),
                "Unit" => Ok(0),
                _ => Err(self.unsupported("non-constant list element")),
            },
            _ => Err(self.unsupported("non-constant list element")),
        }
    }

    fn compile_binary(&mut self, op: &str, left: HirExpr, right: HirExpr) -> Result<(), AiviError> {
        self.compile_expr(left)?;
        self.compile_expr(right)?;
        match op {
            "+" => self.emit(Instruction::I64Add),
            "-" => self.emit(Instruction::I64Sub),
            "*" => self.emit(Instruction::I64Mul),
            "/" => self.emit(Instruction::I64DivS),
            "==" => {
                self.emit(Instruction::I64Eq);
                self.emit(Instruction::I64ExtendI32U);
            }
            "!=" => {
                self.emit(Instruction::I64Ne);
                self.emit(Instruction::I64ExtendI32U);
            }
            "<" => {
                self.emit(Instruction::I64LtS);
                self.emit(Instruction::I64ExtendI32U);
            }
            "<=" => {
                self.emit(Instruction::I64LeS);
                self.emit(Instruction::I64ExtendI32U);
            }
            ">" => {
                self.emit(Instruction::I64GtS);
                self.emit(Instruction::I64ExtendI32U);
            }
            ">=" => {
                self.emit(Instruction::I64GeS);
                self.emit(Instruction::I64ExtendI32U);
            }
            _ => return Err(self.unsupported("binary operator")),
        }
        Ok(())
    }

    fn compile_call(&mut self, func: HirExpr, args: Vec<HirExpr>) -> Result<(), AiviError> {
        let (target, mut collected) = flatten_app(func);
        collected.extend(args);
        let HirExpr::Var { name, .. } = target else {
            return Err(self.unsupported("call target"));
        };
        if name == "print" {
            if collected.len() != 1 {
                return Err(AiviError::Codegen(format!(
                    "print expects 1 argument in {def}",
                    def = self.def_name
                )));
            }
            self.compile_expr(collected.remove(0))?;
            self.emit(Instruction::Call(self.env.print_index));
            return Ok(());
        }
        let Some(meta) = self.env.defs.get(&name).copied() else {
            return Err(AiviError::Codegen(format!(
                "unknown function {name} in {def}",
                def = self.def_name
            )));
        };
        if meta.param_len != collected.len() {
            return Err(AiviError::Codegen(format!(
                "function {name} expects {expected} args, got {actual} in {def}",
                expected = meta.param_len,
                actual = collected.len(),
                def = self.def_name
            )));
        }
        for arg in collected {
            self.compile_expr(arg)?;
        }
        self.emit(Instruction::Call(meta.func_index));
        Ok(())
    }

    fn compile_block(&mut self, items: Vec<HirBlockItem>) -> Result<(), AiviError> {
        if items.is_empty() {
            self.emit(Instruction::I64Const(0));
            return Ok(());
        }
        let total = items.len();
        for (index, item) in items.into_iter().enumerate() {
            let last = index + 1 == total;
            match item {
                HirBlockItem::Bind { pattern, expr } => {
                    self.compile_expr(expr)?;
                    match pattern {
                        HirPattern::Var { name, .. } => {
                            let local_index = self.add_local(name);
                            self.emit(Instruction::LocalSet(local_index));
                        }
                        HirPattern::Wildcard { .. } => {
                            self.emit(Instruction::Drop);
                        }
                        _ => return Err(self.unsupported("pattern binding")),
                    }
                    if last {
                        self.emit(Instruction::I64Const(0));
                    }
                }
                HirBlockItem::Expr { expr } => {
                    self.compile_expr(expr)?;
                    if !last {
                        self.emit(Instruction::Drop);
                    }
                }
                HirBlockItem::Filter { .. }
                | HirBlockItem::Yield { .. }
                | HirBlockItem::Recurse { .. } => return Err(self.unsupported("block item")),
            }
        }
        Ok(())
    }

    fn unsupported(&self, what: &str) -> AiviError {
        AiviError::Codegen(format!(
            "unsupported {what} in WASM backend ({def})",
            def = self.def_name
        ))
    }
}

fn flatten_app(expr: HirExpr) -> (HirExpr, Vec<HirExpr>) {
    let mut args = Vec::new();
    let target = flatten_app_inner(expr, &mut args);
    (target, args)
}

fn flatten_app_inner(expr: HirExpr, args: &mut Vec<HirExpr>) -> HirExpr {
    match expr {
        HirExpr::App { func, arg, .. } => {
            let target = flatten_app_inner(*func, args);
            args.push(*arg);
            target
        }
        other => other,
    }
}

fn pack_value(offset: u32, len: u32) -> i64 {
    ((len as u64) << 32 | offset as u64) as i64
}

fn parse_int(text: &str) -> Result<i64, AiviError> {
    if text.contains('.') {
        return Err(AiviError::Codegen(format!(
            "float literal {text} not supported yet"
        )));
    }
    if text
        .chars()
        .any(|ch| !(ch.is_ascii_digit() || ch == '-'))
    {
        return Err(AiviError::Codegen(format!(
            "numeric literal {text} not supported yet"
        )));
    }
    text.parse::<i64>().map_err(|err| {
        AiviError::Codegen(format!("failed to parse number {text}: {err}"))
    })
}

fn align_up(value: u32, align: u32) -> u32 {
    if align == 0 {
        return value;
    }
    (value + align - 1) / align * align
}

fn signature_key(params: &[ValType], results: &[ValType]) -> String {
    let mut key = String::new();
    for param in params {
        key.push(valtype_code(*param));
    }
    key.push('-');
    for result in results {
        key.push(valtype_code(*result));
    }
    key
}

fn valtype_code(value: ValType) -> char {
    match value {
        ValType::I32 => 'i',
        ValType::I64 => 'I',
        ValType::F32 => 'f',
        ValType::F64 => 'F',
        ValType::V128 => 'v',
        ValType::Ref(RefType::FUNCREF) => 'r',
        ValType::Ref(RefType::EXTERNREF) => 'e',
        ValType::Ref(_) => 'r',
    }
}

pub fn run_wasm(wasm: &[u8]) -> Result<(), AiviError> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_binary(&engine, wasm)
        .map_err(|err| AiviError::Wasm(err.to_string()))?;
    let mut linker: wasmtime::Linker<wasmtime_wasi::preview1::WasiP1Ctx> =
        wasmtime::Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx| ctx)
        .map_err(|err| AiviError::Wasm(err.to_string()))?;
    let wasi = wasmtime_wasi::WasiCtxBuilder::new()
        .inherit_stdio()
        .build_p1();
    let mut store = wasmtime::Store::new(&engine, wasi);
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|err| AiviError::Wasm(err.to_string()))?;
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|err| AiviError::Wasm(err.to_string()))?;
    start
        .call(&mut store, ())
        .map_err(|err| AiviError::Wasm(err.to_string()))?;
    Ok(())
}
