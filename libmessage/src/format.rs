pub const UNIT: u8 = 0x00;
pub const BOOL_FALSE: u8 = 0x01;
pub const BOOL_TRUE: u8 = 0x02;
pub const OPTION_NONE: u8 = 0x03;
pub const OPTION_SOME: u8 = 0x04;
pub const CHAR: u8 = 0x05;

pub const U8: u8 = 0x10;
pub const U16: u8 = 0x11;
pub const U32: u8 = 0x12;
pub const U64: u8 = 0x13;

pub const I8: u8 = 0x20;
pub const I16: u8 = 0x21;
pub const I32: u8 = 0x22;
pub const I64: u8 = 0x23;

pub const F32: u8 = 0x30;
pub const F64: u8 = 0x31;

/// Strings start with a byte 0x40..=0x4F, depending on how many bytes are needed to encode
/// their length
pub const STRING_BASE: u8 = 0x40;

/// Byte arrays start with a byte 0x50..=0x5F, depending on how many bytes are needed to encode
/// their length
pub const ARRAY_BASE: u8 = 0x50;

/// Seqs start with a byte 0x50..=0x5F, depending on how many bytes are needed to encode
/// their length
pub const SEQ_BASE: u8 = 0x60;
