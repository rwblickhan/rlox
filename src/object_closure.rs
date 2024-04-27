use crate::memory::GC;
use crate::object_function::ObjFunction;
use crate::object_upvalue::ObjUpvalue;
use std::fmt::Display;

#[derive(Default, Clone, Copy)]
pub struct Upvalue {
    pub index: u8,
    pub is_local: bool,
}

impl Upvalue {
    pub fn new(index: u8, is_local: bool) -> Upvalue {
        Upvalue { index, is_local }
    }
}

pub struct ObjClosure {
    pub function: *const ObjFunction,
    pub upvalues: Vec<*const ObjUpvalue>,
    next: Option<*mut dyn GC>,
}

impl ObjClosure {
    pub fn new(function: *const ObjFunction) -> ObjClosure {
        ObjClosure {
            function,
            upvalues: Vec::new(),
            next: None,
        }
    }
}

impl GC for ObjClosure {
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

impl Display for ObjClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { (*self.function).fmt(f) }
    }
}
