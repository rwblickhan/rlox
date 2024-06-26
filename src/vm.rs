use crate::chunk::Opcode;
use crate::compiler;
use crate::debug;
use crate::memory::Allocator;
use crate::memory::GC;
use crate::object_closure::ObjClosure;
use crate::object_native::NativeFunction;
use crate::object_native::ObjNative;
use crate::object_string::ObjString;
use crate::object_upvalue::ObjUpvalue;
use crate::value::Value;
use core::panic;
use std::collections::HashMap;
use tinyvec::ArrayVec;

const FRAMES_MAX: usize = 64;
const STACK_MAX: usize = FRAMES_MAX * 8;

pub struct VM<'a> {
    pub stack: [Value; STACK_MAX],
    pub stack_top: usize,
    pub globals: HashMap<String, Value>,
    pub allocator: &'a mut Allocator,
    pub frames: ArrayVec<[CallFrame; FRAMES_MAX]>,
    open_upvalues: Option<*mut ObjUpvalue>,
    debug_stress_gc: bool,
    debug_log_gc: bool,
}

pub struct CallFrame {
    pub closure: *mut ObjClosure,
    pub ip: usize,
    pub first_slot: usize,
}

impl Default for CallFrame {
    fn default() -> Self {
        CallFrame {
            closure: std::ptr::null_mut(),
            ip: 0,
            first_slot: 0,
        }
    }
}

impl CallFrame {
    pub fn read_byte(&mut self) -> u8 {
        let byte = unsafe { (*(*self.closure).function).chunk.code[self.ip] };
        self.ip += 1;
        byte
    }

    pub fn read_short(&mut self) -> u16 {
        (self.read_byte() as u16) << 8 | self.read_byte() as u16
    }

    pub fn read_constant(&mut self) -> Value {
        let constant = self.read_byte() as usize;
        unsafe { (*(*self.closure).function).chunk.constants[constant].clone() }
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
    pub fn new(allocator: &mut Allocator, debug_stress_gc: bool, debug_log_gc: bool) -> VM {
        const VALUE_ARRAY_REPEAT_VALUE: Value = Value::Number(0.0);
        VM {
            stack: [VALUE_ARRAY_REPEAT_VALUE; STACK_MAX],
            stack_top: 0,
            globals: HashMap::new(),
            allocator,
            frames: ArrayVec::new(),
            open_upvalues: None,
            debug_stress_gc,
            debug_log_gc,
        }
    }

    pub fn interpret(&mut self, source: String) -> InterpretResult {
        self.define_native("clock", NativeFunction::Clock);
        let mut compiler = compiler::Compiler::new(
            source.as_str(),
            self.allocator,
            self.debug_stress_gc,
            self.debug_log_gc,
        );
        compiler.prepare();
        match compiler.compile(true) {
            Some(function) => {
                self.push_stack(Value::ObjFunction(function));
                let obj_closure = self.allocator.heap_alloc(ObjClosure::new(function));
                self.pop_stack();
                self.push_stack(Value::ObjClosure(obj_closure));
                self.call(obj_closure, 0);
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
                        unsafe { &(*(*(self.frames.last_mut().unwrap().closure)).function).chunk },
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
                        let result = self.pop_stack();
                        let frame = self.frames.pop().unwrap();
                        self.close_upvalues(frame.first_slot);
                        if self.frames.is_empty() {
                            self.pop_stack();
                            return InterpretResult::Ok;
                        }
                        self.stack_top = frame.first_slot;
                        self.push_stack(result);
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
                    Opcode::Call => {
                        let arg_count = self.read_byte() as usize;
                        if !self.call_value(self.peek(arg_count), arg_count) {
                            return InterpretResult::RuntimeError;
                        }
                    }
                    Opcode::Closure => {
                        let Value::ObjFunction(obj_fun) = self.read_constant() else {
                            panic!("Invalid constant for Opcode::Closure");
                        };
                        let closure = self.heap_alloc(ObjClosure::new(obj_fun));
                        self.push_stack(Value::ObjClosure(closure));
                        let upvalue_count = unsafe { (*closure).upvalue_count };
                        for i in 0..upvalue_count {
                            let is_local = self.read_byte();
                            let index = self.read_byte();
                            let value = if is_local == 1 {
                                let location =
                                    self.frames.last().unwrap().first_slot + (index as usize);
                                self.capture_upvalue(location)
                            } else {
                                unsafe {
                                    (*self.frames.last().unwrap().closure).upvalues[index as usize]
                                }
                            };
                            unsafe { (*closure).upvalues[i] = value }
                        }
                    }
                    Opcode::GetUpvalue => {
                        let slot = self.read_byte() as usize;
                        unsafe {
                            let closure = self.frames.last().unwrap().closure.clone();
                            let upvalue = (*closure).upvalues[slot].clone();
                            match (*upvalue).closed.clone() {
                                Some(closed) => {
                                    self.push_stack(closed);
                                }
                                None => {
                                    let location = (*upvalue).location;
                                    let value = self.stack[location].clone();
                                    self.push_stack(value);
                                }
                            }
                        }
                    }
                    Opcode::SetUpvalue => {
                        let slot = self.read_byte() as usize;
                        let value = self.peek(0);
                        unsafe {
                            let closure = self.frames.last().unwrap().closure.clone();
                            let upvalue = (*closure).upvalues[slot].clone();
                            match (*upvalue).closed.clone() {
                                Some(_) => {
                                    (*upvalue).closed = Some(value);
                                }
                                None => {
                                    let location = (*upvalue).location;
                                    self.stack[location] = value;
                                }
                            }
                        }
                    }
                    Opcode::CloseUpvalue => {
                        self.close_upvalues(self.stack_top - 1);
                        self.pop_stack();
                    }
                }
            }
        }
    }

    fn capture_upvalue(&mut self, location: usize) -> *mut ObjUpvalue {
        // Search for an existing upvalue for this location
        let mut prev_upvalue: Option<*mut ObjUpvalue> = None;
        let mut upvalue = self.open_upvalues;
        while let Some(unwrap_upvalue) = upvalue {
            if unsafe { (*unwrap_upvalue).location } <= location {
                break;
            }
            prev_upvalue = Some(unwrap_upvalue);
            upvalue = unsafe { (*unwrap_upvalue).next_upvalue };
        }

        if let Some(upvalue) = upvalue {
            if unsafe { (*upvalue).location == location } {
                return upvalue;
            }
        }

        // If no existing upvalue, create a new one and insert it into the linked list
        let mut new_upvalue = ObjUpvalue::new(location);
        new_upvalue.next_upvalue = upvalue;
        let new_upvalue_ptr = self.heap_alloc(new_upvalue);
        match prev_upvalue {
            Some(prev_upvalue) => unsafe { (*prev_upvalue).next_upvalue = Some(new_upvalue_ptr) },
            None => self.open_upvalues = Some(new_upvalue_ptr),
        };
        new_upvalue_ptr
    }

    fn close_upvalues(&mut self, last_location: usize) {
        while let Some(upvalue) = self.open_upvalues {
            if unsafe { (*upvalue).location < last_location } {
                break;
            }
            unsafe {
                (*upvalue).closed = Some(self.stack[(*upvalue).location].clone());
                // TODO
                self.open_upvalues = (*upvalue).next_upvalue;
            }
        }
    }

    fn read_byte(&mut self) -> u8 {
        self.frames.last_mut().unwrap().read_byte()
    }

    fn read_short(&mut self) -> u16 {
        self.frames.last_mut().unwrap().read_short()
    }

    fn read_constant(&mut self) -> Value {
        self.frames.last_mut().unwrap().read_constant()
    }

    fn read_string(&mut self) -> &str {
        self.frames.last_mut().unwrap().read_string()
    }

    fn read_slot(&mut self) -> usize {
        let slot = self.read_byte() as usize;
        self.frames.last_mut().unwrap().first_slot + slot
    }

    fn current_ip(&mut self) -> usize {
        self.frames.last_mut().unwrap().ip
    }

    fn inc_ip(&mut self, offset: usize) {
        self.frames.last_mut().unwrap().ip += offset
    }

    fn dec_ip(&mut self, offset: usize) {
        self.frames.last_mut().unwrap().ip -= offset
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
        for frame in self.frames.iter().rev() {
            let function = unsafe { &(*(*frame.closure).function) };
            let instruction = frame.ip - 1;
            let line = function.chunk.lines[instruction];
            eprintln!("[line {line}] in {function}");
        }
        self.reset_stack();
    }

    fn define_native(&mut self, name: &str, function: NativeFunction) {
        let name = self.heap_alloc(ObjString::new(name));
        self.push_stack(Value::ObjString(name));
        let native = self.heap_alloc(ObjNative::new(function));
        self.push_stack(Value::ObjNative(native));

        match self.stack[0] {
            Value::ObjString(str) => self
                .globals
                .insert(unsafe { (*str).str.clone() }, self.stack[1].clone()),
            _ => panic!("This shouldn't be possible..."),
        };

        self.pop_stack();
        self.pop_stack();
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
            let new_obj = self.heap_alloc(ObjString::new(format!("{}{}", str1, str2).as_str()));
            let new_value = Value::ObjString(new_obj);
            self.push_stack(new_value);
        }

        Ok(())
    }

    fn call_value(&mut self, callee: Value, arg_count: usize) -> bool {
        match callee {
            Value::ObjNative(obj_native) => {
                self.call_native(obj_native, arg_count);
                true
            }
            Value::ObjClosure(obj_closure) => self.call(obj_closure, arg_count),
            _ => {
                self.runtime_error("Can only call functions and classes.");
                false
            }
        }
    }

    fn call(&mut self, closure: *mut ObjClosure, arg_count: usize) -> bool {
        let function = unsafe { (*closure).function };
        let arity = unsafe { (*function).arity as usize };
        if arg_count != arity {
            self.runtime_error(format!("Expected {arity} arguments but got {arg_count}").as_str());
            return false;
        }
        if self.frames.len() == FRAMES_MAX {
            self.runtime_error("Stack overflow.");
            return false;
        }
        self.frames.push(CallFrame {
            closure,
            first_slot: self.stack_top - arg_count - 1,
            ip: 0,
        });
        true
    }

    fn call_native(&mut self, native: *const ObjNative, arg_count: usize) {
        let native = unsafe { &(*native) };

        let result = match native.native_function {
            crate::object_native::NativeFunction::Clock => {
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                Value::Number(time as f64)
            }
        };

        self.stack_top -= arg_count + 1;
        self.push_stack(result);
    }

    fn heap_alloc<T>(&mut self, obj: T) -> *mut T
    where
        T: GC + std::fmt::Display + 'static,
    {
        if self.debug_stress_gc {
            self.collect_garbage()
        }
        self.allocator.heap_alloc(obj)
    }

    fn collect_garbage(&mut self) {
        if self.debug_log_gc {
            println!("-- gc begin (vm)");
        }

        self.mark_roots();

        if self.debug_log_gc {
            println!("-- gc end (vm)");
        }
    }

    fn mark_roots(&mut self) {
        // Mark variables on the stack
        for i in 0..self.stack_top {
            VM::mark_value(&self.stack[i], self.debug_log_gc);
        }

        // Mark variables in the globals table
        for (_, val) in self.globals.iter_mut() {
            VM::mark_value(val, self.debug_log_gc);
        }

        // Mark closures in call frames
        for frame in self.frames.iter_mut() {
            VM::mark_value(&Value::ObjClosure(frame.closure), self.debug_log_gc)
        }

        // Mark open upvalues
        let mut upvalue = self.open_upvalues;
        while let Some(unwrapped_upvalue) = upvalue {
            unsafe {
                if self.debug_log_gc {
                    println!("mark {}", (*unwrapped_upvalue));
                }
                (*unwrapped_upvalue).is_marked = true;
                upvalue = (*unwrapped_upvalue).next_upvalue;
            }
        }
    }

    fn mark_value(value: &Value, debug_log_gc: bool) {
        match value {
            Value::Bool(_) | Value::Nil | Value::Number(_) => {
                return;
            }
            Value::ObjString(obj_string) => {
                if debug_log_gc {
                    println!("mark {}", value);
                }
                unsafe { (*(*obj_string)).is_marked = true };
            }
            Value::ObjFunction(obj_function) => {
                if debug_log_gc {
                    println!("mark {}", value);
                }
                unsafe { (*(*obj_function)).is_marked = true }
            }
            Value::ObjNative(obj_native) => {
                if debug_log_gc {
                    println!("mark {}", value);
                }
                unsafe { (*(*obj_native)).is_marked = true }
            }
            Value::ObjClosure(object_closure) => {
                if debug_log_gc {
                    println!("mark {}", value);
                }
                unsafe { (*(*object_closure)).is_marked = true }
            }
        }
    }
}
