mod chunk;
mod debug;
mod value;

use chunk::{Chunk, Opcode};

fn main() {
    let mut chunk = Chunk::new();
    let constant = chunk.add_constant(1.2);
    chunk.write_chunk(Opcode::Constant as u8, 123);
    chunk.write_chunk(constant as u8, 123);
    chunk.write_chunk(Opcode::Return as u8, 123);
    debug::disassemble_chunk(&chunk, "test chunk");
}
