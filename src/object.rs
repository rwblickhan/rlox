use derive_more::Display;
use std::fmt::Display;
use std::hash::Hash;

#[derive(Display, Clone, PartialEq)]
pub(crate) enum Obj {
    String(ObjString),
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ObjString {
    string: String,
    hash: u32,
}

fn hash_string(str: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for i in 0..str.len() {
        hash ^= str.as_bytes()[i] as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

impl ObjString {
    pub(crate) fn new_from_string(string: String) -> ObjString {
        let hash = hash_string(&string);
        ObjString { string, hash }
    }
}

impl Display for ObjString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string)
    }
}

impl Hash for ObjString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
