use core::{cmp, fmt, ops};

macro_rules! opcodes {
    { $($opcode:literal => $name:ident $(,)*)* } => {
        #[derive(Clone, Copy, PartialEq, Debug)]
        #[repr(u8)]
        pub enum Opcode {
            $(
                $name = $opcode,
             )*
        }

        impl TryFrom<u8> for Opcode {
            type Error = ();

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $(
                        $opcode => Ok(Self::$name),
                    )*
                    _ => Err(()),
                }
            }
        }
    };
}

opcodes! {
    0 => Return,
    1 => Constant,
    2 => Negate,
    3 => Add,
    4 => Subtract,
    5 => Multiply,
    6 => Divide,
    7 => True,
    8 => False,
    9 => BitwiseAnd,
    10 => BitwiseOr,
    11 => BitwiseXor,
    12 => Equal,
    13 => NotEqual,
    14 => LessThan,
    15 => LessEqual,
    16 => GreaterThan,
    17 => GreaterEqual,
}

#[derive(Clone, Eq, Debug)]
pub enum Value {
    Unit,
    Integer(i64),
    Bool(bool),
    String(String),
}

impl Value {
    pub fn as_integer(&self) -> Option<i64> {
        if let Value::Integer(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        if let Value::String(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

impl cmp::PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(l), Self::Integer(r)) => l == r,
            (Self::Bool(l), Self::Bool(r)) => l == r,
            (Self::String(l), Self::String(r)) => l == r,
            _ => false,
        }
    }
}

impl cmp::PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match (self, other) {
            (Self::Integer(l), Self::Integer(r)) => Some(l.cmp(r)),
            (Self::Bool(l), Self::Bool(r)) => Some(l.cmp(r)),
            (Self::String(l), Self::String(r)) => Some(l.cmp(r)),
            _ => None,
        }
    }
}

pub struct Chunk {
    code: Vec<u8>,
    constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk { code: Vec::new(), constants: Vec::new() }
    }

    pub fn push(&mut self, byte: u8) {
        self.code.push(byte);
    }

    pub fn create_constant(&mut self, value: Value) -> usize {
        let index = self.constants.len();
        self.constants.push(value);
        index
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut stream = self.code.iter().enumerate();

        while let Some((offset, &opcode)) = stream.next() {
            let do_simple_instruction = |f: &mut fmt::Formatter<'_>, offset, name| {
                writeln!(f, "[{:#x}] {}", offset, name).unwrap();
            };

            let Ok(opcode) = Opcode::try_from(opcode) else {
                writeln!(f, "[!!!] Invalid opcode: {:#x}", opcode).unwrap();
                continue;
            };

            match opcode {
                Opcode::Return => do_simple_instruction(f, offset, "RETURN"),
                Opcode::Constant => {
                    let (_, index) = stream.next().unwrap();
                    let value = self.constants.get(*index as usize);
                    writeln!(f, "[{:#x}] CONSTANT {:?}", offset, value).unwrap();
                }
                Opcode::Negate => do_simple_instruction(f, offset, "NEGATE"),
                Opcode::Add => do_simple_instruction(f, offset, "ADD"),
                Opcode::Subtract => do_simple_instruction(f, offset, "SUBTRACT"),
                Opcode::Multiply => do_simple_instruction(f, offset, "MULTIPLY"),
                Opcode::Divide => do_simple_instruction(f, offset, "DIVIDE"),
                Opcode::True => do_simple_instruction(f, offset, "TRUE"),
                Opcode::False => do_simple_instruction(f, offset, "FALSE"),
                Opcode::BitwiseAnd => do_simple_instruction(f, offset, "BITWISE_AND"),
                Opcode::BitwiseOr => do_simple_instruction(f, offset, "BITWISE_OR"),
                Opcode::BitwiseXor => do_simple_instruction(f, offset, "BITWISE_XOR"),
                Opcode::Equal => do_simple_instruction(f, offset, "EQUAL"),
                Opcode::NotEqual => do_simple_instruction(f, offset, "NOT_EQUAL"),
                Opcode::LessThan => do_simple_instruction(f, offset, "LESS_THAN"),
                Opcode::LessEqual => do_simple_instruction(f, offset, "LESS_EQUAL"),
                Opcode::GreaterThan => do_simple_instruction(f, offset, "GREATER_THAN"),
                Opcode::GreaterEqual => do_simple_instruction(f, offset, "GREATER_EQUAL"),
            }
        }
        Ok(())
    }
}

pub struct Vm {
    stack: Vec<Value>,
    chunk: Option<Chunk>,
    ip: usize,
}

impl Vm {
    pub fn new() -> Vm {
        Vm { stack: Vec::new(), chunk: None, ip: 0 }
    }

    pub fn interpret(&mut self, chunk: Chunk) -> Value {
        self.chunk = Some(chunk);
        self.ip = 0;

        loop {
            let Ok(op) = Opcode::try_from(self.next()) else {
                panic!();
            };

            // TODO: this should be behind a compiler flag or something maybe, as it's useful long-term
            println!("[{:#x}] {:?} ({:?})", self.ip - 1, op, self.stack);

            match op {
                Opcode::Return => break,
                Opcode::Constant => {
                    let index = self.next() as usize;
                    let constant = self.chunk.as_ref().unwrap().constants.get(index).unwrap();
                    self.stack.push(constant.clone());
                }
                Opcode::True => self.stack.push(Value::Bool(true)),
                Opcode::False => self.stack.push(Value::Bool(false)),
                Opcode::Negate => {
                    if let Value::Integer(value) = self.stack.pop().unwrap() {
                        self.stack.push(Value::Integer(-value));
                    } else {
                        panic!("Can't negate value!");
                    }
                }
                Opcode::Add
                | Opcode::Subtract
                | Opcode::Multiply
                | Opcode::Divide
                | Opcode::BitwiseAnd
                | Opcode::BitwiseOr
                | Opcode::BitwiseXor
                | Opcode::Equal
                | Opcode::NotEqual
                | Opcode::LessThan
                | Opcode::LessEqual
                | Opcode::GreaterThan
                | Opcode::GreaterEqual => self.do_binary_op(op),
            }
        }

        Value::Unit
    }

    fn next(&mut self) -> u8 {
        // TODO: handle error
        let byte = *self.chunk.as_ref().unwrap().code.get(self.ip).unwrap();
        self.ip += 1;
        byte
    }

    fn do_binary_op(&mut self, op: Opcode) {
        let right = self.stack.pop().unwrap();
        let left = self.stack.pop().unwrap();

        match op {
            Opcode::Add
            | Opcode::Subtract
            | Opcode::Multiply
            | Opcode::Divide
            | Opcode::BitwiseAnd
            | Opcode::BitwiseOr
            | Opcode::BitwiseXor => {
                let left = left.as_integer().unwrap();
                let right = right.as_integer().unwrap();
                let result = Value::Integer(match op {
                    Opcode::Add => left + right,
                    Opcode::Subtract => left - right,
                    Opcode::Multiply => left * right,
                    Opcode::Divide => left / right,
                    Opcode::BitwiseAnd => left & right,
                    Opcode::BitwiseOr => left | right,
                    Opcode::BitwiseXor => left ^ right,
                    _ => unreachable!(),
                });
                self.stack.push(result);
            }
            Opcode::Equal => self.stack.push(Value::Bool(left == right)),
            Opcode::NotEqual => self.stack.push(Value::Bool(left != right)),
            Opcode::LessThan => self.stack.push(Value::Bool(left < right)),
            Opcode::LessEqual => self.stack.push(Value::Bool(left <= right)),
            Opcode::GreaterThan => self.stack.push(Value::Bool(left > right)),
            Opcode::GreaterEqual => self.stack.push(Value::Bool(left >= right)),
            _ => panic!(),
        }
    }
}
