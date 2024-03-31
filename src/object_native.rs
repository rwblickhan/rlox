use crate::memory::GC;
use std::fmt::Display;

pub enum NativeFunction {
    Clock,
}

pub struct ObjNative {
    pub native_function: NativeFunction,
    next: Option<*mut dyn GC>,
}

impl ObjNative {
    pub fn new(native_function: NativeFunction) -> ObjNative {
        ObjNative {
            native_function,
            next: None,
        }
    }
}

impl GC for ObjNative {
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

impl Display for ObjNative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "<native fn>".fmt(f)
    }
}
