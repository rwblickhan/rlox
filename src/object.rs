use std::{fmt::Display, hash::Hash};

#[derive(Clone, PartialEq)]
pub(crate) enum Obj {
    String { string: String, hash: u32 },
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
    pub(crate) fn new_from_string(string: String) -> Obj {
        let hash = hash_string(&string);
        Obj::String { string, hash }
    }
}

impl Display for Obj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Obj::String { string, .. } => write!(f, "{}", string),
        }
    }
}
