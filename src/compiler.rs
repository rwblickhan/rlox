use crate::chunk::{Chunk, Opcode};
use crate::debug::disassemble_chunk;
use crate::object::Obj;
use crate::scanner::{Scanner, Token, TokenType};
use crate::value::Value;
use std::alloc::Layout;

const MAX_LOCALS: usize = 256;

pub struct Compiler<'a> {
    compiling_chunk: &'a mut Chunk,
    current: Token<'a>,
    previous: Token<'a>,
    scanner: Scanner<'a>,
    had_error: bool,
    panic_mode: bool,
    compiler_state: CompilerState<'a>,
}

pub struct CompilerState<'a> {
    locals: [Option<Local<'a>>; MAX_LOCALS],
    local_count: usize,
    scope_depth: i32,
}

pub struct Local<'a> {
    name: Token<'a>,
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
    pub fn new(source: &'a str, compiling_chunk: &'a mut Chunk) -> Compiler<'a> {
        let mut scanner = Scanner::new(source);
        let starting_token = Compiler::advance_to_start(&mut scanner);
        const LOCAL_REPEAT_VALUE: Option<Local> = None;
        Compiler {
            compiling_chunk,
            current: starting_token,
            previous: starting_token,
            scanner,
            had_error: false,
            panic_mode: false,
            compiler_state: CompilerState {
                locals: [LOCAL_REPEAT_VALUE; MAX_LOCALS],
                local_count: 0,
                scope_depth: 0,
            },
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
        if self.match_token(TokenType::Var) {
            self.var_declaration();
        } else {
            self.statement();
        }
        if self.panic_mode {
            self.synchronize();
        }
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
        if self.compiler_state.scope_depth > 0 {
            // We're handling a local; don't load the identifier into the
            // constant table and return a dummy location
            return 0;
        }
        self.identifier_constant(self.previous.source)
    }

    fn declare_variable(&mut self) {
        if self.compiler_state.scope_depth == 0 {
            return;
        };
        let name = self.previous;
        let mut has_error = false;
        for local in self.compiler_state.locals.iter().rev() {
            let Some(local) = local else {
                continue;
            };
            if local.depth != -1 && local.depth < self.compiler_state.scope_depth {
                break;
            }
            if local.name == name {
                has_error = true;
            }
        }
        if has_error {
            self.error("Already a variable with this name in this scope.");
        }
        self.compiler_state.local_count += 1;
        if self.compiler_state.local_count > MAX_LOCALS {
            self.error("Too many local variables in function.");
            return;
        }
        self.compiler_state.locals[self.compiler_state.local_count - 1] =
            Some(Local { name, depth: -1 })
    }

    fn identifier_constant(&mut self, name: &str) -> u8 {
        let obj = Obj::new_from_string(name);
        let layout = Layout::new::<Obj>();
        unsafe {
            // This will never be garbage collected, but that's okay, because it's a constant
            let ptr = std::alloc::alloc(layout) as *mut Obj;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            *ptr = obj;
            self.make_constant(Value::Obj(ptr))
        }
    }

    fn define_variable(&mut self, global: u8) {
        if self.compiler_state.scope_depth > 0 {
            self.mark_initialized();
            // We're handling a local; don't emit `DefineGlobal`
            return;
        }
        self.emit_bytes(Opcode::DefineGlobal as u8, global);
    }

    fn mark_initialized(&mut self) {
        let slot = self.compiler_state.local_count - 1;
        self.compiler_state.locals[slot].as_mut().unwrap().depth = self.compiler_state.scope_depth;
    }

    fn statement(&mut self) {
        if self.match_token(TokenType::Print) {
            self.print_statement();
        } else if self.match_token(TokenType::LeftBrace) {
            self.begin_scope();
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

    fn begin_scope(&mut self) {
        self.compiler_state.scope_depth += 1;
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")
    }

    fn end_scope(&mut self) {
        self.compiler_state.scope_depth -= 1;
        let local_count = self.compiler_state.local_count;
        for i in (0..(local_count)).rev() {
            let Some(local) = &self.compiler_state.locals[i] else {
                continue;
            };

            if self.compiler_state.local_count > 0 && local.depth > self.compiler_state.scope_depth
            {
                self.emit_byte(Opcode::Pop as u8);
                self.compiler_state.local_count -= 1;
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
        let obj = Obj::new_from_string(string);
        let layout = Layout::new::<Obj>();
        unsafe {
            // This will never be garbage collected, but that's okay, because it's a constant
            let ptr = std::alloc::alloc(layout) as *mut Obj;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            *ptr = obj;
            self.emit_constant(Value::Obj(ptr));
        }
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(self.previous, can_assign);
    }

    fn named_variable(&mut self, name: Token, can_assign: bool) {
        let arg = match self.resolve_local(&self.compiler_state, name) {
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
        let local_count = compiler_state.local_count;
        for i in (0..(local_count)).rev() {
            let Some(local) = &compiler_state.locals[i] else {
                continue;
            };

            if name == local.name {
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
        self.compiling_chunk
    }

    pub fn compile(&mut self, debug_print_code: bool) -> bool {
        while !self.match_token(TokenType::Eof) {
            self.declaration();
        }
        self.consume(TokenType::Eof, "Expect end of expression.");
        self.end_compiler(debug_print_code);
        !self.had_error
    }

    fn end_compiler(&mut self, debug_print_code: bool) {
        self.emit_return();
        if debug_print_code && !self.had_error {
            disassemble_chunk(self.current_chunk(), "code")
        }
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
            _ => None,
        }
    }
}
