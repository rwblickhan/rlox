use crate::memory::GC;
use std::fmt::Display;
use std::hash::Hash;

pub(crate) struct ObjString {
    pub str: String,
    pub is_marked: bool,
    hash: u32,
    next: Option<*mut dyn GC>,
}

impl GC for ObjString {
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

impl ObjString {
    pub(crate) fn new(string: &str) -> ObjString {
        let hash = ObjString::hash_string(string);
        ObjString {
            str: string.to_owned(),
            is_marked: false,
            hash,
            next: None,
        }
    }

    fn hash_string(str: &str) -> u32 {
        let mut hash: u32 = 2166136261;
        for i in 0..str.len() {
            hash ^= str.as_bytes()[i] as u32;
            hash = hash.wrapping_mul(16777619);
        }
        hash
    }
}

impl Display for ObjString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.str.fmt(f)
    }
}

impl Hash for ObjString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state)
    }
}
