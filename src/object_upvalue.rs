use std::fmt::Display;

use crate::{memory::GC, value::Value};

pub struct ObjUpvalue {
    pub location: usize,
    pub next_upvalue: Option<*mut ObjUpvalue>,
    pub closed: Option<Value>,
    pub is_marked: bool,
    next: Option<*mut dyn GC>,
}

impl ObjUpvalue {
    pub fn new(location: usize) -> ObjUpvalue {
        ObjUpvalue {
            location,
            next_upvalue: None,
            closed: None,
            is_marked: false,
            next: None,
        }
    }
}

impl GC for ObjUpvalue {
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

impl Display for ObjUpvalue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "upvalue")
    }
}
