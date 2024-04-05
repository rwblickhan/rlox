use crate::memory::GC;
use crate::object_function::ObjFunction;
use std::fmt::Display;

pub struct ObjClosure {
    pub function: *const ObjFunction,
    next: Option<*mut dyn GC>,
}

impl ObjClosure {
    pub fn new(function: *const ObjFunction) -> ObjClosure {
        ObjClosure {
            function,
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
