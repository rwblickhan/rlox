use crate::object::Obj;
use derive_more::Display;
use std::rc::Rc;

#[derive(Display, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    Obj(Rc<Obj>),
}

impl Value {
    pub fn to_bool_value(bool: bool) -> Value {
        Value::Bool(bool)
    }

    pub fn to_number_value(number: f64) -> Value {
        Value::Number(number)
    }

    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::Bool(false))
    }
}
