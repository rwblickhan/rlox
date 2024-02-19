use derive_more::Display;

use crate::value::Value;

#[derive(Display)]
#[repr(u8)]
pub enum Opcode {
    Return = 0,
    Constant,
    Negate,
    Nil,
    True,
    False,
    Add,
    Subtract,
    Multiply,
    Divide,
    Not,
    Equal,
    Greater,
    Less,
}

pub struct Chunk {
    pub code: Vec<u8>,
    pub lines: Vec<usize>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            code: Vec::new(),
            lines: Vec::new(),
            constants: Vec::new(),
        }
    }

    pub fn write_chunk(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

impl TryFrom<u8> for Opcode {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Opcode::Return),
            1 => Ok(Opcode::Constant),
            2 => Ok(Opcode::Negate),
            3 => Ok(Opcode::Nil),
            4 => Ok(Opcode::True),
            5 => Ok(Opcode::False),
            6 => Ok(Opcode::Add),
            7 => Ok(Opcode::Subtract),
            8 => Ok(Opcode::Multiply),
            9 => Ok(Opcode::Divide),
            10 => Ok(Opcode::Not),
            11 => Ok(Opcode::Equal),
            12 => Ok(Opcode::Greater),
            13 => Ok(Opcode::Less),
            _ => Err(()),
        }
    }
}
