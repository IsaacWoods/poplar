#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Integer(isize),
    String(String),
    Bool(bool),
}
