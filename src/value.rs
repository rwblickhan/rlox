use std::fmt::{Display, Error};

use crate::object::Obj;

#[derive(Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    Obj(*const Obj),
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
            &Value::Obj(obj) => unsafe {
                let Some(obj) = obj.as_ref() else {
                    return Err(Error);
                };
                obj.fmt(f)
            },
        }
    }
}
