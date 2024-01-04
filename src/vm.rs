use crate::chunk::Chunk;
use crate::value::Value;
use crate::{debug, Opcode};

const STACK_MAX: usize = 256;

pub struct VM<'a> {
    pub chunk: &'a Chunk,
    pub ip: usize,
    pub stack: [Value; STACK_MAX],
    pub stack_top: usize,
}

pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
}

macro_rules! binary_op {
    ($struct:expr, $op:tt) => {
        let b = $struct.pop_stack();
        let a = $struct.pop_stack();
        $struct.push_stack(a $op b);
    };
}

impl VM<'_> {
    pub fn new(chunk: &Chunk) -> VM {
        VM {
            chunk,
            ip: 0,
            stack: [0.0; STACK_MAX],
            stack_top: 0,
        }
    }

    pub fn interpret(&mut self, debug_trace_execution: bool) -> InterpretResult {
        loop {
            let byte = self.read_byte();
            if let Ok(instruction) = Opcode::try_from(byte) {
                if debug_trace_execution {
                    print!("          ");
                    for slot in self.stack[0..self.stack_top].iter() {
                        print!("[ {slot} ]");
                    }
                    print!("\n");
                    debug::disassemble_instruction(&instruction, self.chunk, self.ip - 1);
                }
                match instruction {
                    Opcode::Constant => {
                        let constant = self.read_constant();
                        self.push_stack(constant);
                    }
                    Opcode::Negate => {
                        let value = -self.pop_stack();
                        self.push_stack(value);
                    }
                    Opcode::Return => {
                        println!("{}", self.pop_stack());
                        return InterpretResult::Ok;
                    }
                    Opcode::Add => {
                        binary_op!(self, +);
                    }
                    Opcode::Subtract => {
                        binary_op!(self, -);
                    }
                    Opcode::Multiply => {
                        binary_op!(self, *);
                    }
                    Opcode::Divide => {
                        binary_op!(self, /);
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
        self.chunk.constants[self.read_byte() as usize]
    }

    fn push_stack(&mut self, value: Value) {
        self.stack[self.stack_top] = value;
        self.stack_top += 1;
    }

    fn pop_stack(&mut self) -> Value {
        self.stack_top -= 1;
        self.stack[self.stack_top]
    }
}
