use crate::chunk::{Chunk, Opcode};
use crate::compiler;
use crate::debug;
use crate::object::{Obj, ObjString};
use crate::value::Value;
use std::rc::Rc;

const STACK_MAX: usize = 256;

pub struct VM {
    pub chunk: Chunk,
    pub ip: usize,
    pub stack: [Value; STACK_MAX],
    pub stack_top: usize,
    pub heap: Vec<Rc<Obj>>,
}

pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
}

macro_rules! binary_op {
    ($struct:expr, $op:tt, $value_converter:tt) => {
        let (Value::Number(_), Value::Number(_)) = ($struct.peek(0), $struct.peek(1)) else {
            $struct.runtime_error("Operands must be numbers.");
            return InterpretResult::RuntimeError;
        };
        let Value::Number(b) = $struct.pop_stack() else {
            return InterpretResult::RuntimeError;
        };
        let Value::Number(a) = $struct.pop_stack() else {
            return InterpretResult::RuntimeError;
        };
        $struct.push_stack($value_converter(a $op b));
    };
}

impl VM {
    pub fn new() -> VM {
        const ARRAY_REPEAT_VALUE: Value = Value::Number(0.0);
        VM {
            chunk: Chunk::new(),
            ip: 0,
            stack: [ARRAY_REPEAT_VALUE; STACK_MAX],
            stack_top: 0,
            heap: Vec::new(),
        }
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        let mut compiler = compiler::Compiler::new(source.as_str(), &mut self.chunk);
        if !compiler.compile(true) {
            return InterpretResult::CompileError;
        }

        self.run(true)
    }

    pub fn run(&mut self, debug_trace_execution: bool) -> InterpretResult {
        loop {
            let byte = self.read_byte();
            if let Ok(instruction) = Opcode::try_from(byte) {
                if debug_trace_execution {
                    print!("          ");
                    for slot in self.stack[0..self.stack_top].iter() {
                        print!("[ {slot} ]");
                    }
                    println!();
                    debug::disassemble_instruction(&instruction, &self.chunk, self.ip - 1);
                }
                match instruction {
                    Opcode::Constant => {
                        let constant = self.read_constant();
                        self.push_stack(constant);
                    }
                    Opcode::Negate => {
                        let value = self.peek(0);
                        match value {
                            Value::Number(number_value) => {
                                self.push_stack(Value::Number(-number_value));
                            }
                            _ => {
                                self.runtime_error("Operand must be a number.");
                                return InterpretResult::RuntimeError;
                            }
                        }
                    }
                    Opcode::Return => {
                        println!("{}", self.pop_stack());
                        return InterpretResult::Ok;
                    }
                    Opcode::Nil => {
                        self.push_stack(Value::Nil);
                    }
                    Opcode::True => {
                        self.push_stack(Value::Bool(true));
                    }
                    Opcode::False => {
                        self.push_stack(Value::Bool(false));
                    }
                    Opcode::Add => {
                        if let (Value::Obj(_), Value::Obj(_)) = (self.peek(0), self.peek(1)) {
                            match self.concatenate() {
                                Ok(_) => {}
                                Err(err) => return err,
                            }
                        } else {
                            binary_op!(self, +, (Value::to_number_value));
                        }
                    }
                    Opcode::Subtract => {
                        binary_op!(self, -, (Value::to_number_value));
                    }
                    Opcode::Multiply => {
                        binary_op!(self, *, (Value::to_number_value));
                    }
                    Opcode::Divide => {
                        binary_op!(self, /, (Value::to_number_value));
                    }
                    Opcode::Not => {
                        let value = self.pop_stack();
                        self.push_stack(Value::Bool(value.is_falsey()));
                    }
                    Opcode::Equal => {
                        let (a, b) = (self.pop_stack(), self.pop_stack());
                        // We should be interning string values for performance reasons
                        // to avoid walking the length of both strings in `==`,
                        // but that's a hassle, so I don't bother doing it here
                        self.push_stack(Value::Bool(a == b));
                    }
                    Opcode::Greater => {
                        binary_op!(self, >, (Value::to_bool_value));
                    }
                    Opcode::Less => {
                        binary_op!(self, <, (Value::to_bool_value));
                    }
                }
            }
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.chunk.code[self.ip];
        self.ip += 1;
        byte
    }

    fn read_constant(&mut self) -> Value {
        let constant = self.read_byte() as usize;
        self.chunk.constants[constant].clone()
    }

    fn push_stack(&mut self, value: Value) {
        self.stack[self.stack_top] = value;
        self.stack_top += 1;
    }

    fn pop_stack(&mut self) -> Value {
        self.stack_top -= 1;
        self.stack[self.stack_top].clone()
    }

    fn peek(&self, distance: usize) -> Value {
        self.stack[self.stack_top - 1 - distance].clone()
    }

    fn reset_stack(&mut self) {
        self.stack_top = 0;
    }

    fn runtime_error(&mut self, message: &str) {
        eprintln!("{message}");
        let instruction = self.ip - 1;
        let line = self.chunk.lines[instruction];
        eprintln!("[line {line}] in script");
        self.reset_stack();
    }

    fn heap_alloc(&mut self, obj: Obj) -> Rc<Obj> {
        let rc = Rc::new(obj);
        self.heap.push(rc.clone());
        rc
    }

    fn concatenate(&mut self) -> Result<(), InterpretResult> {
        let b = self.pop_stack();
        let a = self.pop_stack();
        let (Value::Obj(rc1), Value::Obj(rc2)) = (a, b) else {
            self.runtime_error("Concatenation operands must be objects.");
            return Err(InterpretResult::CompileError);
        };
        let obj1 = rc1.as_ref();
        let obj2 = rc2.as_ref();

        let (Obj::String(objString1), Obj::String(objString2)) = (obj1, obj2) else {
            self.runtime_error("Concatenation operands must be strings.");
            return Err(InterpretResult::CompileError);
        };

        let new_obj = self.heap_alloc(Obj::String(ObjString::new_from_string(format!(
            "{}{}",
            objString1, objString2
        ))));
        let new_value = Value::Obj(new_obj);
        self.push_stack(new_value);
        Ok(())
    }
}
