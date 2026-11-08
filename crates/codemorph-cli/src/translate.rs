//! Deterministic C++ -> Rust transpiler for a supported subset.
//!
//! The translator is intentionally conservative: anything outside the supported
//! subset is reported as `Unsupported(reason)` instead of producing garbage.
//! Neural models may still be used as *proposers* by the caller, but this module
//! never calls a model and never fabricates a result.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum TranslationOutcome {
    Translated(String),
    Unsupported(String),
}

pub fn translate_cpp_to_rust(source: &str) -> TranslationOutcome {
    let stripped = strip_preprocessor_lines(source);
    let tokens = match tokenize(&stripped) {
        Ok(t) => t,
        Err(e) => return TranslationOutcome::Unsupported(e),
    };
    if let Some(reason) = detect_unsupported(&tokens) {
        return TranslationOutcome::Unsupported(format!("unsupported construct: {}", reason));
    }
    let mut parser = Parser::new(&tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => return TranslationOutcome::Unsupported(e),
    };
    match Codegen::new(&program).generate() {
        Ok(code) => TranslationOutcome::Translated(code),
        Err(e) => TranslationOutcome::Unsupported(e),
    }
}

// ---------------------------------------------------------------------------
// Preprocessing / tokenizer
// ---------------------------------------------------------------------------

fn strip_preprocessor_lines(source: &str) -> String {
    let mut out = String::new();
    let mut in_preproc = false;
    for line in source.lines() {
        if in_preproc {
            if !line.trim_end().ends_with('\\') {
                in_preproc = false;
            }
            out.push('\n');
            continue;
        }
        if line.trim_start().starts_with('#') {
            if line.trim_end().ends_with('\\') {
                in_preproc = true;
            }
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Num(String),
    Str(String),
    Chr(String),
    Op(String),
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        if c == '"' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Tok::Str(chars[start..i.min(chars.len())].iter().collect()));
            continue;
        }
        if c == '\'' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == '\'' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Tok::Chr(chars[start..i.min(chars.len())].iter().collect()));
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            if c == '0' && i + 1 < chars.len() && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
            } else {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    i += 1;
                    if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                        i += 1;
                    }
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            out.push(Tok::Num(chars[start..i].iter().collect()));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        const OPS2: [&str; 17] = [
            "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "++", "--", "+=", "-=", "*=", "/=",
            "%=", "->", "::",
        ];
        if i + 1 < chars.len() {
            let two: String = chars[i..i + 2].iter().collect();
            if OPS2.contains(&two.as_str()) {
                out.push(Tok::Op(two));
                i += 2;
                continue;
            }
        }
        out.push(Tok::Op(c.to_string()));
        i += 1;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Unsupported construct detection (first offending construct wins)
// ---------------------------------------------------------------------------

fn ident_at(tokens: &[Tok], i: usize) -> Option<&str> {
    match tokens.get(i) {
        Some(Tok::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn op_at(tokens: &[Tok], i: usize) -> Option<&str> {
    match tokens.get(i) {
        Some(Tok::Op(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn detect_unsupported(tokens: &[Tok]) -> Option<String> {
    let mut i = 0usize;
    while i < tokens.len() {
        if let Some(id) = ident_at(tokens, i) {
            let prev_is_std = i >= 2
                && ident_at(tokens, i - 2) == Some("std")
                && op_at(tokens, i - 1) == Some("::");
            let next_is_lt = op_at(tokens, i + 1) == Some("<");
            let reason: Option<&str> = match id {
                "template" => Some("template"),
                "class" => Some("class"),
                "enum" => Some("enum"),
                "namespace" if ident_at(tokens, i.wrapping_sub(1)) != Some("using") => {
                    Some("namespace")
                }
                "operator" => match ident_at(tokens, i + 1) {
                    Some("new") => Some("operator new"),
                    Some("delete") => Some("operator delete"),
                    _ => Some("operator overloading"),
                },
                "new" => Some("operator new"),
                "delete" => Some("operator delete"),
                "reinterpret_cast" => Some("reinterpret_cast"),
                "static_cast" => Some("static_cast"),
                "dynamic_cast" => Some("dynamic_cast"),
                "const_cast" => Some("const_cast"),
                "alignof" => Some("alignof"),
                "alignas" => Some("alignas"),
                "requires" => Some("requires"),
                "concept" | "concepts" => Some("concepts"),
                "noexcept" => Some("noexcept"),
                "auto" => Some("auto"),
                "decltype" => Some("decltype"),
                "unique_ptr" => Some("unique_ptr"),
                "shared_ptr" => Some("shared_ptr"),
                "make_unique" => Some("make_unique"),
                "make_shared" => Some("make_shared"),
                "variant" => Some("variant"),
                "visit" if prev_is_std => Some("std::visit"),
                "unordered_map" => Some("unordered_map"),
                "map" if prev_is_std => Some("std::map"),
                "vector" if prev_is_std || next_is_lt => Some("vector"),
                "set" if prev_is_std => Some("std::set"),
                "pair" if prev_is_std || next_is_lt => Some("pair"),
                "tuple" => Some("tuple"),
                "istringstream" => Some("istringstream"),
                "ostringstream" => Some("ostringstream"),
                "stringstream" => Some("stringstream"),
                "string_view" => Some("string_view"),
                "sort" if prev_is_std => Some("std::sort"),
                "move" if prev_is_std => Some("std::move"),
                "forward" if prev_is_std => Some("std::forward"),
                "throw" => Some("throw"),
                "try" => Some("try"),
                "catch" => Some("catch"),
                "switch" => Some("switch"),
                "case" => Some("switch"),
                "goto" => Some("goto"),
                "do" => Some("do-while loop"),
                "nodiscard" => Some("[[nodiscard]]"),
                _ => None,
            };
            if let Some(r) = reason {
                let suggestion = unsupported_suggestion(r);
                let msg = match suggestion {
                    Some(s) => format!("{} (suggest: {})", r, s),
                    None => r.to_string(),
                };
                return Some(msg);
            }
        }
        if op_at(tokens, i) == Some("[") {
            let next = op_at(tokens, i + 1);
            if matches!(next, Some("]") | Some("&") | Some("=")) {
                return Some("lambda".to_string());
            }
        }
        i += 1;
    }
    None
}

fn unsupported_suggestion(name: &str) -> Option<&'static str> {
    // Deterministic, non-hallucinated API/library mapping hints (the
    // idiomatizer model may later refine these). Keep short and crate-focused.
    match name {
        "vector" => Some("Vec<T>"),
        "unordered_map" => Some("HashMap<K, V>"),
        "map" => Some("HashMap<K, V> (or BTreeMap)"),
        "set" => Some("HashSet<T> (or BTreeSet)"),
        "pair" => Some("tuple (a, b)"),
        "tuple" => Some("tuple (..)"),
        "string_view" => Some("&str"),
        "sort" => Some(".sort() / .sort_unstable()"),
        "istringstream" | "ostringstream" | "stringstream" => {
            Some("std::io::Cursor / String split")
        }
        "unique_ptr" => Some("Box<T>"),
        "shared_ptr" => Some("Arc<T>"),
        "make_unique" => Some("Box::new"),
        "make_shared" => Some("Arc::new"),
        "variant" => Some("enum"),
        "visit" => Some("match on an enum"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Program {
    items: Vec<Item>,
}

#[derive(Clone, Debug)]
enum Item {
    Struct(StructDef),
    Function(FunctionDef),
}

#[derive(Clone, Debug)]
struct StructDef {
    name: String,
    fields: Vec<Field>,
    methods: Vec<Method>,
}

#[derive(Clone, Debug)]
struct Field {
    name: String,
    ty: CppType,
}

#[derive(Clone, Debug)]
struct Method {
    ret: CppType,
    name: String,
    params: Vec<Param>,
    body: Stmt,
}

#[derive(Clone, Debug)]
struct FunctionDef {
    ret: CppType,
    name: String,
    params: Vec<Param>,
    body: Stmt,
}

#[derive(Clone, Debug)]
struct Param {
    name: String,
    ty: CppType,
}

#[derive(Clone, Debug, PartialEq)]
struct CppType {
    base: BaseType,
    is_const: bool,
    pointers: usize,
    reference: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
enum BaseType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Usize,
    Isize,
    F32,
    F64,
    Bool,
    Char,
    String,
    Void,
    Named(String),
}

impl BaseType {
    fn is_integer(&self) -> bool {
        matches!(
            self,
            BaseType::I8
                | BaseType::I16
                | BaseType::I32
                | BaseType::I64
                | BaseType::U8
                | BaseType::U16
                | BaseType::U32
                | BaseType::U64
                | BaseType::Usize
                | BaseType::Isize
        )
    }
}

#[derive(Clone, Debug)]
enum Expr {
    Ident(String),
    Num(String),
    Str(String),
    Chr(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Member(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Inc(Box<Expr>, i32, bool),
    Assign(Box<Expr>, String, Box<Expr>),
    InitList(Vec<Expr>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Block(Vec<Stmt>),
    Decl(Vec<Declarator>),
    Expr(Expr),
    If {
        cond: Expr,
        then: Box<Stmt>,
        els: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Empty,
    Cout {
        args: Vec<Expr>,
        newline: bool,
    },
    Cin {
        vars: Vec<String>,
        guard: Option<Box<Stmt>>,
    },
}

#[derive(Clone, Debug)]
struct Declarator {
    name: String,
    ty: CppType,
    init: Option<Expr>,
    is_const: bool,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    no_shift: bool,
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "return"
            | "if"
            | "else"
            | "while"
            | "for"
            | "break"
            | "continue"
            | "do"
            | "switch"
            | "case"
            | "throw"
            | "try"
            | "catch"
            | "goto"
            | "using"
            | "struct"
            | "class"
            | "namespace"
            | "template"
            | "new"
            | "delete"
            | "sizeof"
            | "static_cast"
            | "reinterpret_cast"
            | "const_cast"
            | "dynamic_cast"
            | "auto"
            | "const"
            | "unsigned"
            | "signed"
            | "long"
            | "short"
            | "int"
            | "char"
            | "bool"
            | "float"
            | "double"
            | "void"
            | "size_t"
            | "public"
            | "private"
            | "protected"
            | "static"
            | "operator"
    )
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Tok]) -> Self {
        Self {
            toks,
            pos: 0,
            no_shift: false,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn peek_ident(&self) -> Option<&str> {
        ident_at(self.toks, self.pos)
    }

    fn peek_op(&self) -> Option<&str> {
        op_at(self.toks, self.pos)
    }

    fn is_ident(&self, name: &str) -> bool {
        ident_at(self.toks, self.pos) == Some(name)
    }

    fn is_op(&self, op: &str) -> bool {
        op_at(self.toks, self.pos) == Some(op)
    }

    fn is_std(&self, name: &str) -> bool {
        ident_at(self.toks, self.pos) == Some("std")
            && op_at(self.toks, self.pos + 1) == Some("::")
            && ident_at(self.toks, self.pos + 2) == Some(name)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn eat_ident(&mut self, name: &str) -> bool {
        if self.is_ident(name) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.toks.get(self.pos) {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.pos += 1;
                Ok(s)
            }
            other => Err(format!("expected identifier, found {:?}", other)),
        }
    }

    fn expect_op(&mut self, op: &str) -> Result<(), String> {
        if self.eat_op(op) {
            Ok(())
        } else {
            Err(format!(
                "expected '{}', found {:?}",
                op,
                self.toks.get(self.pos)
            ))
        }
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut items = Vec::new();
        while !self.at_end() {
            if self.eat_op(";") {
                continue;
            }
            if self.is_ident("using") {
                self.advance();
                if self.eat_ident("namespace") {
                    let _ = self.expect_ident()?;
                    self.expect_op(";")?;
                    continue;
                }
                return Err("unsupported construct: using declaration".to_string());
            }
            if self.is_ident("struct") {
                items.push(Item::Struct(self.parse_struct()?));
                continue;
            }
            let ret = self.parse_type()?;
            let name = self.expect_ident()?;
            if !self.is_op("(") {
                return Err("unsupported construct: global variable or declaration".to_string());
            }
            self.advance();
            let params = self.parse_params()?;
            self.expect_op(")")?;
            let _ = self.eat_ident("const");
            if !self.is_op("{") {
                return Err(format!(
                    "unsupported construct: function declaration without definition ({})",
                    name
                ));
            }
            let body = self.parse_block()?;
            items.push(Item::Function(FunctionDef {
                ret,
                name,
                params,
                body,
            }));
        }
        Ok(Program { items })
    }

    fn parse_struct(&mut self) -> Result<StructDef, String> {
        self.advance();
        let name = self.expect_ident()?;
        self.expect_op("{")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.eat_op("}") {
            if self.at_end() {
                return Err("unexpected end of input in struct".to_string());
            }
            if self.is_ident("public") || self.is_ident("private") || self.is_ident("protected") {
                return Err("unsupported construct: access specifier".to_string());
            }
            if self.is_op("~") {
                return Err("unsupported construct: destructor".to_string());
            }
            if self.is_ident("static") {
                return Err("unsupported construct: static member".to_string());
            }
            if self.is_ident("struct") || self.is_ident("class") {
                return Err("unsupported construct: nested type".to_string());
            }
            if self.peek_ident() == Some(name.as_str())
                && op_at(self.toks, self.pos + 1) == Some("(")
            {
                return Err("unsupported construct: constructor".to_string());
            }
            let ty = self.parse_type()?;
            let member_name = self.expect_ident()?;
            if self.is_op("(") {
                self.advance();
                let params = self.parse_params()?;
                self.expect_op(")")?;
                let _ = self.eat_ident("const");
                if !self.is_op("{") {
                    return Err(
                        "unsupported construct: method declaration without definition".to_string(),
                    );
                }
                let body = self.parse_block()?;
                methods.push(Method {
                    ret: ty,
                    name: member_name,
                    params,
                    body,
                });
                continue;
            }
            if self.is_op("=") {
                return Err("unsupported construct: field initializer".to_string());
            }
            let mut decls = vec![(member_name, ty.clone())];
            while self.eat_op(",") {
                let n = self.expect_ident()?;
                decls.push((n, ty.clone()));
            }
            self.expect_op(";")?;
            for (n, t) in decls {
                fields.push(Field { name: n, ty: t });
            }
        }
        self.expect_op(";")?;
        Ok(StructDef {
            name,
            fields,
            methods,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        if self.is_op(")") {
            return Ok(params);
        }
        if self.is_ident("void") && op_at(self.toks, self.pos + 1) == Some(")") {
            self.advance();
            return Ok(params);
        }
        loop {
            let ty = self.parse_type()?;
            let name = self.expect_ident()?;
            if self.eat_op("=") {
                return Err("unsupported construct: default argument".to_string());
            }
            params.push(Param { name, ty });
            if self.eat_op(",") {
                continue;
            }
            break;
        }
        Ok(params)
    }

    fn parse_type(&mut self) -> Result<CppType, String> {
        let mut is_const = false;
        while self.eat_ident("const") {
            is_const = true;
        }
        let mut unsigned = false;
        let mut signed = false;
        if self.eat_ident("unsigned") {
            unsigned = true;
        } else if self.eat_ident("signed") {
            signed = true;
        }
        let mut long_count = 0;
        while self.eat_ident("long") {
            long_count += 1;
        }
        let mut short = false;
        if self.eat_ident("short") {
            short = true;
        }
        let has_modifier = unsigned || signed || long_count > 0 || short;

        let base = if self.eat_ident("int") {
            if unsigned {
                if long_count > 0 {
                    BaseType::U64
                } else {
                    BaseType::U32
                }
            } else if long_count > 0 {
                BaseType::I64
            } else if short {
                BaseType::I16
            } else {
                BaseType::I32
            }
        } else if self.eat_ident("char") {
            if unsigned {
                BaseType::U8
            } else if signed {
                BaseType::I8
            } else {
                BaseType::Char
            }
        } else if self.eat_ident("bool") {
            BaseType::Bool
        } else if self.eat_ident("float") {
            BaseType::F32
        } else if self.eat_ident("double") {
            BaseType::F64
        } else if self.eat_ident("void") {
            BaseType::Void
        } else if self.eat_ident("size_t") {
            BaseType::Usize
        } else if self.is_ident("std")
            && op_at(self.toks, self.pos + 1) == Some("::")
            && ident_at(self.toks, self.pos + 2) == Some("string")
        {
            self.pos += 3;
            BaseType::String
        } else if has_modifier {
            if unsigned {
                if long_count > 0 {
                    BaseType::U64
                } else {
                    BaseType::U32
                }
            } else if long_count > 0 {
                BaseType::I64
            } else if short {
                BaseType::I16
            } else {
                BaseType::I32
            }
        } else if let Some(name) = self.peek_ident().map(str::to_string) {
            if is_reserved(&name) {
                return Err(format!("expected a type, found keyword '{}'", name));
            }
            self.advance();
            BaseType::Named(name)
        } else {
            return Err(format!(
                "expected a type, found {:?}",
                self.toks.get(self.pos)
            ));
        };

        let mut pointers = 0usize;
        let mut reference = false;
        loop {
            if self.eat_op("*") {
                pointers += 1;
            } else if self.eat_op("&") || self.eat_op("&&") {
                reference = true;
            } else {
                break;
            }
            while self.eat_ident("const") {}
        }
        Ok(CppType {
            base,
            is_const,
            pointers,
            reference,
        })
    }

    fn looks_like_decl(&self) -> bool {
        let mut i = self.pos;
        while ident_at(self.toks, i) == Some("const") {
            i += 1;
        }
        while matches!(
            ident_at(self.toks, i),
            Some("unsigned") | Some("signed") | Some("long") | Some("short")
        ) {
            i += 1;
        }
        if ident_at(self.toks, i) == Some("std")
            && op_at(self.toks, i + 1) == Some("::")
            && ident_at(self.toks, i + 2) == Some("string")
        {
            i += 3;
            while matches!(op_at(self.toks, i), Some("*") | Some("&") | Some("&&")) {
                i += 1;
            }
            return ident_at(self.toks, i).is_some();
        }
        if matches!(
            ident_at(self.toks, i),
            Some("int")
                | Some("char")
                | Some("bool")
                | Some("float")
                | Some("double")
                | Some("void")
                | Some("size_t")
        ) {
            i += 1;
            while matches!(op_at(self.toks, i), Some("*") | Some("&") | Some("&&")) {
                i += 1;
            }
            return ident_at(self.toks, i).is_some();
        }
        if let Some(name) = ident_at(self.toks, i) {
            if is_reserved(name) {
                return false;
            }
            if op_at(self.toks, i + 1) == Some("::") {
                return false;
            }
            let mut j = i + 1;
            while matches!(op_at(self.toks, j), Some("*") | Some("&") | Some("&&")) {
                j += 1;
            }
            return ident_at(self.toks, j).is_some();
        }
        false
    }

    fn parse_block(&mut self) -> Result<Stmt, String> {
        self.expect_op("{")?;
        let mut stmts = Vec::new();
        while !self.eat_op("}") {
            if self.at_end() {
                return Err("unexpected end of input in block".to_string());
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(Stmt::Block(stmts))
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if self.is_ident("using") {
            self.advance();
            if self.eat_ident("namespace") {
                let _ = self.expect_ident()?;
                self.expect_op(";")?;
                return Ok(Stmt::Empty);
            }
            return Err("unsupported construct: using declaration".to_string());
        }
        if self.is_ident("if") {
            return self.parse_if();
        }
        if self.is_ident("while") {
            return self.parse_while();
        }
        if self.is_ident("for") {
            return self.parse_for();
        }
        if self.is_ident("return") {
            self.advance();
            if self.eat_op(";") {
                return Ok(Stmt::Return(None));
            }
            let e = self.parse_assignment()?;
            self.expect_op(";")?;
            return Ok(Stmt::Return(Some(e)));
        }
        if self.is_ident("break") {
            self.advance();
            self.expect_op(";")?;
            return Ok(Stmt::Break);
        }
        if self.is_ident("continue") {
            self.advance();
            self.expect_op(";")?;
            return Ok(Stmt::Continue);
        }
        if self.is_ident("struct") || self.is_ident("class") {
            return Err("unsupported construct: nested type".to_string());
        }
        if self.is_op("{") {
            return self.parse_block();
        }
        if self.eat_op(";") {
            return Ok(Stmt::Empty);
        }
        if self.is_std("cout") {
            return self.parse_cout();
        }
        if self.is_std("cin") {
            let vars = self.parse_cin_vars()?;
            self.expect_op(";")?;
            return Ok(Stmt::Cin { vars, guard: None });
        }
        if self.looks_like_decl() {
            return self.parse_decl();
        }
        let e = self.parse_assignment()?;
        self.expect_op(";")?;
        Ok(Stmt::Expr(e))
    }

    fn parse_decl(&mut self) -> Result<Stmt, String> {
        let ty = self.parse_type()?;
        let mut decls = Vec::new();
        loop {
            let name = self.expect_ident()?;
            if self.eat_op("[") {
                return Err("unsupported construct: array declaration".to_string());
            }
            let init = if self.eat_op("=") {
                Some(self.parse_assignment()?)
            } else if self.eat_op("{") {
                let mut args = Vec::new();
                if !self.eat_op("}") {
                    loop {
                        args.push(self.parse_assignment()?);
                        if self.eat_op(",") {
                            if self.eat_op("}") {
                                break;
                            }
                            continue;
                        }
                        self.expect_op("}")?;
                        break;
                    }
                }
                Some(Expr::InitList(args))
            } else {
                None
            };
            decls.push(Declarator {
                name,
                ty: ty.clone(),
                init,
                is_const: ty.is_const,
            });
            if self.eat_op(",") {
                continue;
            }
            break;
        }
        self.expect_op(";")?;
        Ok(Stmt::Decl(decls))
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.expect_op("(")?;
        // Guard form: if (!(std::cin >> a >> b)) <body>
        if op_at(self.toks, self.pos) == Some("!")
            && op_at(self.toks, self.pos + 1) == Some("(")
            && ident_at(self.toks, self.pos + 2) == Some("std")
            && op_at(self.toks, self.pos + 3) == Some("::")
            && ident_at(self.toks, self.pos + 4) == Some("cin")
        {
            self.pos += 2;
            let vars = self.parse_cin_vars()?;
            self.expect_op(")")?;
            self.expect_op(")")?;
            let body = self.parse_stmt()?;
            return Ok(Stmt::Cin {
                vars,
                guard: Some(Box::new(body)),
            });
        }
        if self.is_std("cin") {
            return Err("unsupported construct: std::cin used as a truth value".to_string());
        }
        let cond = self.parse_assignment()?;
        self.expect_op(")")?;
        let then = Box::new(self.parse_stmt()?);
        let els = if self.eat_ident("else") {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(Stmt::If { cond, then, els })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.expect_op("(")?;
        let cond = self.parse_assignment()?;
        self.expect_op(")")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { cond, body })
    }

    fn for_header_is_range(&self) -> bool {
        let mut depth = 0usize;
        let mut j = self.pos;
        while j < self.toks.len() {
            match op_at(self.toks, j) {
                Some("(") => depth += 1,
                Some(")") => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                Some(";") if depth == 0 => return false,
                Some(":") if depth == 0 => return true,
                _ => {}
            }
            j += 1;
        }
        false
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        self.advance();
        self.expect_op("(")?;
        if self.for_header_is_range() {
            return Err("unsupported construct: range-based for loop".to_string());
        }
        let init = if self.eat_op(";") {
            None
        } else if self.looks_like_decl() {
            Some(Box::new(self.parse_decl()?))
        } else {
            let e = self.parse_assignment()?;
            self.expect_op(";")?;
            Some(Box::new(Stmt::Expr(e)))
        };
        let cond = if self.eat_op(";") {
            None
        } else {
            let e = self.parse_assignment()?;
            self.expect_op(";")?;
            Some(e)
        };
        let step = if self.eat_op(")") {
            None
        } else {
            let e = self.parse_assignment()?;
            self.expect_op(")")?;
            Some(e)
        };
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For {
            init,
            cond,
            step,
            body,
        })
    }

    fn parse_cin_vars(&mut self) -> Result<Vec<String>, String> {
        // assumes position at std::cin
        self.pos += 3;
        let mut vars = Vec::new();
        while self.eat_op(">>") {
            vars.push(self.expect_ident()?);
        }
        if vars.is_empty() {
            return Err("unsupported construct: empty std::cin chain".to_string());
        }
        Ok(vars)
    }

    fn parse_cout(&mut self) -> Result<Stmt, String> {
        self.pos += 3;
        let mut args = Vec::new();
        let mut newline = false;
        let saved_no_shift = self.no_shift;
        self.no_shift = true;
        let result = loop {
            if !self.eat_op("<<") {
                break Ok(());
            }
            if self.is_std("endl") {
                self.pos += 3;
                newline = true;
                continue;
            }
            match self.parse_assignment() {
                Ok(e) => args.push(e),
                Err(e) => break Err(e),
            }
        };
        self.no_shift = saved_no_shift;
        result?;
        self.expect_op(";")?;
        if let Some(Expr::Str(raw)) = args.last() {
            if decode_string_literal(raw).ends_with('\n') {
                newline = true;
            }
        }
        Ok(Stmt::Cout { args, newline })
    }

    // ----- expressions -----------------------------------------------------

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_ternary()?;
        if let Some(op) = self.peek_op() {
            if matches!(op, "=" | "+=" | "-=" | "*=" | "/=" | "%=") {
                let op = op.to_string();
                self.advance();
                let rhs = self.parse_assignment()?;
                return Ok(Expr::Assign(Box::new(lhs), op, Box::new(rhs)));
            }
        }
        Ok(lhs)
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let cond = self.parse_binary(0)?;
        if self.eat_op("?") {
            let a = self.parse_assignment()?;
            self.expect_op(":")?;
            let b = self.parse_assignment()?;
            return Ok(Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b)));
        }
        Ok(cond)
    }

    fn parse_binary(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        while let Some(op) = self.peek_op() {
            let op = op.to_string();
            if self.no_shift && (op == "<<" || op == ">>") {
                break;
            }
            let (lbp, rbp) = match infix_bp(&op) {
                Some(x) => x,
                None => break,
            };
            if lbp < min_bp {
                break;
            }
            self.advance();
            let rhs = self.parse_binary(rbp)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if let Some(op) = self.peek_op() {
            if matches!(op, "!" | "-" | "+" | "~") {
                let op = op.to_string();
                self.advance();
                let inner = self.parse_unary()?;
                return Ok(Expr::Unary(op, Box::new(inner)));
            }
            if matches!(op, "++" | "--") {
                let delta = if op == "++" { 1 } else { -1 };
                self.advance();
                let inner = self.parse_unary()?;
                return Ok(Expr::Inc(Box::new(inner), delta, true));
            }
            if matches!(op, "*" | "&") {
                return Err("unsupported construct: pointer dereference/address-of".to_string());
            }
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_primary()?;
        loop {
            if self.eat_op("(") {
                let mut args = Vec::new();
                if !self.eat_op(")") {
                    loop {
                        args.push(self.parse_assignment()?);
                        if self.eat_op(",") {
                            continue;
                        }
                        self.expect_op(")")?;
                        break;
                    }
                }
                e = Expr::Call(Box::new(e), args);
            } else if self.eat_op("[") {
                let idx = self.parse_assignment()?;
                self.expect_op("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else if self.eat_op(".") || self.eat_op("->") {
                let name = self.expect_ident()?;
                e = Expr::Member(Box::new(e), name);
            } else if self.eat_op("++") {
                e = Expr::Inc(Box::new(e), 1, false);
            } else if self.eat_op("--") {
                e = Expr::Inc(Box::new(e), -1, false);
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.toks.get(self.pos).cloned() {
            Some(Tok::Ident(name)) => {
                self.advance();
                Ok(Expr::Ident(name))
            }
            Some(Tok::Num(n)) => {
                self.advance();
                Ok(Expr::Num(n))
            }
            Some(Tok::Str(s)) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Some(Tok::Chr(c)) => {
                self.advance();
                Ok(Expr::Chr(c))
            }
            Some(Tok::Op(op)) if op == "(" => {
                self.advance();
                let e = self.parse_assignment()?;
                self.expect_op(")")?;
                Ok(e)
            }
            Some(Tok::Op(op)) if op == "{" => {
                self.advance();
                let mut args = Vec::new();
                if !self.eat_op("}") {
                    loop {
                        args.push(self.parse_assignment()?);
                        if self.eat_op(",") {
                            if self.eat_op("}") {
                                break;
                            }
                            continue;
                        }
                        self.expect_op("}")?;
                        break;
                    }
                }
                Ok(Expr::InitList(args))
            }
            other => Err(format!("unexpected token in expression: {:?}", other)),
        }
    }
}

fn infix_bp(op: &str) -> Option<(u8, u8)> {
    match op {
        "||" => Some((1, 2)),
        "&&" => Some((3, 4)),
        "|" => Some((5, 6)),
        "^" => Some((7, 8)),
        "&" => Some((9, 10)),
        "==" | "!=" => Some((11, 12)),
        "<" | "<=" | ">" | ">=" => Some((13, 14)),
        "<<" | ">>" => Some((15, 16)),
        "+" | "-" => Some((17, 18)),
        "*" | "/" | "%" => Some((19, 20)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

#[derive(Default)]
struct FnCtx {
    mutated: HashSet<String>,
    locals: HashSet<String>,
    var_types: HashMap<String, BaseType>,
    fields: HashSet<String>,
    is_main: bool,
    in_method: bool,
}

struct Codegen<'a> {
    program: &'a Program,
    structs: HashMap<String, Vec<Field>>,
    out: String,
}

impl<'a> Codegen<'a> {
    fn new(program: &'a Program) -> Self {
        let mut structs = HashMap::new();
        for item in &program.items {
            if let Item::Struct(s) = item {
                structs.insert(s.name.clone(), s.fields.clone());
            }
        }
        Self {
            program,
            structs,
            out: String::new(),
        }
    }

    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn generate(&mut self) -> Result<String, String> {
        let items = self.program.items.clone();
        for item in &items {
            match item {
                Item::Struct(s) => self.emit_struct(s)?,
                Item::Function(f) => self.emit_function(f)?,
            }
            self.out.push('\n');
        }
        Ok(format!("{}\n", self.out.trim_end()))
    }

    fn emit_struct(&mut self, s: &StructDef) -> Result<(), String> {
        self.line(0, "#[derive(Default)]");
        self.line(0, &format!("struct {} {{", s.name));
        for f in &s.fields {
            let ty = self.rust_type(&f.ty)?;
            self.line(1, &format!("{}: {},", f.name, ty));
        }
        self.line(0, "}");
        if !s.methods.is_empty() {
            self.line(0, "");
            self.line(0, &format!("impl {} {{", s.name));
            for m in &s.methods {
                self.emit_method(&s.name, m)?;
            }
            self.line(0, "}");
        }
        Ok(())
    }

    fn emit_method(&mut self, struct_name: &str, m: &Method) -> Result<(), String> {
        let mut ctx = FnCtx {
            in_method: true,
            fields: self
                .structs
                .get(struct_name)
                .map(|fs| fs.iter().map(|f| f.name.clone()).collect())
                .unwrap_or_default(),
            mutated: collect_mutations(&m.body),
            locals: collect_locals(&m.body),
            ..Default::default()
        };
        let self_mut = ctx.mutated.iter().any(|n| ctx.fields.contains(n));
        let mut params = vec![if self_mut {
            "&mut self".to_string()
        } else {
            "&self".to_string()
        }];
        for p in &m.params {
            let name = if ctx.mutated.contains(&p.name) {
                format!("mut {}", p.name)
            } else {
                p.name.clone()
            };
            params.push(format!("{}: {}", name, self.rust_param_type(&p.ty)?));
            ctx.var_types.insert(p.name.clone(), p.ty.base.clone());
            ctx.locals.insert(p.name.clone());
        }
        let ret = match m.ret.base {
            BaseType::Void => String::new(),
            _ => format!(" -> {}", self.rust_type(&m.ret)?),
        };
        self.line(
            1,
            &format!("fn {}({}){} {{", m.name, params.join(", "), ret),
        );
        self.emit_function_body(&m.body, 2, &mut ctx)?;
        self.line(1, "}");
        Ok(())
    }

    fn emit_function(&mut self, f: &FunctionDef) -> Result<(), String> {
        let is_main = f.name == "main";
        let mut ctx = FnCtx {
            is_main,
            mutated: collect_mutations(&f.body),
            locals: collect_locals(&f.body),
            ..Default::default()
        };
        let mut params = Vec::new();
        if !is_main {
            for p in &f.params {
                let name = if ctx.mutated.contains(&p.name) {
                    format!("mut {}", p.name)
                } else {
                    p.name.clone()
                };
                params.push(format!("{}: {}", name, self.rust_param_type(&p.ty)?));
                ctx.var_types.insert(p.name.clone(), p.ty.base.clone());
                ctx.locals.insert(p.name.clone());
            }
        }
        let ret = if is_main {
            String::new()
        } else {
            match f.ret.base {
                BaseType::Void => String::new(),
                _ => format!(" -> {}", self.rust_type(&f.ret)?),
            }
        };
        self.line(
            0,
            &format!("fn {}({}){} {{", f.name, params.join(", "), ret),
        );
        if stmt_contains_cin(&f.body) {
            if !is_main {
                return Err("unsupported construct: std::cin outside main".to_string());
            }
            self.emit_cin_preamble(1);
        }
        self.emit_function_body(&f.body, 1, &mut ctx)?;
        self.line(0, "}");
        Ok(())
    }

    fn emit_function_body(
        &mut self,
        body: &Stmt,
        indent: usize,
        ctx: &mut FnCtx,
    ) -> Result<(), String> {
        if let Stmt::Block(stmts) = body {
            let n = stmts.len();
            for (i, s) in stmts.iter().enumerate() {
                if ctx.is_main && i + 1 == n && is_return_zero(s) {
                    continue;
                }
                self.emit_stmt(s, indent, ctx)?;
            }
        } else {
            self.emit_stmt(body, indent, ctx)?;
        }
        Ok(())
    }

    fn emit_cin_preamble(&mut self, indent: usize) {
        self.line(indent, "let mut __input = String::new();");
        self.line(
            indent,
            "std::io::Read::read_to_string(&mut std::io::stdin(), &mut __input).ok();",
        );
        self.line(indent, "let mut __tokens = __input.split_whitespace();");
    }

    fn emit_decl(&mut self, d: &Declarator, indent: usize, ctx: &mut FnCtx) -> Result<(), String> {
        let rust_ty = self.rust_type(&d.ty)?;
        let is_mut = !d.is_const && ctx.mutated.contains(&d.name);
        let init = match &d.init {
            Some(Expr::InitList(args)) => self.emit_aggregate_init(&d.ty, args, ctx)?,
            Some(Expr::Str(raw)) if d.ty.base == BaseType::String => {
                format!("String::from({})", raw)
            }
            Some(e) => self.emit_expr(e, 0, ctx)?,
            None => self.default_value(&d.ty),
        };
        let kw = if is_mut { "let mut" } else { "let" };
        self.line(
            indent,
            &format!("{} {}: {} = {};", kw, d.name, rust_ty, init),
        );
        ctx.var_types.insert(d.name.clone(), d.ty.base.clone());
        ctx.locals.insert(d.name.clone());
        Ok(())
    }

    fn emit_aggregate_init(
        &self,
        ty: &CppType,
        args: &[Expr],
        ctx: &FnCtx,
    ) -> Result<String, String> {
        if let BaseType::Named(name) = &ty.base {
            let fields = self.structs.get(name).ok_or_else(|| {
                format!(
                    "unsupported construct: aggregate initialization of unknown struct {}",
                    name
                )
            })?;
            if fields.len() != args.len() {
                return Err(format!(
                    "unsupported construct: positional aggregate initialization for {}",
                    name
                ));
            }
            let mut parts = Vec::new();
            for (f, a) in fields.iter().zip(args.iter()) {
                let val = match a {
                    Expr::Str(raw) if f.ty.base == BaseType::String => {
                        format!("String::from({})", raw)
                    }
                    _ => self.emit_expr(a, 0, ctx)?,
                };
                parts.push(format!("{}: {}", f.name, val));
            }
            Ok(format!("{} {{ {} }}", name, parts.join(", ")))
        } else if args.len() == 1 {
            self.emit_expr(&args[0], 0, ctx)
        } else {
            Err("unsupported construct: brace initialization".to_string())
        }
    }

    fn default_value(&self, ty: &CppType) -> String {
        match &ty.base {
            BaseType::F32 | BaseType::F64 => "0.0".to_string(),
            BaseType::Bool => "false".to_string(),
            BaseType::Char => "'\\0'".to_string(),
            BaseType::String => "String::new()".to_string(),
            BaseType::Named(n) => format!("{}::default()", n),
            BaseType::Void => "()".to_string(),
            _ => "0".to_string(),
        }
    }

    fn rust_type(&self, ty: &CppType) -> Result<String, String> {
        if ty.pointers > 0 {
            return Err("unsupported construct: raw pointer type".to_string());
        }
        Ok(match &ty.base {
            BaseType::I8 => "i8",
            BaseType::I16 => "i16",
            BaseType::I32 => "i32",
            BaseType::I64 => "i64",
            BaseType::U8 => "u8",
            BaseType::U16 => "u16",
            BaseType::U32 => "u32",
            BaseType::U64 => "u64",
            BaseType::Usize => "usize",
            BaseType::Isize => "isize",
            BaseType::F32 => "f32",
            BaseType::F64 => "f64",
            BaseType::Bool => "bool",
            BaseType::Char => "char",
            BaseType::String => "String",
            BaseType::Void => "()",
            BaseType::Named(n) => n.as_str(),
        }
        .to_string())
    }

    fn rust_param_type(&self, ty: &CppType) -> Result<String, String> {
        if ty.reference && ty.base == BaseType::String {
            return Ok("&str".to_string());
        }
        self.rust_type(ty)
    }
}

impl<'a> Codegen<'a> {
    fn emit_stmt(&mut self, stmt: &Stmt, indent: usize, ctx: &mut FnCtx) -> Result<(), String> {
        match stmt {
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.emit_stmt(s, indent, ctx)?;
                }
            }
            Stmt::Empty => {}
            Stmt::Decl(decls) => {
                for d in decls {
                    self.emit_decl(d, indent, ctx)?;
                }
            }
            Stmt::Expr(e) => {
                let line = self.emit_expr_stmt(e, ctx)?;
                self.line(indent, &line);
            }
            Stmt::If { cond, then, els } => {
                let c = self.emit_condition(cond, ctx)?;
                self.line(indent, &format!("if {} {{", c));
                self.emit_stmt(then, indent + 1, ctx)?;
                if let Some(e) = els {
                    self.line(indent, "} else {");
                    self.emit_stmt(e, indent + 1, ctx)?;
                }
                self.line(indent, "}");
            }
            Stmt::While { cond, body } => {
                let c = self.emit_condition(cond, ctx)?;
                self.line(indent, &format!("while {} {{", c));
                self.emit_stmt(body, indent + 1, ctx)?;
                self.line(indent, "}");
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                self.emit_for(init, cond, step, body, indent, ctx)?;
            }
            Stmt::Return(e) => {
                if ctx.is_main {
                    match e {
                        None => self.line(indent, "return;"),
                        Some(Expr::Num(n)) if normalize_number(n) == "0" => {
                            self.line(indent, "return;")
                        }
                        Some(expr) => {
                            let s = self.emit_expr(expr, 0, ctx)?;
                            self.line(indent, &format!("std::process::exit(({}) as i32);", s));
                        }
                    }
                } else {
                    match e {
                        None => self.line(indent, "return;"),
                        Some(expr) => {
                            let s = self.emit_expr(expr, 0, ctx)?;
                            self.line(indent, &format!("return {};", s));
                        }
                    }
                }
            }
            Stmt::Break => self.line(indent, "break;"),
            Stmt::Continue => self.line(indent, "continue;"),
            Stmt::Cout { args, newline } => {
                let (fmt, fmt_args) = self.build_format(args, ctx)?;
                let mac = if *newline { "println" } else { "print" };
                if fmt_args.is_empty() {
                    self.line(indent, &format!("{}!(\"{}\");", mac, fmt));
                } else {
                    self.line(
                        indent,
                        &format!("{}!(\"{}\", {});", mac, fmt, fmt_args.join(", ")),
                    );
                }
            }
            Stmt::Cin { vars, guard } => {
                if let Some(g) = guard {
                    self.line(
                        indent,
                        &format!("if __tokens.clone().count() < {} {{", vars.len()),
                    );
                    self.emit_stmt(g, indent + 1, ctx)?;
                    self.line(indent, "}");
                }
                for v in vars {
                    if !ctx.var_types.contains_key(v) {
                        self.line(indent, &format!("let mut {}: i32 = 0;", v));
                        ctx.var_types.insert(v.clone(), BaseType::I32);
                        ctx.mutated.insert(v.clone());
                    }
                    let default = match ctx.var_types.get(v) {
                        Some(BaseType::F32) | Some(BaseType::F64) => "0.0",
                        Some(BaseType::Bool) => "false",
                        Some(BaseType::Char) => "'\\0'",
                        Some(BaseType::String) => "String::new()",
                        _ => "0",
                    };
                    self.line(
                        indent,
                        &format!(
                            "{} = __tokens.next().and_then(|t| t.parse().ok()).unwrap_or({});",
                            v, default
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn emit_for(
        &mut self,
        init: &Option<Box<Stmt>>,
        cond: &Option<Expr>,
        step: &Option<Expr>,
        body: &Stmt,
        indent: usize,
        ctx: &mut FnCtx,
    ) -> Result<(), String> {
        if self.try_emit_range_for(init, cond, step, body, indent, ctx)? {
            return Ok(());
        }
        self.line(indent, "{");
        if let Some(i) = init {
            self.emit_stmt(i, indent + 1, ctx)?;
        }
        let c = match cond {
            Some(e) => self.emit_condition(e, ctx)?,
            None => "true".to_string(),
        };
        self.line(indent + 1, &format!("while {} {{", c));
        self.emit_stmt(body, indent + 2, ctx)?;
        if let Some(s) = step {
            let line = self.emit_expr_stmt(s, ctx)?;
            self.line(indent + 2, &line);
        }
        self.line(indent + 1, "}");
        self.line(indent, "}");
        Ok(())
    }

    fn try_emit_range_for(
        &mut self,
        init: &Option<Box<Stmt>>,
        cond: &Option<Expr>,
        step: &Option<Expr>,
        body: &Stmt,
        indent: usize,
        ctx: &mut FnCtx,
    ) -> Result<bool, String> {
        let decls = match init.as_deref() {
            Some(Stmt::Decl(ds)) if ds.len() == 1 => ds,
            _ => return Ok(false),
        };
        let d = &decls[0];
        if !d.ty.base.is_integer() {
            return Ok(false);
        }
        let start = match &d.init {
            Some(e) => e,
            None => return Ok(false),
        };
        let var = d.name.clone();
        let (op, end) = match cond {
            Some(Expr::Binary(op, l, r)) if expr_is_ident(l, &var) && (op == "<" || op == "<=") => {
                (op.clone(), r.as_ref())
            }
            _ => return Ok(false),
        };
        let step_size = match step {
            Some(Expr::Inc(inner, 1, _)) if expr_is_ident(inner, &var) => None,
            Some(Expr::Assign(inner, op2, rhs)) if expr_is_ident(inner, &var) && op2 == "+=" => {
                match rhs.as_ref() {
                    Expr::Num(n) => {
                        let v = normalize_number(n);
                        if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) && v != "0" {
                            Some(v)
                        } else {
                            return Err(
                                "unsupported construct: for-loop step is not a positive integer literal"
                                    .to_string(),
                            );
                        }
                    }
                    _ => return Err(
                        "unsupported construct: for-loop step is not a positive integer literal"
                            .to_string(),
                    ),
                }
            }
            _ => return Ok(false),
        };
        let start_s = self.emit_expr(start, 0, ctx)?;
        let end_s = self.emit_expr(end, 0, ctx)?;
        let range = match step_size {
            Some(s) => {
                if op == "<" {
                    format!("({}..{}).step_by({} as usize)", start_s, end_s, s)
                } else {
                    format!("({}..={}).step_by({} as usize)", start_s, end_s, s)
                }
            }
            None => {
                if op == "<" {
                    format!("{}..{}", start_s, end_s)
                } else {
                    format!("{}..={}", start_s, end_s)
                }
            }
        };
        self.line(indent, &format!("for {} in {} {{", var, range));
        ctx.var_types.insert(var.clone(), d.ty.base.clone());
        ctx.locals.insert(var.clone());
        self.emit_stmt(body, indent + 1, ctx)?;
        self.line(indent, "}");
        Ok(true)
    }

    fn emit_expr_stmt(&self, e: &Expr, ctx: &FnCtx) -> Result<String, String> {
        match e {
            Expr::Inc(inner, d, _) => {
                let target = self.emit_expr(inner, 0, ctx)?;
                if *d >= 0 {
                    Ok(format!("{} += {};", target, d))
                } else {
                    Ok(format!("{} -= {};", target, -d))
                }
            }
            Expr::Assign(lhs, op, rhs) => {
                let l = self.emit_expr(lhs, 0, ctx)?;
                let r = self.emit_expr(rhs, 0, ctx)?;
                Ok(format!("{} {} {};", l, op, r))
            }
            _ => Ok(format!("{};", self.emit_expr(e, 0, ctx)?)),
        }
    }

    fn emit_expr(&self, e: &Expr, parent_prec: u8, ctx: &FnCtx) -> Result<String, String> {
        let own = prec_of(e);
        let s = match e {
            Expr::Ident(name) => {
                if ctx.in_method && ctx.fields.contains(name) && !ctx.locals.contains(name) {
                    format!("self.{}", name)
                } else {
                    name.clone()
                }
            }
            Expr::Num(n) => normalize_number(n),
            Expr::Str(raw) => raw.clone(),
            Expr::Chr(raw) => raw.clone(),
            Expr::Unary(op, inner) => {
                let child = self.emit_expr(inner, 13, ctx)?;
                format!("{}{}", op, child)
            }
            Expr::Binary(op, l, r) => {
                let left = self.emit_expr(l, own, ctx)?;
                let right = self.emit_expr(r, own + 1, ctx)?;
                format!("{} {} {}", left, op, right)
            }
            Expr::Ternary(c, a, b) => {
                let cond = self.emit_condition(c, ctx)?;
                let then = self.emit_expr(a, 0, ctx)?;
                let els = self.emit_expr(b, 0, ctx)?;
                format!("if {} {{ {} }} else {{ {} }}", cond, then, els)
            }
            Expr::Call(callee, args) => {
                let f = self.emit_expr(callee, 14, ctx)?;
                let mut out = Vec::new();
                for a in args {
                    out.push(self.emit_expr(a, 0, ctx)?);
                }
                format!("{}({})", f, out.join(", "))
            }
            Expr::Member(obj, name) => {
                let o = self.emit_expr(obj, 14, ctx)?;
                format!("{}.{}", o, name)
            }
            Expr::Index(obj, idx) => {
                let o = self.emit_expr(obj, 14, ctx)?;
                let i = self.emit_expr(idx, 0, ctx)?;
                format!("{}[{}]", o, i)
            }
            Expr::Inc(..) => {
                return Err(
                    "unsupported construct: increment/decrement inside an expression".to_string(),
                )
            }
            Expr::Assign(..) => {
                return Err("unsupported construct: assignment inside an expression".to_string())
            }
            Expr::InitList(..) => {
                return Err("unsupported construct: initializer list in expression".to_string())
            }
        };
        if own < parent_prec {
            Ok(format!("({})", s))
        } else {
            Ok(s)
        }
    }

    fn emit_condition(&self, e: &Expr, ctx: &FnCtx) -> Result<String, String> {
        match e {
            Expr::Binary(op, l, r) if op == "&&" || op == "||" => {
                let ls = self.emit_condition(l, ctx)?;
                let rs = self.emit_condition(r, ctx)?;
                Ok(format!("{} {} {}", ls, op, rs))
            }
            Expr::Unary(op, inner) if op == "!" => match inner.as_ref() {
                Expr::Ident(name) => match ctx.var_types.get(name) {
                    Some(t) if t.is_integer() => Ok(format!("{} == 0", name)),
                    Some(BaseType::F32) | Some(BaseType::F64) => Ok(format!("{} == 0.0", name)),
                    _ => {
                        let s = self.emit_condition(inner, ctx)?;
                        Ok(format!("!{}", s))
                    }
                },
                _ => {
                    let s = self.emit_condition(inner, ctx)?;
                    Ok(format!("!{}", s))
                }
            },
            Expr::Binary(op, _, _)
                if matches!(op.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=") =>
            {
                self.emit_expr(e, 0, ctx)
            }
            Expr::Ident(name) => match ctx.var_types.get(name) {
                Some(BaseType::Bool) => Ok(name.clone()),
                Some(BaseType::Char) => Ok(format!("{} != '\\0'", name)),
                Some(BaseType::F32) | Some(BaseType::F64) => Ok(format!("{} != 0.0", name)),
                Some(t) if t.is_integer() => Ok(format!("{} != 0", name)),
                _ => Ok(name.clone()),
            },
            Expr::Num(n) => {
                let s = normalize_number(n);
                if s == "0" || s == "0.0" {
                    Ok("false".to_string())
                } else {
                    Ok("true".to_string())
                }
            }
            _ => self.emit_expr(e, 0, ctx),
        }
    }

    fn build_format(&self, args: &[Expr], ctx: &FnCtx) -> Result<(String, Vec<String>), String> {
        let mut fmt = String::new();
        let mut fmt_args = Vec::new();
        let last = args.len().saturating_sub(1);
        for (i, a) in args.iter().enumerate() {
            match a {
                Expr::Str(raw) => {
                    let mut content = decode_string_literal(raw);
                    if i == last && content.ends_with('\n') {
                        content.pop();
                    }
                    let escaped = escape_rust_string(&content);
                    fmt.push_str(&escaped.replace('{', "{{").replace('}', "}}"));
                }
                _ => {
                    let rendered = self.emit_expr(a, 0, ctx)?;
                    if expr_is_bool_expr(a) {
                        fmt.push_str("{}");
                        fmt_args.push(format!("({}) as i32", rendered));
                    } else {
                        fmt.push_str("{}");
                        fmt_args.push(rendered);
                    }
                }
            }
        }
        Ok((fmt, fmt_args))
    }
}

fn expr_is_ident(e: &Expr, name: &str) -> bool {
    matches!(e, Expr::Ident(n) if n == name)
}

fn expr_is_bool_expr(e: &Expr) -> bool {
    match e {
        Expr::Binary(op, l, r) => {
            matches!(
                op.as_str(),
                "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
            ) || expr_is_bool_expr(l)
                || expr_is_bool_expr(r)
        }
        Expr::Unary(op, _) => op == "!",
        Expr::Ternary(_, _, _) => true,
        _ => false,
    }
}

fn prec_of(e: &Expr) -> u8 {
    match e {
        Expr::Assign(..) => 1,
        Expr::Ternary(..) => 2,
        Expr::Binary(op, _, _) => match op.as_str() {
            "||" => 3,
            "&&" => 4,
            "|" => 5,
            "^" => 6,
            "&" => 7,
            "==" | "!=" => 8,
            "<" | "<=" | ">" | ">=" => 9,
            "<<" | ">>" => 10,
            "+" | "-" => 11,
            "*" | "/" | "%" => 12,
            _ => 11,
        },
        Expr::Unary(..) => 13,
        Expr::Inc(_, _, prefix) => {
            if *prefix {
                13
            } else {
                14
            }
        }
        Expr::Call(..) | Expr::Member(..) | Expr::Index(..) => 14,
        _ => 15,
    }
}

// ---------------------------------------------------------------------------
// Analysis helpers
// ---------------------------------------------------------------------------

fn collect_mutations(stmt: &Stmt) -> HashSet<String> {
    let mut out = HashSet::new();
    walk_stmt_mutations(stmt, &mut out);
    out
}

fn walk_stmt_mutations(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Block(v) => {
            for s in v {
                walk_stmt_mutations(s, out);
            }
        }
        Stmt::Decl(ds) => {
            for d in ds {
                if let Some(e) = &d.init {
                    walk_expr_mutations(e, out);
                }
            }
        }
        Stmt::Expr(e) => walk_expr_mutations(e, out),
        Stmt::If { cond, then, els } => {
            walk_expr_mutations(cond, out);
            walk_stmt_mutations(then, out);
            if let Some(e) = els {
                walk_stmt_mutations(e, out);
            }
        }
        Stmt::While { cond, body } => {
            walk_expr_mutations(cond, out);
            walk_stmt_mutations(body, out);
        }
        Stmt::For {
            init,
            cond,
            step,
            body,
        } => {
            if let Some(i) = init {
                walk_stmt_mutations(i, out);
            }
            if let Some(c) = cond {
                walk_expr_mutations(c, out);
            }
            if let Some(s) = step {
                walk_expr_mutations(s, out);
            }
            walk_stmt_mutations(body, out);
        }
        Stmt::Return(Some(e)) => {
            walk_expr_mutations(e, out);
        }
        Stmt::Return(None) => {}
        Stmt::Cout { args, .. } => {
            for a in args {
                walk_expr_mutations(a, out);
            }
        }
        Stmt::Cin { vars, .. } => {
            for v in vars {
                out.insert(v.clone());
            }
        }
        _ => {}
    }
}

fn walk_expr_mutations(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Assign(lhs, _, rhs) => {
            mark_root(lhs, out);
            walk_expr_mutations(rhs, out);
        }
        Expr::Inc(inner, _, _) => mark_root(inner, out),
        Expr::Unary(_, x) => walk_expr_mutations(x, out),
        Expr::Binary(_, l, r) => {
            walk_expr_mutations(l, out);
            walk_expr_mutations(r, out);
        }
        Expr::Ternary(c, a, b) => {
            walk_expr_mutations(c, out);
            walk_expr_mutations(a, out);
            walk_expr_mutations(b, out);
        }
        Expr::Call(callee, args) => {
            walk_expr_mutations(callee, out);
            for a in args {
                walk_expr_mutations(a, out);
            }
        }
        Expr::Member(o, _) => walk_expr_mutations(o, out),
        Expr::Index(o, i) => {
            walk_expr_mutations(o, out);
            walk_expr_mutations(i, out);
        }
        Expr::InitList(v) => {
            for x in v {
                walk_expr_mutations(x, out);
            }
        }
        _ => {}
    }
}

fn mark_root(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Ident(n) => {
            out.insert(n.clone());
        }
        Expr::Member(o, _) => mark_root(o, out),
        Expr::Index(o, _) => mark_root(o, out),
        _ => {}
    }
}

fn collect_locals(stmt: &Stmt) -> HashSet<String> {
    let mut out = HashSet::new();
    walk_locals(stmt, &mut out);
    out
}

fn walk_locals(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Block(v) => {
            for s in v {
                walk_locals(s, out);
            }
        }
        Stmt::Decl(ds) => {
            for d in ds {
                out.insert(d.name.clone());
            }
        }
        Stmt::If { then, els, .. } => {
            walk_locals(then, out);
            if let Some(e) = els {
                walk_locals(e, out);
            }
        }
        Stmt::While { body, .. } => walk_locals(body, out),
        Stmt::For { init, body, .. } => {
            if let Some(i) = init {
                walk_locals(i, out);
            }
            walk_locals(body, out);
        }
        _ => {}
    }
}

fn stmt_contains_cin(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Cin { .. } => true,
        Stmt::Block(v) => v.iter().any(stmt_contains_cin),
        Stmt::If { then, els, .. } => {
            stmt_contains_cin(then) || els.as_ref().map(|e| stmt_contains_cin(e)).unwrap_or(false)
        }
        Stmt::While { body, .. } => stmt_contains_cin(body),
        Stmt::For { body, .. } => stmt_contains_cin(body),
        _ => false,
    }
}

fn is_return_zero(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::Return(Some(Expr::Num(n))) if normalize_number(n) == "0")
}

// ---------------------------------------------------------------------------
// Literal helpers
// ---------------------------------------------------------------------------

fn normalize_number(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    // Strip C++ integer/float suffixes.
    while s
        .chars()
        .last()
        .map(|c| matches!(c, 'u' | 'U' | 'l' | 'L' | 'f' | 'F'))
        .unwrap_or(false)
    {
        s.pop();
    }
    if s.starts_with("0x") || s.starts_with("0X") {
        return s;
    }
    if s.ends_with('.') {
        s.push('0');
    }
    if s.is_empty() {
        return "0".to_string();
    }
    s
}

fn decode_string_literal(raw: &str) -> String {
    let inner = if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('0') => out.push('\0'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn escape_rust_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn translated(source: &str) -> String {
        match translate_cpp_to_rust(source) {
            TranslationOutcome::Translated(s) => s,
            TranslationOutcome::Unsupported(reason) => panic!("unexpected unsupported: {}", reason),
        }
    }

    #[test]
    fn hello_world() {
        let src = "#include <iostream>\nint main() { std::cout << \"hello\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("println!(\"hello\")"), "output was:\n{}", out);
        assert!(!out.contains("std::"), "output was:\n{}", out);
    }

    #[test]
    fn arithmetic_input() {
        let src = "#include <iostream>\nint main() { int a, b; if (!(std::cin >> a >> b)) return 0; std::cout << (a + b * 2 - 1) << \"\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("let mut a: i32"), "output was:\n{}", out);
        assert!(out.contains("let mut b: i32"), "output was:\n{}", out);
        assert!(out.contains("__tokens"), "output was:\n{}", out);
        assert!(out.contains("println!"), "output was:\n{}", out);
        assert!(!out.contains("std::cin"), "output was:\n{}", out);
        assert!(!out.contains("int "), "output was:\n{}", out);
    }

    #[test]
    fn ternary() {
        let src = "#include <iostream>\nint main() { int a, b; if (!(std::cin >> a >> b)) return 0; int r = (a > b) ? a * 2 : b + 1; std::cout << r << \"\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("if a > b {"), "output was:\n{}", out);
        assert!(out.contains("else {"), "output was:\n{}", out);
    }

    #[test]
    fn for_loop_range() {
        let src = "#include <iostream>\nint main() { int n; if (!(std::cin >> n)) return 0; int r = 0; for (int i = 1; i <= n; i++) r += i; std::cout << r << \"\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("for i in 1..=n"), "output was:\n{}", out);
    }

    #[test]
    fn while_loop() {
        let src = "#include <iostream>\nint main() { int a, b; if (!(std::cin >> a >> b)) return 0; int x = a; int y = b; while (y != 0) { int t = y; y = x % y; x = t; } std::cout << x << \"\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("while y != 0"), "output was:\n{}", out);
    }

    #[test]
    fn char_declaration() {
        let src = "#include <iostream>\nint main() { char c = 'a'; std::cout << c << \"\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("let c: char = 'a'"), "output was:\n{}", out);
        assert!(out.contains("println!(\"{}\", c)"), "output was:\n{}", out);
    }

    #[test]
    fn using_namespace_is_stripped() {
        let src = "#include <iostream>\nusing namespace std;\nint main() { std::cout << \"hi\\n\"; return 0; }";
        let out = translated(src);
        assert!(out.contains("println!(\"hi\")"), "output was:\n{}", out);
        assert!(!out.contains("namespace"), "output was:\n{}", out);
    }

    #[test]
    fn unsupported_unordered_map() {
        let src =
            "#include <unordered_map>\nint main() { std::unordered_map<int, int> m; return 0; }";
        match translate_cpp_to_rust(src) {
            TranslationOutcome::Unsupported(reason) => {
                assert!(reason.contains("unordered_map"), "reason was: {}", reason);
            }
            TranslationOutcome::Translated(out) => panic!("expected unsupported, got:\n{}", out),
        }
    }
}
