use crate::value::Value;

#[repr(u8)]
pub enum Opcode {
    Return = 0,
    Constant,
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
        return self.constants.len() - 1;
    }
}

impl TryFrom<u8> for Opcode {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Opcode::Return),
            1 => Ok(Opcode::Constant),
            _ => Err(()),
        }
    }
}