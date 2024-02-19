use derive_more::Display;

#[derive(Display, Clone, Copy, PartialEq)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
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
