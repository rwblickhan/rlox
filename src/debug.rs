use crate::chunk::{Chunk, Opcode};

pub fn disassemble_chunk(chunk: &Chunk, name: &str) {
    println!("== {} ==", name);

    let mut offset = 0;
    while offset < chunk.code.len() {
        print!("{:04} ", offset);

        if offset > 0 && chunk.lines[offset] == chunk.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:4} ", chunk.lines[offset]);
        }

        let byte = chunk.code[offset];
        if let Ok(opcode) = Opcode::try_from(byte) {
            offset = disassemble_instruction(&opcode, chunk, offset);
        } else {
            println!("Unknown opcode {byte}");
            offset += 1;
        }
    }
}

pub fn disassemble_instruction(opcode: &Opcode, chunk: &Chunk, offset: usize) -> usize {
    match opcode {
        Opcode::Return => disassemble_simple_instruction(opcode, offset),
        Opcode::Constant => disassemble_constant_instruction(opcode, chunk, offset),
        Opcode::Negate => disassemble_simple_instruction(opcode, offset),
        Opcode::Add => disassemble_simple_instruction(opcode, offset),
        Opcode::Subtract => disassemble_simple_instruction(opcode, offset),
        Opcode::Multiply => disassemble_simple_instruction(opcode, offset),
        Opcode::Divide => disassemble_simple_instruction(opcode, offset),
    }
}

fn disassemble_simple_instruction(opcode: &Opcode, offset: usize) -> usize {
    println!("{}", opcode);
    return offset + 1;
}

fn disassemble_constant_instruction(opcode: &Opcode, chunk: &Chunk, offset: usize) -> usize {
    let constant_offset = chunk.code[offset + 1];
    println!(
        "{:<16} {:>4} '{}'",
        opcode, constant_offset, chunk.constants[constant_offset as usize]
    );
    return offset + 2;
}
