use crate::value::Value;

#[repr(u8)]
pub enum Opcode {
    Return = 0,
    Constant,
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
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
            2 => Ok(Opcode::Negate),
            3 => Ok(Opcode::Add),
            4 => Ok(Opcode::Subtract),
            5 => Ok(Opcode::Multiply),
            6 => Ok(Opcode::Divide),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Opcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Opcode::Return => write!(f, "OP_RETURN"),
            Opcode::Constant => write!(f, "OP_CONSTANT"),
            Opcode::Negate => write!(f, "OP_NEGATE"),
            Opcode::Add => write!(f, "OP_ADD"),
            Opcode::Subtract => write!(f, "OP_SUBTRACT"),
            Opcode::Multiply => write!(f, "OP_MULTIPLY"),
            Opcode::Divide => write!(f, "OP_DIVIDE"),
        }
    }
}
