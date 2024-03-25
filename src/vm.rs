use crate::chunk::Opcode;
use crate::compiler;
use crate::debug;
use crate::memory::GarbageCollector;
use crate::object_function::ObjFunction;
use crate::object_string::ObjString;
use crate::value::Value;
use std::collections::HashMap;

const FRAMES_MAX: usize = 64;
const STACK_MAX: usize = FRAMES_MAX * 8;

pub struct VM<'a> {
    pub stack: [Value; STACK_MAX],
    pub stack_top: usize,
    pub globals: HashMap<String, Value>,
    pub garbage_collector: &'a mut GarbageCollector,
    pub frames: [Option<CallFrame>; FRAMES_MAX],
    pub frames_top: usize,
}

pub struct CallFrame {
    pub function: *const ObjFunction,
    pub ip: usize,
    pub first_slot: usize,
}

impl CallFrame {
    pub fn read_byte(&mut self) -> u8 {
        let byte = unsafe { (*self.function).chunk.code[self.ip] };
        self.ip += 1;
        byte
    }

    pub fn read_short(&mut self) -> u16 {
        (self.read_byte() as u16) << 8 | self.read_byte() as u16
    }

    pub fn read_constant(&mut self) -> Value {
        let constant = self.read_byte() as usize;
        unsafe { (*self.function).chunk.constants[constant].clone() }
    }

    fn read_string(&mut self) -> &str {
        let constant = self.read_constant();
        match constant {
            Value::ObjString(obj_str) => unsafe { &(*obj_str).str },
            _ => panic!("Not a string"),
        }
    }
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

impl<'a> VM<'a> {
    pub fn new(garbage_collector: &mut GarbageCollector) -> VM {
        const VALUE_ARRAY_REPEAT_VALUE: Value = Value::Number(0.0);
        const FRAME_ARRAY_REPEAT_VALUE: Option<CallFrame> = None;
        VM {
            stack: [VALUE_ARRAY_REPEAT_VALUE; STACK_MAX],
            stack_top: 0,
            globals: HashMap::new(),
            garbage_collector,
            frames: [FRAME_ARRAY_REPEAT_VALUE; FRAMES_MAX],
            frames_top: 0,
        }
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        let mut compiler = compiler::Compiler::new(source.as_str(), self.garbage_collector);
        match compiler.compile(true) {
            Some(function) => {
                self.push_stack(Value::ObjFunction(function));
                self.frames[self.frames_top] = Some(CallFrame {
                    function,
                    ip: 0,
                    first_slot: self.stack_top,
                })
            }
            None => return InterpretResult::CompileError,
        };

        self.run(false)
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
                    debug::disassemble_instruction(
                        &instruction,
                        unsafe {
                            &(*(self.frames[self.frames_top].as_mut().unwrap().function)).chunk
                        },
                        self.current_ip() - 1,
                    );
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
                        if let (Value::ObjString(_), Value::ObjString(_)) =
                            (self.peek(0), self.peek(1))
                        {
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
                    Opcode::Print => {
                        let value = self.pop_stack();
                        println!("{value}");
                    }
                    Opcode::Pop => {
                        self.pop_stack();
                    }
                    Opcode::DefineGlobal => {
                        let name = self.read_string().to_owned();
                        self.globals.insert(name, self.peek(0));
                        self.pop_stack();
                    }
                    Opcode::GetGlobal => {
                        let name = self.read_string().to_owned();
                        match self.globals.get(&name) {
                            Some(value) => self.push_stack(value.clone()),
                            None => {
                                self.runtime_error(format!("Undefined variable {name}.").as_str());
                                return InterpretResult::RuntimeError;
                            }
                        }
                    }
                    Opcode::SetGlobal => {
                        let name = self.read_string().to_owned();
                        match self.globals.insert(name.clone(), self.peek(0)) {
                            Some(_) => {}
                            None => {
                                self.globals.remove(&name);
                                self.runtime_error(
                                    format!("Undefined variable {}.", name.clone()).as_str(),
                                );
                                return InterpretResult::RuntimeError;
                            }
                        }
                    }
                    Opcode::GetLocal => {
                        let slot = self.read_slot();
                        self.push_stack(self.stack[slot].clone());
                    }
                    Opcode::SetLocal => {
                        let slot = self.read_slot();
                        self.push_stack(self.stack[slot].clone());
                        self.stack[slot] = self.peek(0);
                    }
                    Opcode::JumpIfFalse => {
                        let offset = self.read_short();
                        let is_falsey = self.peek(0).is_falsey();
                        if is_falsey {
                            self.inc_ip(offset as usize);
                        }
                    }
                    Opcode::Jump => {
                        let offset = self.read_short();
                        self.inc_ip(offset as usize);
                    }
                    Opcode::Loop => {
                        let offset = self.read_short();
                        self.dec_ip(offset as usize);
                    }
                }
            }
        }
    }

    fn read_byte(&mut self) -> u8 {
        self.frames[self.frames_top].as_mut().unwrap().read_byte()
    }

    fn read_short(&mut self) -> u16 {
        self.frames[self.frames_top].as_mut().unwrap().read_short()
    }

    fn read_constant(&mut self) -> Value {
        self.frames[self.frames_top]
            .as_mut()
            .unwrap()
            .read_constant()
    }

    fn read_string(&mut self) -> &str {
        self.frames[self.frames_top].as_mut().unwrap().read_string()
    }

    fn read_slot(&mut self) -> usize {
        let slot = self.read_byte() as usize;
        self.frames[self.frames_top].as_mut().unwrap().first_slot + slot
    }

    fn current_ip(&self) -> usize {
        self.frames[self.frames_top].as_ref().unwrap().ip
    }

    fn inc_ip(&mut self, offset: usize) {
        self.frames[self.frames_top].as_mut().unwrap().ip += offset
    }

    fn dec_ip(&mut self, offset: usize) {
        self.frames[self.frames_top].as_mut().unwrap().ip -= offset
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
        let instruction = self.current_ip() - 1;
        let line = unsafe {
            (*self.frames[self.frames_top].as_mut().unwrap().function)
                .chunk
                .lines[instruction]
        };
        eprintln!("[line {line}] in script");
        self.reset_stack();
    }

    fn concatenate(&mut self) -> Result<(), InterpretResult> {
        let b = self.pop_stack();
        let a = self.pop_stack();
        let (Value::ObjString(obj_str1), Value::ObjString(obj_str2)) = (a, b) else {
            self.runtime_error("Concatenation operands must be strings.");
            return Err(InterpretResult::CompileError);
        };

        unsafe {
            let str1 = &(*obj_str1).str;
            let str2 = &(*obj_str2).str;
            let new_obj = self
                .garbage_collector
                .heap_alloc(ObjString::new(format!("{}{}", str1, str2).as_str()));
            let new_value = Value::ObjString(new_obj);
            self.push_stack(new_value);
        }

        Ok(())
    }
}
