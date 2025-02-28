use crate::lexer::{Token, TokenItem};
use core::{
    fmt::{self, Display, Write},
    num::{ParseFloatError, ParseIntError},
};
use serde::de;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    UnknownToken,
    UnexpectedToken(Token, Expected),
    InvalidInteger(ParseIntError),
    InvalidFloat(ParseFloatError),
    TableAlreadyDefined,
    TrailingCharacters,
    MissingToken,
    Custom([u8; 64], usize),
    Unsupported([u8; 64], usize),
    FailedToLex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expected {
    Token(Token),
    LineStart,
    Value,
    Bool,
    String,
    MapStart,
    SeqStart,
    EolOrEof,
    Enum,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub range: core::ops::Range<usize>,
    pub kind: ErrorKind,
}

pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    pub fn new(range: core::ops::Range<usize>, kind: ErrorKind) -> Self {
        Self { range, kind }
    }

    pub fn unexpected(lexer: &crate::de::Deserializer<'_>, unexpected: Token, expected: Expected) -> Self {
        Self { range: lexer.tokens.inner().inner().span(), kind: ErrorKind::UnexpectedToken(unexpected, expected) }
    }

    pub fn unsupported<T: Display>(item: &TokenItem, msg: T) -> Self {
        let mut buf = [0u8; 64];
        let offset = {
            let mut wrapper = Wrapper::new(&mut buf);
            let _ = write!(&mut wrapper, "{}", msg);
            wrapper.offset
        };

        Error { range: item.range.clone(), kind: ErrorKind::Unsupported(buf, offset) }
    }

    pub fn end(de: &crate::de::Deserializer<'_>, kind: ErrorKind) -> Error {
        Error { range: de.input.len()..de.input.len(), kind }
    }
}

impl serde::ser::StdError for Error {}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        let mut buf = [0u8; 64];
        let offset = {
            let mut wrapper = Wrapper::new(&mut buf);
            let _ = write!(&mut wrapper, "{}", msg);
            wrapper.offset
        };

        Error { range: 0..0, kind: ErrorKind::Custom(buf, offset) }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.kind))
    }
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            ErrorKind::UnknownToken => "",
            ErrorKind::UnexpectedToken(token, expected) => {
                return f.write_fmt(format_args!("UnexpectedToken: {:?} - expected: {:?}", token, expected))
            }
            ErrorKind::InvalidInteger(parse_err) => {
                return f.write_fmt(format_args!("Failed to parse int: {:?}", parse_err))
            }
            ErrorKind::InvalidFloat(parse_err) => {
                return f.write_fmt(format_args!("Failed to parse float: {:?}", parse_err))
            }
            ErrorKind::TableAlreadyDefined => "Table already defined",
            ErrorKind::TrailingCharacters => "Trailing characters",
            ErrorKind::MissingToken => "Missing token",
            // SAFETY: We only format valid utf8 within these variants
            ErrorKind::Custom(bytes, len) => unsafe { core::str::from_utf8_unchecked(&bytes[..*len]) },
            ErrorKind::Unsupported(bytes, len) => unsafe { core::str::from_utf8_unchecked(&bytes[..*len]) },
            ErrorKind::FailedToLex => "Failed to lex",
        };
        f.write_str(s)
    }
}

// Wrapper type so that we can format a T: Display into a fixed size buffer
struct Wrapper<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> Wrapper<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Wrapper { buf, offset: 0 }
    }
}

impl<'a> fmt::Write for Wrapper<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();

        let remainder = &mut self.buf[self.offset..];
        if remainder.len() < bytes.len() {
            // Return error instead of panicking if out of space
            return Err(core::fmt::Error);
        }
        let remainder = &mut remainder[..bytes.len()];
        remainder.copy_from_slice(bytes);

        self.offset += bytes.len();

        Ok(())
    }
}
