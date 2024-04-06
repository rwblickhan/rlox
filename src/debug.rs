use crate::{
    chunk::{Chunk, Opcode},
    value::Value,
};

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
        Opcode::Nil => disassemble_simple_instruction(opcode, offset),
        Opcode::True => disassemble_simple_instruction(opcode, offset),
        Opcode::False => disassemble_simple_instruction(opcode, offset),
        Opcode::Add => disassemble_simple_instruction(opcode, offset),
        Opcode::Subtract => disassemble_simple_instruction(opcode, offset),
        Opcode::Multiply => disassemble_simple_instruction(opcode, offset),
        Opcode::Divide => disassemble_simple_instruction(opcode, offset),
        Opcode::Not => disassemble_simple_instruction(opcode, offset),
        Opcode::Equal => disassemble_simple_instruction(opcode, offset),
        Opcode::Greater => disassemble_simple_instruction(opcode, offset),
        Opcode::Less => disassemble_simple_instruction(opcode, offset),
        Opcode::Print => disassemble_simple_instruction(opcode, offset),
        Opcode::Pop => disassemble_simple_instruction(opcode, offset),
        Opcode::DefineGlobal => disassemble_constant_instruction(opcode, chunk, offset),
        Opcode::GetGlobal => disassemble_constant_instruction(opcode, chunk, offset),
        Opcode::SetGlobal => disassemble_constant_instruction(opcode, chunk, offset),
        Opcode::GetLocal => disassemble_byte_instruction(opcode, chunk, offset),
        Opcode::SetLocal => disassemble_byte_instruction(opcode, chunk, offset),
        Opcode::JumpIfFalse => disassemble_jump_instruction(opcode, chunk, offset, true),
        Opcode::Jump => disassemble_jump_instruction(opcode, chunk, offset, true),
        Opcode::Loop => disassemble_jump_instruction(opcode, chunk, offset, false),
        Opcode::Call => disassemble_byte_instruction(opcode, chunk, offset),
        Opcode::Closure => {
            let constant_offset = chunk.code[offset + 1];
            println!(
                "{:<16} {:>4} {}",
                opcode, constant_offset, chunk.constants[constant_offset as usize]
            );

            let upvalue_count =
                if let Value::ObjFunction(obj_fun) = &chunk.constants[constant_offset as usize] {
                    let upvalue_count = unsafe { (**obj_fun).upvalue_count };
                    for i in 0..upvalue_count {
                        let is_local = chunk.code[(offset + 2) + i];
                        let index = chunk.code[(offset + 2) + (i + 1)];
                        println!(
                            "{:>4}       |                     {} {}",
                            offset,
                            if is_local == 1 { "local" } else { "upvalue" },
                            index
                        );
                    }
                    upvalue_count
                } else {
                    0
                };

            offset + 2 + (upvalue_count * 2)
        }
        Opcode::GetUpvalue => disassemble_byte_instruction(opcode, chunk, offset),
        Opcode::SetUpvalue => disassemble_byte_instruction(opcode, chunk, offset),
    }
}

fn disassemble_simple_instruction(opcode: &Opcode, offset: usize) -> usize {
    println!("{}", opcode);
    offset + 1
}

fn disassemble_constant_instruction(opcode: &Opcode, chunk: &Chunk, offset: usize) -> usize {
    let constant_offset = chunk.code[offset + 1];
    println!(
        "{:<16} {:>4} '{}'",
        opcode, constant_offset, chunk.constants[constant_offset as usize]
    );
    offset + 2
}

fn disassemble_byte_instruction(opcode: &Opcode, chunk: &Chunk, offset: usize) -> usize {
    let slot = chunk.code[offset + 1];
    println!("{:<16} {:>4}", opcode, slot);
    offset + 2
}

fn disassemble_jump_instruction(
    opcode: &Opcode,
    chunk: &Chunk,
    offset: usize,
    forward: bool,
) -> usize {
    let jump = ((chunk.code[offset + 1] as u16) << 8 | chunk.code[offset + 2] as u16) as usize;
    let target = if forward {
        offset + 3 + jump
    } else {
        offset + 3 - jump
    };
    println!("{:<16} {:>4} -> {}", opcode, offset, target);
    offset + 3
}
