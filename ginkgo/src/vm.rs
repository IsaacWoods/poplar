use core::fmt;

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum Opcode {
    Return = 0,
}

impl TryFrom<u8> for Opcode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Opcode::Return),
            _ => Err(()),
        }
    }
}

pub struct Chunk(Vec<u8>);

impl Chunk {
    pub fn new() -> Chunk {
        Chunk(Vec::new())
    }

    pub fn push(&mut self, byte: u8) {
        self.0.push(byte);
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut stream = self.0.iter().enumerate();
        while let Some((offset, &opcode)) = stream.next() {
            match opcode.try_into() {
                Ok(Opcode::Return) => {
                    write!(f, "[{:#04x}] RETURN", offset).unwrap();
                }
                Err(_) => {
                    write!(f, "[!!!] Invalid opcode: {:#x}", opcode).unwrap();
                }
            }
        }
        Ok(())
    }
}
