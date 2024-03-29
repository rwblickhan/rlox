use std::fmt::Display;

use crate::object_string::ObjString;

#[derive(Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    ObjString(*const ObjString),
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

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(bool) => bool.fmt(f),
            Value::Nil => write!(f, "nil"),
            Value::Number(number) => number.fmt(f),
            Value::ObjString(obj_str) => unsafe { (**obj_str).fmt(f) },
        }
    }
}
