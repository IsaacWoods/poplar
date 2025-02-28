use crate::{Error, ErrorKind, Expected};
use core::ops::Range;
use logos::Logos;

pub type Lexer<'a> = logos::Lexer<'a, Token>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BracketType {
    Open,
    Close,
}

#[derive(Logos, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Token {
    #[regex("[A-Za-z0-9_-]+")]
    BareString,

    #[regex("#[^\n|\r\n]*")]
    Comment,

    #[token(",")]
    Comma,

    #[token("=")]
    Equals,

    #[regex("[ |\t]+", logos::skip, priority = 2)]
    Whitespace,

    #[regex("'[^']*'")]
    LiteralString,

    #[token("[[", |_| BracketType::Open)]
    #[token("]]", |_| BracketType::Close)]
    DoubleSquareBracket(BracketType),

    #[token("[", |_| BracketType::Open)]
    #[token("]", |_| BracketType::Close)]
    SquareBracket(BracketType),

    #[token("{", |_| BracketType::Open)]
    #[token("}", |_| BracketType::Close)]
    CurlyBracket(BracketType),

    #[regex("'''[^']*'''", priority = 3)]
    LiteralMutlilineString,

    #[regex("\"\"\"[^\"]*\"\"\"", priority = 3)]
    MutlilineString,

    #[regex("[\n|\r\n]")]
    Eol,

    #[regex("[+-]?([0-9]*[.])?[0-9]+", priority = 2)]
    NumberLit,

    #[token("true", |_| true)]
    #[token("false", |_| false)]
    BoolLit(bool),

    #[token(".")]
    Period,

    #[regex("\"[^\"]*\"", priority = 2)]
    QuotedString,
}

pub fn lex(input: &str) -> logos::Lexer<'_, Token> {
    Token::lexer(input)
}

pub struct LexerIterator<'a> {
    lexer: Lexer<'a>,
}

impl<'a> LexerIterator<'a> {
    pub fn new(lexer: Lexer<'a>) -> Self {
        Self { lexer }
    }

    pub fn inner(&self) -> &Lexer<'a> {
        &self.lexer
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenItem {
    pub token: Token,
    pub range: Range<usize>,
}

impl<'a> TokenItem {
    pub fn new(t: Token, r: Range<usize>) -> Self {
        Self { token: t, range: r }
    }

    pub fn expect(&self, expected: Token) -> crate::error::Result<()> {
        match self.token {
            token if token == expected => Ok(()),
            _ => Err(Error::new(
                self.range.clone(),
                ErrorKind::UnexpectedToken(self.token, Expected::Token(expected)),
            )),
        }
    }
}

/// Returns a slice containing the text of the given token item using the provided deserializer
#[macro_export]
macro_rules! range_to_str {
    ($self:ident, $tok_item:expr) => {
        &$self.input[$tok_item.range.start..$tok_item.range.end]
    };
}

#[macro_export]
macro_rules! expect_next {
    ($self:ident, $expected:expr, $($matcher:pat $(if $pred:expr)* => $result:expr),*) => {
        {
            let item = $self.next()?;
            match item.token {
                 $($matcher $(if $pred)* => $result),*
                ,
                _ =>
                    return $crate::error::Result::Err(
                        $crate::error::Error::new(
                            item.range,
                            $crate::error::ErrorKind::UnexpectedToken(item.token, $expected)
                        )
                    ),
            }
        }
    };
}

#[macro_export]
macro_rules! expect_next_peeked {
    ($self:ident, $expected:expr, $($matcher:pat $(if $pred:expr)* => $result:expr),*) => {
        {
            let item = $self.peek()?;
            match item.token {
                 $($matcher $(if $pred)* => $result),*
                ,
                _ =>
                    return $crate::error::Result::Err(
                        $crate::error::Error::new(
                            item.range,
                            $crate::error::ErrorKind::UnexpectedToken(item.token, $expected)
                        )
                    ),
            }
        }
    };
}

#[macro_export]
macro_rules! expect_next_with_item {
    ($self:ident, $expected:expr, $($matcher:pat $(if $pred:expr)* => $result:expr),*) => {
        {
            let item = $self.next()?;
            let second = match item.token {
                 $($matcher $(if $pred)* => $result),*
                ,
                _ =>
                    return $crate::error::Result::Err(
                        $crate::error::Error::new(
                            item.range,
                            $crate::error::ErrorKind::UnexpectedToken(item.token, $expected)
                        )
                    ),
            };
            (item, second)
        }
    };
}

impl<'a> Iterator for LexerIterator<'a> {
    type Item = TokenItem;

    fn next(&mut self) -> Option<Self::Item> {
        self.lexer.next().map(|t| TokenItem::new(t.unwrap(), self.lexer.span()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BracketType::*;

    fn test_lex(toml: &'static str, expected_tokens: impl IntoIterator<Item = (Token, &'static str)>) {
        let mut lex = Token::lexer(toml);

        for (expected_token, expected_str) in expected_tokens {
            let token = lex.next();
            assert_eq!(token, Some(Ok(expected_token)));
            assert_eq!(expected_str, lex.slice());
        }
        assert_eq!(lex.next(), None);
    }

    #[test]
    fn lex1() {
        test_lex(
            r#"#bare_str
    # This is a comment

    #^ Empty line above

            "#,
            [
                (Token::Comment, "#bare_str"),
                (Token::Eol, "\n"),
                (Token::Comment, "# This is a comment"),
                (Token::Eol, "\n"),
                (Token::Eol, "\n"),
                (Token::Comment, "#^ Empty line above"),
                (Token::Eol, "\n"),
                (Token::Eol, "\n"),
            ],
        );
    }

    #[test]
    fn lex2() {
        test_lex(
            r#"
    server = "test"
    "my fav number" = 7
    "127.0.0.1" = 15
            "#,
            [
                (Token::Eol, "\n"),
                (Token::BareString, "server"),
                (Token::Equals, "="),
                (Token::QuotedString, "\"test\""),
                (Token::Eol, "\n"),
                (Token::QuotedString, "\"my fav number\""),
                (Token::Equals, "="),
                (Token::NumberLit, "7"),
                (Token::Eol, "\n"),
                (Token::QuotedString, "\"127.0.0.1\""),
                (Token::Equals, "="),
                (Token::NumberLit, "15"),
                (Token::Eol, "\n"),
            ],
        );
    }

    #[test]
    fn lex3() {
        test_lex(
            "test = \"test\"\na\n#ABC\na",
            [
                (Token::BareString, "test"),
                (Token::Equals, "="),
                (Token::QuotedString, "\"test\""),
                (Token::Eol, "\n"),
                (Token::BareString, "a"),
                (Token::Eol, "\n"),
                (Token::Comment, "#ABC"),
                (Token::Eol, "\n"),
                (Token::BareString, "a"),
            ],
        );
    }

    #[test]
    fn lex4() {
        test_lex(
            r#"
             """multi line test
  please parse these newlines
   """ = '''
me too!
            '''
            "#,
            [
                (Token::Eol, "\n"),
                (Token::MutlilineString, "\"\"\"multi line test\n  please parse these newlines\n   \"\"\""),
                (Token::Equals, "="),
                (Token::LiteralMutlilineString, "'''\nme too!\n            '''"),
                (Token::Eol, "\n"),
            ],
        );
    }

    #[test]
    fn lex5() {
        test_lex(
            r#"
             
numbers = [ 0.1, 0.2, 0.5, 1, 2, 5 ]
contributors = [
  "Foo Bar <foo@example.com>",
  { name = "Baz Qux", email = "bazqux@example.com", url = "https://example.com/bazqux" }
]
            "#,
            [
                (Token::Eol, "\n"),
                (Token::Eol, "\n"),
                (Token::BareString, "numbers"),
                (Token::Equals, "="),
                (Token::SquareBracket(Open), "["),
                (Token::NumberLit, "0.1"),
                (Token::Comma, ","),
                (Token::NumberLit, "0.2"),
                (Token::Comma, ","),
                (Token::NumberLit, "0.5"),
                (Token::Comma, ","),
                (Token::NumberLit, "1"),
                (Token::Comma, ","),
                (Token::NumberLit, "2"),
                (Token::Comma, ","),
                (Token::NumberLit, "5"),
                (Token::SquareBracket(Close), "]"),
                (Token::Eol, "\n"),
                (Token::BareString, "contributors"),
                (Token::Equals, "="),
                (Token::SquareBracket(Open), "["),
                (Token::Eol, "\n"),
                (Token::QuotedString, "\"Foo Bar <foo@example.com>\""),
                (Token::Comma, ","),
                (Token::Eol, "\n"),
                (Token::CurlyBracket(Open), "{"),
                (Token::BareString, "name"),
                (Token::Equals, "="),
                (Token::QuotedString, "\"Baz Qux\""),
                (Token::Comma, ","),
                (Token::BareString, "email"),
                (Token::Equals, "="),
                (Token::QuotedString, "\"bazqux@example.com\""),
                (Token::Comma, ","),
                (Token::BareString, "url"),
                (Token::Equals, "="),
                (Token::QuotedString, "\"https://example.com/bazqux\""),
                (Token::CurlyBracket(Close), "}"),
                (Token::Eol, "\n"),
                (Token::SquareBracket(Close), "]"),
                (Token::Eol, "\n"),
            ],
        );
    }

    #[test]
    fn lex6() {
        test_lex(
            r#"
[[]]
a = 3564
b = false"#,
            [
                (Token::Eol, "\n"),
                (Token::DoubleSquareBracket(Open), "[["),
                (Token::DoubleSquareBracket(Close), "]]"),
                (Token::Eol, "\n"),
                (Token::BareString, "a"),
                (Token::Equals, "="),
                (Token::NumberLit, "3564"),
                (Token::Eol, "\n"),
                (Token::BareString, "b"),
                (Token::Equals, "="),
                (Token::BoolLit(false), "false"),
            ],
        );
    }

    #[test]
    fn lex7() {
        test_lex(
            r#"
            [[array]]
            [[array.sub]]
    server = "test"
    "my fav number" = 7.564
    "127.0.0.1" = 15.56
            "#,
            [
                (Token::Eol, "\n"),
                (Token::DoubleSquareBracket(Open), "[["),
                (Token::BareString, "array"),
                (Token::DoubleSquareBracket(Close), "]]"),
                (Token::Eol, "\n"),
                (Token::DoubleSquareBracket(Open), "[["),
                (Token::BareString, "array"),
                (Token::Period, "."),
                (Token::BareString, "sub"),
                (Token::DoubleSquareBracket(Close), "]]"),
                (Token::Eol, "\n"),
                (Token::BareString, "server"),
                (Token::Equals, "="),
                (Token::QuotedString, "\"test\""),
                (Token::Eol, "\n"),
                (Token::QuotedString, "\"my fav number\""),
                (Token::Equals, "="),
                (Token::NumberLit, "7.564"),
                (Token::Eol, "\n"),
                (Token::QuotedString, "\"127.0.0.1\""),
                (Token::Equals, "="),
                (Token::NumberLit, "15.56"),
                (Token::Eol, "\n"),
            ],
        );
    }
}
