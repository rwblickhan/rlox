use derive_more::Display;

#[derive(Display, Clone, PartialEq)]
pub(crate) enum Obj {
    String(String),
}
