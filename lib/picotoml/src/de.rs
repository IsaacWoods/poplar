use crate::{
    error::{Error, ErrorKind, Expected, Result},
    expect_next,
    expect_next_peeked,
    lexer::{self, BracketType::*, LexerIterator, Token, TokenItem},
    peeking::PeekingIterator,
    range_to_str,
};
use core::{ops::Range, str::FromStr};
use serde::{
    de::{Deserialize, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor},
    Deserializer as SerdeDeserializer,
};

pub struct Deserializer<'de> {
    // This string starts with the input data and characters are truncated off
    // the beginning as data is parsed.
    pub(crate) input: &'de str,
    pub(crate) tokens: PeekingIterator<LexerIterator<'de>>,
    depth: usize,
    in_array_table: Option<&'de str>,
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut de = Deserializer::from_str(s);
    let t = T::deserialize(&mut de)?;
    Ok(t)
}

impl<'de> Deserializer<'de> {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &'de str) -> Self {
        Deserializer {
            input,
            tokens: PeekingIterator::new(LexerIterator::new(lexer::lex(input))),
            depth: 0,
            in_array_table: None,
        }
    }

    fn handle_opt_item(&self, opt: Option<lexer::TokenItem>) -> Result<TokenItem> {
        match opt {
            Some(item) => Ok(item),
            None => Err(Error::new(self.input.len()..self.input.len(), ErrorKind::MissingToken)),
        }
    }

    /// Returns the next element to be returned by [`Self::next`].
    fn peek(&mut self) -> Result<TokenItem> {
        // Always reset the cursor so that we can peek anywhere and always get the first value
        // Callers who need to read ahead must call peek_next() until they are satisfied
        self.tokens.reset_cursor();
        let item = self.tokens.peek().cloned();
        self.handle_opt_item(item)
    }

    /// Advances the peek cursor and peeks at the next element
    fn peek_next(&mut self) -> Result<TokenItem> {
        let item = self.tokens.peek_next().cloned();
        self.handle_opt_item(item)
    }

    /// Consumes the next token item from the lexer
    fn next(&mut self) -> Result<TokenItem> {
        let item = self.tokens.next();
        self.handle_opt_item(item)
    }

    fn range_to_str(&self, range: Range<usize>) -> &str {
        if range.start > range.end || range.end > self.input.len() {
            &self.input[0..0]
        } else {
            &self.input[range.start..range.end]
        }
    }

    /// Consumes any whitespace tokens in the token buffer. Returns Err(...) When out of tokens, or
    /// Ok(()) to indicate that the next token is valid and non-whitespace
    fn consume_whitespace_and_comments(&mut self) -> Result<()> {
        loop {
            let _ = match self.peek()?.token {
                // Tabs and spaces are consumed implicitly by the lexer so we only need to eat Eol here
                Token::Eol | Token::Comment => self.next().unwrap(),
                _ => return Ok(()),
            };
        }
    }

    fn expect_eol_or_eof(&mut self) -> Result<()> {
        match self.next() {
            Ok(token) => match token.token {
                Token::Eol => Ok(()),
                t => Err(Error::new(token.range, ErrorKind::UnexpectedToken(t, Expected::EolOrEof))),
            },
            Err(_eof) => Ok(()),
        }
    }

    fn parse_string(&mut self) -> Result<&'de str> {
        // Peek at data first
        let item = self.peek()?;
        let base = range_to_str!(self, item);
        let range = item.range;
        let s = expect_next! {
            self,
            Expected::String,
            Token::BareString => base,
            Token::QuotedString => &base[1..range.len() - 1],
            Token::MutlilineString => &base[3..range.len() - 3]
        };
        // TODO: Check if there are any escape sequences because we can't support them
        Ok(s)
    }

    fn parse_bool(&mut self) -> Result<bool> {
        expect_next! {
            self,
            Expected::Bool,
            Token::BoolLit(value) => Ok(value)
        }
    }
}

macro_rules! deserialize_int_from_str {
    ($self:ident, $visitor:ident, $typ:ident, $visit_fn:ident) => {{
        let item = $self.next()?;
        let s = &$self.range_to_str(item.range.clone());
        let v = $typ::from_str(s).map_err(|e| Error::new(item.range, ErrorKind::InvalidInteger(e)))?;

        $visitor.$visit_fn(v)
    }};
}

macro_rules! deserialize_float_from_str {
    ($self:ident, $visitor:ident, $typ:ident, $visit_fn:ident) => {{
        let item = $self.next()?;
        let s = &$self.range_to_str(item.range.clone());
        let v = $typ::from_str(s).map_err(|e| Error::new(item.range, ErrorKind::InvalidFloat(e)))?;

        $visitor.$visit_fn(v)
    }};
}

impl<'de, 'a> SerdeDeserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    // Entry point of parsing. Looks at token and decides what to do
    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, i8, visit_i8)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, i16, visit_i16)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, i32, visit_i32)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, i64, visit_i64)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, u8, visit_u8)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, u16, visit_u16)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, u32, visit_u32)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_int_from_str!(self, visitor, u64, visit_u64)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_float_from_str!(self, visitor, f32, visit_f32)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        deserialize_float_from_str!(self, visitor, f64, visit_f64)
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Parse a string, check that it is one character, call `visit_char`.
        unimplemented!()
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = expect_next_peeked! {
            self, Expected::SeqStart,
            Token::SquareBracket(Open) => {
                //Consume [ because we peeked earlier
                self.next().unwrap();

                // Give the visitor access to each element of the sequence.
                self.depth += 1;
                let v = visitor.visit_seq(CommaSeparated::new(&mut self))?;
                self.depth -= 1;
                // Parse the closing bracket of the sequence.
                self.next()?.expect(Token::SquareBracket(Close))?;
                v
            },
            Token::DoubleSquareBracket(Open) => {
                self.depth += 1;
                let v = visitor.visit_seq(ArrayOfTables::new(&mut self))?;
                self.depth -= 1;

                v
            }
        };

        Ok(v)
    }

    // Tuples look just like sequences in JSON. Some formats may be able to
    // represent tuples more efficiently.
    //
    // As indicated by the length parameter, the `Deserialize` implementation
    // for a tuple in the Serde data model is required to know the length of the
    // tuple before even looking at the input data.
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Tuple structs look just like sequences in JSON.
    fn deserialize_tuple_struct<V>(self, _name: &'static str, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Much like `deserialize_seq` but calls the visitors `visit_map` method
    // with a `MapAccess` implementation, rather than the visitor's `visit_seq`
    // method with a `SeqAccess` implementation.
    fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.depth += 1;
        let r = visitor.visit_map(KeyValuePairs::new(&mut self));
        self.depth -= 1;
        r
    }

    // Structs look just like maps in JSON.
    //
    // Notice the `fields` parameter - a "struct" in the Serde data model means
    // that the `Deserialize` implementation is required to know what the fields
    // are before even looking at the input data. Any key-value pairing in which
    // the fields cannot be known ahead of time is probably a map.
    fn deserialize_struct<V>(
        mut self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.depth += 1;
        let r = visitor.visit_map(KeyValuePairs::new(&mut self));
        self.depth -= 1;
        r
    }

    fn deserialize_enum<V>(
        self,
        _enum_name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // An enum is in the format <Enum name> = "Normal_Variant"
        // Or <Enum Name> = {
        expect_next_peeked! {
            self, Expected::Enum,
            Token::QuotedString => {
                visitor.visit_enum(self.parse_string()?.into_deserializer())
            }
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
        /*

        match token {
            Token::BoolLit(_)
            | Token::NumberLit
            | Token::QuotedString
            | Token::LiteralString
            | Token::MutlilineString
            | Token::LiteralMutlilineString => {}
            _ => unimplemented!("deserialize_ignored_any is not implemented for {:?}", token),
        }

        visitor.visit_none()
        */
    }
}

struct CommaSeparated<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    first: bool,
}

impl<'a, 'de> CommaSeparated<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self { de, first: true }
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for CommaSeparated<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        // Check if there are no more elements.
        if let Token::SquareBracket(Close) = self.de.peek()?.token {
            return Ok(None);
        }

        // Comma is required before every element except the first.
        if !self.first {
            let de_self = &mut self.de;
            de_self.next()?.expect(Token::Comma)?;
        }
        self.first = false;
        // Deserialize an array element.
        seed.deserialize(&mut *self.de).map(Some)
    }
}

// `MapAccess` is provided to the `Visitor` to give it the ability to iterate
// through entries of the map.
impl<'de, 'a> MapAccess<'de> for CommaSeparated<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        // Check if there are no more elements.
        if let Token::CurlyBracket(Close) = self.de.peek()?.token {
            return Ok(None);
        }

        // Comma is required before every entry except the first.
        if !self.first {
            let de_self = &mut self.de;
            de_self.next()?.expect(Token::Comma)?;
        }
        self.first = false;
        // Deserialize a map key.
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let de_self = &mut self.de;
        de_self.next()?.expect(Token::Equals)?;

        self.de.depth += 1;
        let r = seed.deserialize(&mut *self.de);
        self.de.depth -= 1;
        r
    }
}

struct KeyValuePairs<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    first: bool,
}

impl<'a, 'de> KeyValuePairs<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self { de, first: true }
    }
}

impl<'de, 'a> MapAccess<'de> for KeyValuePairs<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        // Here we are getting the name of a particular key. This can either be a "key_name = 'value'" pair line
        // Or a table, where the value is a sub-struct with more key-value pairs
        let de_self = &mut self.de;
        if de_self.consume_whitespace_and_comments().is_err() {
            // consume_whitespace_and_comments returns error on end of stream
            return Ok(None);
        }
        let token_item = de_self.peek().unwrap();
        let token = token_item.token;
        let result = match token {
            // Simple key value pair - parse name
            Token::BareString => seed.deserialize(&mut *self.de).map(Some),
            Token::SquareBracket(Open) => {
                if !self.first && de_self.depth > 1 {
                    // Start of new table that is not part of our entries
                    return Ok(None);
                }
                // Table in format:
                // [...]
                // <struct data>

                //Consume [
                de_self.next().unwrap();
                let key_item = de_self.next()?;
                key_item.expect(Token::BareString)?;
                // Read string within [...]
                let result =
                    seed.deserialize(SingleStrDeserializer { s: de_self.range_to_str(key_item.range) }).map(Some);

                de_self.next()?.expect(Token::SquareBracket(Close))?;
                de_self.consume_whitespace_and_comments()?;

                result
            }
            Token::DoubleSquareBracket(Open) => {
                // Table in format:
                // [[...]]
                // <element data>

                let second = de_self.peek_next()?;
                second.expect(Token::BareString)?;
                let mut name = range_to_str!(de_self, second);
                if let Some(outer_name) = de_self.in_array_table {
                    // We are inside a table already
                    let next_item = de_self.peek_next()?;
                    if let Token::Period = next_item.token {
                        if name != outer_name {
                            return Err(Error::unsupported(
                                &next_item,
                                "current table path start doesn't equal last table path",
                            ));
                        }
                        // Move the cursor to where we are peaked at so that deserialize below gets
                        //the right name
                        let name_token = de_self.peek_next()?;
                        name = range_to_str!(de_self, name_token);
                    } else if name == outer_name {
                        return Ok(None);
                    } else {
                        // We found a table with a different name than ours. Because
                        // `de_self.in_array_table` is `Some`, we are too deep to parse this because
                        // the next table that were seeing is at the root level
                        return Ok(None);
                    }
                } else {
                    de_self.in_array_table = Some(name);
                }

                // Read string within [[...]]
                let result = seed.deserialize(SingleStrDeserializer { s: name }).map(Some);

                result
            }
            _ => unimplemented!(),
        };
        self.first = false;
        result
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let de_self = &mut *self.de;

        let allow_consume_whitespace = expect_next_peeked! {
            de_self, Expected::Value,
            Token::Equals => {
                // Consume equals
                let _ = de_self.next().unwrap();
                false
            },
            Token::DoubleSquareBracket(Open) => {
                true
            },
            Token::BareString => {
                true
            }
        };

        de_self.depth += 1;
        let result = seed.deserialize(&mut *de_self);
        self.de.depth -= 1;

        let de_self = &mut *self.de;
        if de_self.depth <= 2 && allow_consume_whitespace {
            // Failure indicates end of stream so dont fail here because we will retry in next_key_seed,
            // see the end of stream there, then end the map
            let _ = self.de.consume_whitespace_and_comments();
        } else {
            // Normal key-value pair - Consume end of line
            self.de.expect_eol_or_eof()?;
        }

        result
    }
}

struct SingleStrDeserializer<'a> {
    s: &'a str,
}

impl<'de, 'a> SerdeDeserializer<'de> for SingleStrDeserializer<'a> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.s)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

struct ArrayOfTables<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    table_name: Option<&'a str>,
}

impl<'a, 'de> ArrayOfTables<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self { de, table_name: None }
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for ArrayOfTables<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let de_self = &mut self.de;
        match de_self.peek() {
            Ok(token) => token.expect(Token::DoubleSquareBracket(Open))?,
            // End of stream
            Err(_) => return Ok(None),
        }

        let name_item = de_self.peek_next()?;
        name_item.expect(Token::BareString)?;
        let name = range_to_str!(de_self, name_item);

        match self.table_name {
            Some(table_name) => {
                if table_name != name {
                    // We used peek above so that we can stop reading this sequence and leave the
                    // start of the new table un-consumed.
                    // This keeps the code generic for the start of any table
                    unimplemented!();
                }
            }
            None => self.table_name = Some(name),
        }

        // Consume [[ and <name>
        let _ = de_self.next().unwrap();
        let _ = de_self.next().unwrap();

        de_self.next()?.expect(Token::DoubleSquareBracket(Close))?;

        // Deserialize an array element (probably a struct)
        self.de.depth += 1;
        let r = seed.deserialize(&mut *self.de).map(Some);
        self.de.depth -= 1;
        r
    }
}
