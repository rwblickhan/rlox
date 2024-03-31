use std::fmt::Display;

use crate::object_string::ObjString;
use crate::{chunk::Chunk, memory::GC};

#[derive(Clone, Copy, PartialEq)]
pub enum FunctionType {
    Function,
    Script,
}

pub struct ObjFunction {
    pub function_type: FunctionType,
    pub arity: u8,
    pub chunk: Chunk,
    pub name: Option<ObjString>,
    next: Option<*mut dyn GC>,
}

impl GC for ObjFunction {
    fn next(&self) -> Option<*mut dyn GC> {
        self.next
    }

    fn set_next(&mut self, next: Option<*mut dyn GC>) {
        self.next = next;
    }

    fn layout(&self) -> std::alloc::Layout {
        std::alloc::Layout::new::<Self>()
    }
}

impl ObjFunction {
    pub fn new(function_type: FunctionType, name: Option<ObjString>) -> ObjFunction {
        ObjFunction {
            function_type,
            arity: 0,
            chunk: Chunk::new(),
            name,
            next: None,
        }
    }
}

impl Display for ObjFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => name.fmt(f),
            None => write!(f, "<script>"),
        }
    }
}
