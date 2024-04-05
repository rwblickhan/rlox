use crate::chunk::{Chunk, Opcode};
use crate::debug::disassemble_chunk;
use crate::memory::GarbageCollector;
use crate::object_function::{FunctionType, ObjFunction};
use crate::object_string::ObjString;
use crate::scanner::{Scanner, Token, TokenType};
use crate::value::Value;
use std::alloc::Layout;
use tinyvec::ArrayVec;

const MAX_LOCALS: usize = 256;

pub struct Compiler<'a> {
    current: Token<'a>,
    previous: Token<'a>,
    scanner: Scanner<'a>,
    had_error: bool,
    panic_mode: bool,
    compiler_states: Vec<CompilerState<'a>>,
    garbage_collector: &'a mut GarbageCollector,
}

pub struct CompilerState<'a> {
    locals: ArrayVec<[Local<'a>; MAX_LOCALS]>,
    scope_depth: i32,
    function: *mut ObjFunction,
}

impl CompilerState<'_> {
    pub fn new(function: *mut ObjFunction) -> CompilerState<'static> {
        let mut locals = ArrayVec::new();
        let name_local = Local {
            name: None,
            depth: 0,
        };
        locals.push(name_local);
        CompilerState {
            locals,
            scope_depth: 0,
            function,
        }
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }
}

#[derive(Default)]
pub struct Local<'a> {
    name: Option<Token<'a>>,
    depth: i32,
}

enum PrefixParserType {
    Grouping,
    Unary,
    Number,
    Literal,
    String,
    Variable,
}

enum InfixParserType {
    Binary,
    And,
    Or,
    Call,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum Precedence {
    None = 0,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl Precedence {
    fn next_level(self) -> Precedence {
        match self {
            Precedence::None => Precedence::Assignment,
            Precedence::Assignment => Precedence::Or,
            Precedence::Or => Precedence::And,
            Precedence::And => Precedence::Equality,
            Precedence::Equality => Precedence::Comparison,
            Precedence::Comparison => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::Primary,
        }
    }
}

impl<'a> Compiler<'a> {
    pub fn new(source: &'a str, garbage_collector: &'a mut GarbageCollector) -> Compiler<'a> {
        let mut scanner = Scanner::new(source);
        let starting_token = Compiler::advance_to_start(&mut scanner);
        let function = garbage_collector.heap_alloc(ObjFunction::new(FunctionType::Script, None));
        Compiler {
            current: starting_token,
            previous: starting_token,
            scanner,
            had_error: false,
            panic_mode: false,
            garbage_collector,
            compiler_states: vec![CompilerState::new(function)],
        }
    }

    // Parsing

    fn advance_to_start(scanner: &mut Scanner<'a>) -> Token<'a> {
        loop {
            let result = scanner.scan_token();
            match result {
                Ok(token) => return token,
                Err(err) => eprintln!("Error at first token: {err}"),
            }
        }
    }

    fn advance(&mut self) {
        self.previous = self.current;
        loop {
            let result = self.scanner.scan_token();
            match result {
                Ok(token) => {
                    self.current = token;
                    return;
                }
                Err(err) => self.error_at_current(err.to_string().as_ref()),
            }
        }
    }

    fn consume(&mut self, token_type: TokenType, message: &str) {
        if self.current.token_type == token_type {
            return self.advance();
        }
        self.error_at_current(message)
    }

    fn match_token(&mut self, token_type: TokenType) -> bool {
        if !self.check(token_type) {
            return false;
        };
        self.advance();
        true
    }

    fn check(&mut self, token_type: TokenType) -> bool {
        self.current.token_type == token_type
    }

    fn error_at_current(&mut self, message: &str) {
        self.error_at(self.current, message)
    }

    fn error(&mut self, message: &str) {
        self.error_at(self.previous, message)
    }

    fn error_at(&mut self, token: Token, message: &str) {
        if self.panic_mode {
            return;
        }

        eprint!("[line {}] Error", token.line);
        match token.token_type {
            TokenType::Eof => eprint!(" at end"),
            _ => eprint!(" at '{}'", token.source),
        }
        eprintln!(": {message}");
        self.had_error = true;
        self.panic_mode = true;
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn declaration(&mut self) {
        if self.match_token(TokenType::Fun) {
            self.fun_declaration();
        } else if self.match_token(TokenType::Var) {
            self.var_declaration();
        } else {
            self.statement();
        }
        if self.panic_mode {
            self.synchronize();
        }
    }

    fn fun_declaration(&mut self) {
        let global = self.parse_variable("Expect function name.");
        self.mark_initialized();
        self.function();
        self.define_variable(global);
    }

    fn function(&mut self) {
        let function = self
            .garbage_collector
            .heap_alloc(ObjFunction::new(FunctionType::Function, None));
        unsafe {
            (*function).name = Some(ObjString::new(self.previous.source));
        }
        let compiler_state = CompilerState::new(function);
        self.compiler_states.push(compiler_state);

        self.current_compiler_state_mut().begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        if !self.check(TokenType::RightParen) {
            loop {
                unsafe {
                    (*(self.current_compiler_state_mut().function)).arity += 1;
                    let constant = self.parse_variable("Expect parameter name.");
                    self.define_variable(constant);
                    if !self.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.");
        self.consume(TokenType::LeftBrace, "Expect '{' before function body.");
        self.block();

        let function = self.end_compiler(false);
        let constant = self.make_constant(Value::ObjFunction(function));
        self.emit_bytes(Opcode::Closure as u8, constant);
    }

    fn var_declaration(&mut self) {
        let global = self.parse_variable("Expect variable name.");

        if self.match_token(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_byte(Opcode::Nil as u8);
        }
        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        );

        self.define_variable(global);
    }

    fn parse_variable(&mut self, error_message: &str) -> u8 {
        self.consume(TokenType::Identifier, error_message);
        self.declare_variable();
        if self.current_compiler_state().scope_depth > 0 {
            // We're handling a local; don't load the identifier into the
            // constant table and return a dummy location
            return 0;
        }
        self.identifier_constant(self.previous.source)
    }

    fn declare_variable(&mut self) {
        if self.current_compiler_state().scope_depth == 0 {
            return;
        };
        let name = self.previous;
        let mut has_error = false;
        for local in self.current_compiler_state().locals.iter().rev() {
            if local.depth != -1 && local.depth < self.current_compiler_state().scope_depth {
                break;
            }
            if local.name == Some(name) {
                has_error = true;
            }
        }
        if has_error {
            self.error("Already a variable with this name in this scope.");
        }
        if self.current_compiler_state().locals.len() > MAX_LOCALS {
            self.error("Too many local variables in function.");
            return;
        }
        let current_compiler_state = self.current_compiler_state_mut();
        current_compiler_state.locals.push(Local {
            name: Some(name),
            depth: -1,
        });
    }

    fn identifier_constant(&mut self, name: &str) -> u8 {
        let obj_str = self.garbage_collector.heap_alloc(ObjString::new(name));
        self.make_constant(Value::ObjString(obj_str))
    }

    fn define_variable(&mut self, global: u8) {
        if self.current_compiler_state().scope_depth > 0 {
            self.mark_initialized();
            // We're handling a local; don't emit `DefineGlobal`
            return;
        }
        self.emit_bytes(Opcode::DefineGlobal as u8, global);
    }

    fn mark_initialized(&mut self) {
        if self.current_compiler_state().scope_depth == 0 {
            return;
        }
        let slot = self.current_compiler_state().locals.len() - 1;
        self.current_compiler_state_mut().locals[slot].depth =
            self.current_compiler_state().scope_depth;
    }

    fn statement(&mut self) {
        if self.match_token(TokenType::Print) {
            self.print_statement();
        } else if self.match_token(TokenType::For) {
            self.for_statement();
        } else if self.match_token(TokenType::If) {
            self.if_statement();
        } else if self.match_token(TokenType::Return) {
            self.return_statement();
        } else if self.match_token(TokenType::While) {
            self.while_statement();
        } else if self.match_token(TokenType::LeftBrace) {
            self.current_compiler_state_mut().begin_scope();
            self.block();
            self.end_scope();
        } else {
            self.expression_statement();
        }
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after print expression.");
        self.emit_byte(Opcode::Print as u8);
    }

    fn for_statement(&mut self) {
        self.current_compiler_state_mut().begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.");
        if self.match_token(TokenType::Semicolon) {
            // No initializer!
        } else if self.match_token(TokenType::Var) {
            self.var_declaration();
        } else {
            self.expression_statement();
        }

        let mut loop_start = self.current_chunk().code.len();
        let mut exit_jump = None;
        if !self.match_token(TokenType::Semicolon) {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");
            exit_jump = Some(self.emit_jump(Opcode::JumpIfFalse));
            self.emit_byte(Opcode::Pop as u8);
        }

        if !self.match_token(TokenType::RightParen) {
            let body_jump = self.emit_jump(Opcode::Jump);
            let increment_start = self.current_chunk().code.len();
            self.expression();
            self.emit_byte(Opcode::Pop as u8);
            self.consume(TokenType::RightParen, "Expect ')' after for clauses.");

            self.emit_loop(loop_start);
            loop_start = increment_start;
            self.patch_jump(body_jump);
        }

        self.statement();
        self.emit_loop(loop_start);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit_byte(Opcode::Pop as u8);
        }

        self.end_scope();
    }

    fn if_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");

        let then_jump = self.emit_jump(Opcode::JumpIfFalse);
        self.emit_byte(Opcode::Pop as u8);
        self.statement();
        let else_jump = self.emit_jump(Opcode::Jump);
        self.patch_jump(then_jump);
        self.emit_byte(Opcode::Pop as u8);
        if self.match_token(TokenType::Else) {
            self.statement();
        }
        self.patch_jump(else_jump);
    }

    fn return_statement(&mut self) {
        if let FunctionType::Script =
            unsafe { (*self.current_compiler_state().function).function_type }
        {
            self.error("Can't return from top-level code.");
            return;
        }
        if self.match_token(TokenType::Semicolon) {
            self.emit_return();
        } else {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after return value.");
            self.emit_byte(Opcode::Return as u8);
        }
    }

    fn while_statement(&mut self) {
        let loop_start = self.current_chunk().code.len();
        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");

        let exit_jump = self.emit_jump(Opcode::JumpIfFalse);
        self.emit_byte(Opcode::Pop as u8);
        self.statement();
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        self.emit_byte(Opcode::Pop as u8);
    }

    fn emit_loop(&mut self, loop_start: usize) {
        self.emit_byte(Opcode::Loop as u8);
        let offset = self.current_chunk().code.len() - loop_start + 2;
        if offset > u16::MAX as usize {
            self.error("Loop body too large.");
        }

        self.emit_byte((offset as u16 >> 8 & 0xff) as u8);
        self.emit_byte((offset & 0xff) as u8)
    }

    fn emit_jump(&mut self, opcode: Opcode) -> usize {
        self.emit_byte(opcode as u8);
        self.emit_byte(0xff);
        self.emit_byte(0xff);
        self.current_chunk().code.len() - 2
    }

    fn patch_jump(&mut self, offset: usize) {
        // -2 to adjust for the bytecode for the jump offset itself
        let jump = self.current_chunk().code.len() - offset - 2;

        let jump: u16 = match jump.try_into() {
            Ok(jump) => jump,
            Err(_) => {
                self.error("Too much code to jump over.");
                0
            }
        };

        self.current_chunk().code[offset] = (jump >> 8) as u8;
        self.current_chunk().code[offset + 1] = jump as u8;
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")
    }

    fn end_scope(&mut self) {
        self.current_compiler_state_mut().scope_depth -= 1;
        for i in (0..(self.current_compiler_state().locals.len())).rev() {
            let local = &self.current_compiler_state().locals[i];
            if local.depth > self.current_compiler_state().scope_depth {
                self.emit_byte(Opcode::Pop as u8);
                self.current_compiler_state_mut().locals.pop();
            }
        }
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(
            TokenType::Semicolon,
            "Expect ';' after expression statement expression.",
        );
        self.emit_byte(Opcode::Pop as u8);
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;

        while self.current.token_type != TokenType::Eof {
            if self.previous.token_type == TokenType::Semicolon {
                return;
            }
            match self.current.token_type {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => (),
            }
            self.advance();
        }
    }

    fn number(&mut self) {
        let value = self.previous.source.parse::<f64>().unwrap();
        self.emit_constant(Value::Number(value));
    }

    fn literal(&mut self) {
        match self.previous.token_type {
            TokenType::False => self.emit_byte(Opcode::False as u8),
            TokenType::Nil => self.emit_byte(Opcode::Nil as u8),
            TokenType::True => self.emit_byte(Opcode::True as u8),
            _ => self.error("Expect literal."),
        }
    }

    fn string(&mut self) {
        // Trim the leading and trailing quotes
        let string = &self.previous.source[1..self.previous.source.len() - 1];
        let obj = ObjString::new(string);
        let layout = Layout::new::<ObjString>();
        unsafe {
            // This will never be garbage collected, but that's okay, because it's a constant
            let ptr = std::alloc::alloc(layout) as *mut ObjString;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            *ptr = obj;
            self.emit_constant(Value::ObjString(ptr));
        }
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(self.previous, can_assign);
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let arg = match self.resolve_local(self.current_compiler_state(), name) {
            Ok(arg) => arg,
            Err(err) => {
                self.error(err.as_str());
                0
            }
        };
        let (set_op, get_op, arg) = if arg != -1 {
            (Opcode::SetLocal, Opcode::GetLocal, arg as u8)
        } else {
            (
                Opcode::SetGlobal,
                Opcode::GetGlobal,
                self.identifier_constant(name.source),
            )
        };
        if self.match_token(TokenType::Equal) && can_assign {
            self.expression();
            self.emit_bytes(set_op as u8, arg);
        } else {
            self.emit_bytes(get_op as u8, arg);
        }
    }

    fn resolve_local(&self, compiler_state: &CompilerState, name: Token) -> Result<i32, String> {
        for i in (0..(compiler_state.locals.len())).rev() {
            let local = &compiler_state.locals[i];
            if Some(name) == local.name {
                if local.depth == -1 {
                    return Err("Can't read local variable in its own initializer.".to_string());
                }
                let i = i32::try_from(i);
                match i {
                    Ok(i) => return Ok(i),
                    Err(_) => return Err("Failed to convert to integer".to_string()),
                }
            }
        }

        Ok(-1)
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator_type = self.previous.token_type;
        self.parse_precedence(Precedence::Unary);
        match operator_type {
            TokenType::Minus => self.emit_byte(Opcode::Negate as u8),
            TokenType::Bang => self.emit_byte(Opcode::Not as u8),
            _ => self.error("Expect unary operator."),
        }
    }

    fn binary(&mut self) {
        let operator_type = self.previous.token_type;
        self.parse_precedence(operator_type.precedence().next_level());
        match operator_type {
            TokenType::Plus => self.emit_byte(Opcode::Add as u8),
            TokenType::Minus => self.emit_byte(Opcode::Subtract as u8),
            TokenType::Star => self.emit_byte(Opcode::Multiply as u8),
            TokenType::Slash => self.emit_byte(Opcode::Divide as u8),
            TokenType::BangEqual => self.emit_bytes(Opcode::Equal as u8, Opcode::Not as u8),
            TokenType::EqualEqual => self.emit_byte(Opcode::Equal as u8),
            TokenType::Greater => self.emit_byte(Opcode::Greater as u8),
            TokenType::GreaterEqual => self.emit_bytes(Opcode::Less as u8, Opcode::Not as u8),
            TokenType::Less => self.emit_byte(Opcode::Less as u8),
            TokenType::LessEqual => self.emit_bytes(Opcode::Greater as u8, Opcode::Not as u8),
            _ => self.error("Expect binary operator."),
        }
    }

    fn and(&mut self) {
        let jump = self.emit_jump(Opcode::JumpIfFalse);
        self.emit_byte(Opcode::Pop as u8);
        self.parse_precedence(Precedence::And);
        self.patch_jump(jump);
    }

    fn or(&mut self) {
        let else_jump = self.emit_jump(Opcode::JumpIfFalse);
        let end_jump = self.emit_jump(Opcode::Jump);

        self.patch_jump(else_jump);
        self.emit_byte(Opcode::Pop as u8);

        self.parse_precedence(Precedence::Or);
        self.patch_jump(end_jump);
    }

    fn call(&mut self) {
        let arg_count = self.argument_list();
        self.emit_bytes(Opcode::Call as u8, arg_count);
    }

    fn argument_list(&mut self) -> u8 {
        let mut arg_count = 0;
        if !self.check(TokenType::RightParen) {
            loop {
                self.expression();
                arg_count += 1;

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.");
        arg_count
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();

        match self.previous.token_type.prefix_parser_type() {
            Some(prefix_parser_type) => match prefix_parser_type {
                PrefixParserType::Grouping => self.grouping(),
                PrefixParserType::Unary => self.unary(),
                PrefixParserType::Number => self.number(),
                PrefixParserType::Literal => self.literal(),
                PrefixParserType::String => self.string(),
                PrefixParserType::Variable => self.variable(precedence <= Precedence::Assignment),
            },
            None => self.error("Expect expression with prefix parser."),
        }

        while precedence <= self.current.token_type.precedence() {
            self.advance();
            match self.previous.token_type.infix_parser_type() {
                Some(infix_parser_type) => match infix_parser_type {
                    InfixParserType::Binary => self.binary(),
                    InfixParserType::And => self.and(),
                    InfixParserType::Or => self.or(),
                    InfixParserType::Call => self.call(),
                },
                None => self.error("Expect expression with infix parser."),
            }
        }

        if self.match_token(TokenType::Equal) && precedence <= Precedence::Assignment {
            self.error("Invalid assignment target.");
        }
    }

    // Code generation

    fn current_chunk(&mut self) -> &mut Chunk {
        unsafe { &mut (*self.current_compiler_state().function).chunk }
    }

    fn current_compiler_state(&self) -> &CompilerState<'a> {
        self.compiler_states.last().unwrap()
    }

    fn current_compiler_state_mut(&mut self) -> &mut CompilerState<'a> {
        self.compiler_states.last_mut().unwrap()
    }

    pub fn compile(&mut self, debug_print_code: bool) -> Option<*const ObjFunction> {
        while !self.match_token(TokenType::Eof) {
            self.declaration();
        }
        self.consume(TokenType::Eof, "Expect end of expression.");
        let function = self.end_compiler(debug_print_code);
        if !self.had_error {
            Some(function)
        } else {
            None
        }
    }

    fn end_compiler(&mut self, debug_print_code: bool) -> *const ObjFunction {
        self.emit_return();
        if debug_print_code && !self.had_error {
            disassemble_chunk(self.current_chunk(), "code");
        }
        let function = self.current_compiler_state().function;
        self.compiler_states.pop();
        function
    }

    fn emit_byte(&mut self, byte: u8) {
        let line = self.previous.line;
        self.current_chunk().write_chunk(byte, line);
    }

    fn emit_bytes(&mut self, byte1: u8, byte2: u8) {
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    fn emit_return(&mut self) {
        self.emit_byte(Opcode::Nil as u8);
        self.emit_byte(Opcode::Return as u8);
    }

    fn emit_constant(&mut self, value: Value) {
        let constant = self.make_constant(value);
        self.emit_bytes(Opcode::Constant as u8, constant);
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        let constant = self.current_chunk().add_constant(value);
        if constant > u8::MAX as usize {
            self.error("Too many constants in one chunk.");
            return 0;
        }
        constant as u8
    }
}

impl TokenType {
    fn precedence(&self) -> Precedence {
        match self {
            TokenType::Minus => Precedence::Term,
            TokenType::Plus => Precedence::Term,
            TokenType::Slash => Precedence::Factor,
            TokenType::Star => Precedence::Factor,
            TokenType::Number => Precedence::None,
            TokenType::True => Precedence::None,
            TokenType::False => Precedence::None,
            TokenType::Bang => Precedence::None,
            TokenType::BangEqual => Precedence::Equality,
            TokenType::EqualEqual => Precedence::Equality,
            TokenType::Greater => Precedence::Comparison,
            TokenType::GreaterEqual => Precedence::Comparison,
            TokenType::Less => Precedence::Comparison,
            TokenType::LessEqual => Precedence::Comparison,
            TokenType::String => Precedence::None,
            TokenType::Identifier => Precedence::None,
            TokenType::And => Precedence::And,
            TokenType::Or => Precedence::Or,
            TokenType::LeftParen => Precedence::Call,
            _ => Precedence::None,
        }
    }

    fn prefix_parser_type(&self) -> Option<PrefixParserType> {
        match self {
            TokenType::LeftParen => Some(PrefixParserType::Grouping),
            TokenType::Minus => Some(PrefixParserType::Unary),
            TokenType::Number => Some(PrefixParserType::Number),
            TokenType::Nil => Some(PrefixParserType::Literal),
            TokenType::True => Some(PrefixParserType::Literal),
            TokenType::False => Some(PrefixParserType::Literal),
            TokenType::Bang => Some(PrefixParserType::Unary),
            TokenType::String => Some(PrefixParserType::String),
            TokenType::Identifier => Some(PrefixParserType::Variable),
            _ => None,
        }
    }

    fn infix_parser_type(&self) -> Option<InfixParserType> {
        match self {
            TokenType::LeftParen => Some(InfixParserType::Call),
            TokenType::Plus => Some(InfixParserType::Binary),
            TokenType::Minus => Some(InfixParserType::Binary),
            TokenType::Star => Some(InfixParserType::Binary),
            TokenType::Slash => Some(InfixParserType::Binary),
            TokenType::BangEqual => Some(InfixParserType::Binary),
            TokenType::EqualEqual => Some(InfixParserType::Binary),
            TokenType::Greater => Some(InfixParserType::Binary),
            TokenType::GreaterEqual => Some(InfixParserType::Binary),
            TokenType::Less => Some(InfixParserType::Binary),
            TokenType::LessEqual => Some(InfixParserType::Binary),
            TokenType::And => Some(InfixParserType::And),
            TokenType::Or => Some(InfixParserType::Or),
            _ => None,
        }
    }
}
