use crate::object::{GinkgoObj, GinkgoString, ObjHeader};
use core::{cmp, fmt};
use std::collections::BTreeMap;

macro_rules! opcodes {
    { $($(#[$($attrss:meta)*])* $opcode:literal => $name:ident $(,)*)* } => {
        #[derive(Clone, Copy, PartialEq, Debug)]
        #[repr(u8)]
        pub enum Opcode {
            $(
                $(#[$($attrss)*])*
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
    /// Push a constant onto the stack. Followed by a single-byte operand which is the index into the chunk's constant table.
    1 => Constant,
    2 => Negate,
    3 => Add,
    4 => Subtract,
    5 => Multiply,
    6 => Divide,
    /// Push a `true` boolean value onto the stack.
    7 => True,
    /// Push a `false` boolean value onto the stack.
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
    18 => Pop,
    19 => DefineGlobal,
    20 => GetGlobal,
    21 => SetGlobal,
    22 => GetLocal,
    23 => SetLocal,
    24 => Jump,
    25 => JumpIfTrue,
    26 => JumpIfFalse,
}

pub struct Chunk {
    pub code: Vec<u8>,
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

    pub fn pop_last(&mut self) -> Option<u8> {
        self.code.pop()
    }

    pub fn pop_last_op(&mut self) -> Option<Opcode> {
        let last = self.code.pop();
        last?.try_into().ok()
    }

    pub fn current_offset(&self) -> usize {
        self.code.len()
    }

    pub fn patch_jump(&mut self, jump_operand_offset: usize) {
        // XXX: minus an extra 2 to account for the `i16` operand
        let bytes = i16::try_from(self.code.len().checked_signed_diff(jump_operand_offset).unwrap() - 2)
            .unwrap()
            .to_le_bytes();
        self.code[jump_operand_offset] = bytes[0];
        self.code[jump_operand_offset + 1] = bytes[1];
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut stream = self.code.iter().enumerate();

        while let Some((offset, &opcode)) = stream.next() {
            macro_rules! decompile {
                ($name:literal) => {{
                    writeln!(f, "[{:#x}] {}", offset, $name).unwrap();
                }};
                ($name:literal, operand) => {{
                    let (_, operand) = stream.next().unwrap();
                    writeln!(f, "[{:#x}] {} {}", offset, $name, operand).unwrap();
                }};
                ($name:literal, constant) => {{
                    let (_, index) = stream.next().unwrap();
                    let value = self.constants.get(*index as usize);
                    writeln!(f, "[{:#x}] {} {:?}", offset, $name, value).unwrap();
                }};
                ($name:literal, jump_operand) => {{
                    let jump_by = i16::from_le_bytes([*stream.next().unwrap().1, *stream.next().unwrap().1]);
                    writeln!(f, "[{:#x}] {} {}", offset, $name, jump_by).unwrap();
                }};
            }

            let Ok(opcode) = Opcode::try_from(opcode) else {
                writeln!(f, "[!!!] Invalid opcode: {:#x}", opcode).unwrap();
                continue;
            };

            match opcode {
                Opcode::Return => decompile!("Return"),
                Opcode::Constant => decompile!("Constant", constant),
                Opcode::Negate => decompile!("Negate"),
                Opcode::Add => decompile!("Add"),
                Opcode::Subtract => decompile!("Subtract"),
                Opcode::Multiply => decompile!("Multiply"),
                Opcode::Divide => decompile!("Divide"),
                Opcode::True => decompile!("True"),
                Opcode::False => decompile!("False"),
                Opcode::BitwiseAnd => decompile!("BitwiseAnd"),
                Opcode::BitwiseOr => decompile!("BitwiseOr"),
                Opcode::BitwiseXor => decompile!("BitwiseXor"),
                Opcode::Equal => decompile!("Equal"),
                Opcode::NotEqual => decompile!("NotEqual"),
                Opcode::LessThan => decompile!("LessThan"),
                Opcode::LessEqual => decompile!("LessEqual"),
                Opcode::GreaterThan => decompile!("GreaterThan"),
                Opcode::GreaterEqual => decompile!("GreaterEqual"),
                Opcode::Pop => decompile!("Pop"),
                Opcode::DefineGlobal => decompile!("DefineGlobal"),
                Opcode::GetGlobal => decompile!("GetGlobal", constant),
                Opcode::SetGlobal => decompile!("SetGlobal", constant),
                Opcode::GetLocal => decompile!("GetLocal", operand),
                Opcode::SetLocal => decompile!("SetLocal", operand),
                Opcode::Jump => decompile!("Jump", jump_operand),
                Opcode::JumpIfTrue => decompile!("JumpIfTrue", jump_operand),
                Opcode::JumpIfFalse => decompile!("JumpIfFalse", jump_operand),
            }
        }
        Ok(())
    }
}

pub struct Vm {
    pub stack: Vec<Value>,
    chunk: Option<Chunk>,
    ip: usize,

    globals: BTreeMap<String, Value>,
}

impl Vm {
    pub fn new() -> Vm {
        Vm { stack: Vec::new(), chunk: None, ip: 0, globals: BTreeMap::new() }
    }

    pub fn interpret(&mut self, chunk: Chunk) -> Value {
        self.chunk = Some(chunk);
        self.ip = 0;

        loop {
            let Ok(op) = Opcode::try_from(self.next()) else {
                panic!();
            };

            // TODO: this should be behind a compiler flag or something maybe, as it's useful long-term
            println!("{:?}", self.stack); // Print stack before we execute this op under last instruction
            println!("[{:#x}] {:?}", self.ip - 1, op);

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
                Opcode::Pop => {
                    let _ = self.stack.pop().unwrap();
                }
                Opcode::DefineGlobal => {
                    let value = self.stack.pop().unwrap();
                    let name = {
                        let name = self.stack.pop().unwrap();
                        let name = unsafe { name.as_obj::<GinkgoString>().unwrap() };
                        name.as_str().to_string()
                    };
                    self.globals.insert(name, value);
                }
                Opcode::GetGlobal => {
                    let index = self.next() as usize;
                    let name = {
                        let name = self.chunk.as_ref().unwrap().constants.get(index).unwrap();
                        let name = unsafe { name.as_obj::<GinkgoString>().unwrap() };
                        name.as_str().to_string()
                    };
                    let value = self.globals.get(&name).unwrap().clone();
                    self.stack.push(value);
                }
                Opcode::SetGlobal => {
                    let index = self.next() as usize;
                    let value = self.stack.pop().unwrap();
                    let name = {
                        let name = self.chunk.as_ref().unwrap().constants.get(index).unwrap();
                        let name = unsafe { name.as_obj::<GinkgoString>().unwrap() };
                        name.as_str().to_string()
                    };

                    // XXX: assignment keeps the value on the stack, so push it back after the global is popped
                    self.stack.push(value.clone());

                    // Replace the value, erroring if it doesn't yet exist
                    if let None = self.globals.insert(name.clone(), value) {
                        self.globals.remove(&name);
                        // TODO: runtime error
                        panic!("Tried to assign to undefined variable!");
                    }
                }
                Opcode::GetLocal => {
                    let slot = self.next() as usize;
                    self.stack.push(self.stack.get(slot).unwrap().clone());
                }
                Opcode::SetLocal => {
                    let slot = self.next() as usize;
                    // XXX: we leave the value on the stack, as the assignment should still produce a value
                    self.stack.insert(slot, self.stack.last().unwrap().clone());
                }
                Opcode::Jump => {
                    let jump_offset = i16::from_le_bytes([self.next(), self.next()]);
                    self.ip = self.ip.checked_add_signed(jump_offset as isize).unwrap();
                }
                Opcode::JumpIfTrue => {
                    let jump_offset = i16::from_le_bytes([self.next(), self.next()]);
                    let Value::Bool(result) = self.stack.last().unwrap() else {
                        panic!("Tried to jump but result is not a boolean!");
                    };
                    if *result {
                        self.ip = self.ip.checked_add_signed(jump_offset as isize).unwrap();
                    }
                }
                Opcode::JumpIfFalse => {
                    let jump_offset = i16::from_le_bytes([self.next(), self.next()]);
                    let Value::Bool(result) = self.stack.last().unwrap() else {
                        panic!("Tried to jump but result is not a boolean!");
                    };
                    if !result {
                        self.ip = self.ip.checked_add_signed(jump_offset as isize).unwrap();
                    }
                }
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

#[derive(Clone, Eq, Debug)]
pub enum Value {
    Unit,
    Integer(i64),
    Bool(bool),
    Obj(*const ObjHeader),
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

    pub unsafe fn as_obj<T: GinkgoObj>(&self) -> Option<&T> {
        if let Value::Obj(ptr) = self {
            let obj_typ = unsafe { (**ptr).typ };
            if obj_typ == T::TYP {
                Some(unsafe { &*(*ptr as *const T) })
            } else {
                None
            }
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
            (Self::Obj(l), Self::Obj(r)) => crate::object::object_eq(*l, *r),
            _ => false,
        }
    }
}

impl cmp::PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match (self, other) {
            (Self::Integer(l), Self::Integer(r)) => Some(l.cmp(r)),
            (Self::Bool(l), Self::Bool(r)) => Some(l.cmp(r)),
            (Self::Obj(l), Self::Obj(r)) => todo!(),
            _ => None,
        }
    }
}
