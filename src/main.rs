mod chunk;
mod debug;
mod value;
mod vm;

use chunk::{Chunk, Opcode};

fn main() {
    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(1.2);
    chunk.write_chunk(Opcode::Constant as u8, 123);
    chunk.write_chunk(constant as u8, 123);
    let constant = chunk.add_constant(3.4);
    chunk.write_chunk(Opcode::Constant as u8, 123);
    chunk.write_chunk(constant as u8, 123);
    chunk.write_chunk(Opcode::Add as u8, 123);
    let constant = chunk.add_constant(5.6);
    chunk.write_chunk(Opcode::Constant as u8, 123);
    chunk.write_chunk(constant as u8, 123);
    chunk.write_chunk(Opcode::Divide as u8, 123);
    chunk.write_chunk(Opcode::Negate as u8, 123);
    chunk.write_chunk(Opcode::Return as u8, 123);

    let mut vm = vm::VM::new(&chunk);
    vm.interpret(true);
}
