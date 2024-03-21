use core::str;
use std::fmt::Display;
use std::hash::Hash;
use std::ptr::null_mut;

pub(crate) struct Obj {
    pub obj_type: ObjType,
    pub next: *mut Obj,
}

pub enum ObjType {
    String(String, u32),
}

fn hash_string(str: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for i in 0..str.len() {
        hash ^= str.as_bytes()[i] as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

impl Obj {
    pub(crate) fn new_from_string(string: &str) -> Obj {
        let hash = hash_string(string);
        Obj {
            obj_type: ObjType::String(string.to_owned(), hash),
            next: null_mut(),
        }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.obj_type.fmt(f)
    }
}

impl Display for ObjType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjType::String(str, _) => str.fmt(f),
        }
    }
}

impl Hash for ObjType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ObjType::String(_, hash) => hash.hash(state),
        }
    }
}
