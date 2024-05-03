use std::str::Chars;

#[derive(Clone, Copy, Debug)]
pub enum Token<'s> {
    /*
     * Symbols.
     */
    Plus,
    Minus,
    Dot,
    Equals,
    EqualEquals,
    Asterix,
    Slash,
    Colon,
    Semicolon,
    Comma,
    Bang,
    BangEquals,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    QuestionMark,

    /*
     * Literals.
     */
    // TODO: this makes `Token` quite a bit bigger. We could avoid this by providing a method to
    // extract it at time of usage from the stream? (same with all valued tokens)
    Identifier(&'s str),
    String(&'s str),
    Integer(isize),
    Decimal(f64),

    /*
     * Keywords.
     */
    Let,
    If,
    Else,
    For,
    Loop,
    While,
    True,
    False,
    Return,
    Fn,
}

// TODO: this could be `Token` and `Token` could be `TokenType` or something
#[derive(Clone, Copy, Debug)]
pub struct TokenInfo<'s> {
    typ: Token<'s>,
    offset: usize,
    length: usize,
}

pub struct Lex<'s> {
    source: &'s str,
    // TODO: I wonder if we, in a UTF8-aware world, want to iterate over grapheme clusters instead
    // (this may well be a fair bit slower??)
    stream: PeekingIter<Chars<'s>>,
    offset: usize,

    // Tracks info for the token that is currently being lexed
    start_offset: usize,
    current_length: usize,
}

impl<'s> Lex<'s> {
    pub fn new(source: &'s str) -> Lex<'s> {
        Lex { source, stream: PeekingIter::new(source.chars()), offset: 0, start_offset: 0, current_length: 0 }
    }

    pub fn advance(&mut self) -> Option<char> {
        let c = self.stream.next()?;
        self.offset += 1;
        self.current_length += 1;
        Some(c)
    }

    /// Returns the value of the currently-lexing token
    pub fn value(&self) -> &'s str {
        &self.source[self.start_offset..self.offset]
    }

    pub fn produce(&mut self, typ: Token<'s>) -> TokenInfo<'s> {
        TokenInfo { typ, offset: self.start_offset, length: self.current_length }
    }
}

impl<'s> Iterator for Lex<'s> {
    type Item = TokenInfo<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            /*
             * Each loop iteration starts a new token. This ensures skipped whitespace etc. is not
             * counted in tokens.
             */
            self.start_offset = self.offset;
            self.current_length = 0;
            let c = self.advance()?;

            match c {
                '+' => return Some(self.produce(Token::Plus)),
                '-' => return Some(self.produce(Token::Minus)),
                '.' => return Some(self.produce(Token::Dot)),
                '=' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(Token::EqualEquals));
                    }
                    _ => return Some(self.produce(Token::Equals)),
                },
                '*' => return Some(self.produce(Token::Asterix)),
                ':' => return Some(self.produce(Token::Colon)),
                ';' => return Some(self.produce(Token::Semicolon)),
                ',' => return Some(self.produce(Token::Comma)),
                '!' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(Token::BangEquals));
                    }
                    _ => return Some(self.produce(Token::Bang)),
                },
                '(' => return Some(self.produce(Token::LeftParen)),
                ')' => return Some(self.produce(Token::RightParen)),
                '{' => return Some(self.produce(Token::LeftBrace)),
                '}' => return Some(self.produce(Token::RightBrace)),
                '<' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(Token::LessEqual));
                    }
                    _ => return Some(self.produce(Token::LessThan)),
                },
                '>' => match self.stream.peek() {
                    Some('=') => {
                        self.advance();
                        return Some(self.produce(Token::GreaterEqual));
                    }
                    _ => return Some(self.produce(Token::GreaterThan)),
                },
                '?' => return Some(self.produce(Token::QuestionMark)),

                // TODO: parse comments, both line and block here
                '/' => return Some(self.produce(Token::Slash)),

                /*
                 * Skip whitespace.
                 */
                ' ' | '\t' | '\n' => continue,

                /*
                 * Number literals.
                 */
                c if c.is_digit(10) => {
                    // TODO: parse hex
                    // TODO: support octal?

                    while self.stream.peek().map_or(false, |c| c.is_digit(10)) {
                        self.advance();
                    }

                    /*
                     * Handle floating-point numbers. We want to consume the decimal point, but
                     * only if it is followed by another digit.
                     */
                    if self.stream.peek().map_or(false, |c| c == '.')
                        && self.stream.peek_next().map_or(false, |c| c.is_digit(10))
                    {
                        self.advance();
                        while self.stream.peek().map_or(false, |c| c.is_digit(10)) {
                            self.advance();
                        }
                        return Some(self.produce(Token::Decimal(str::parse(self.value()).unwrap())));
                    } else {
                        return Some(self.produce(Token::Integer(str::parse(self.value()).unwrap())));
                    }
                }

                '"' => {
                    // TODO: string literals
                    todo!()
                }

                /*
                 * Parse keywords and identifiers.
                 */
                c if c.is_alphanumeric() => {
                    /*
                     * Do a maximal munch to make sure identifiers that start with reserved
                     * keywords are not mistaken for those keywords.
                     */
                    while self.stream.peek().map_or(false, char::is_alphanumeric) {
                        self.advance()?;
                    }

                    match self.value() {
                        "let" => return Some(self.produce(Token::Let)),
                        "if" => return Some(self.produce(Token::If)),
                        "else" => return Some(self.produce(Token::Else)),
                        "for" => return Some(self.produce(Token::For)),
                        "loop" => return Some(self.produce(Token::Loop)),
                        "while" => return Some(self.produce(Token::While)),
                        "true" => return Some(self.produce(Token::True)),
                        "false" => return Some(self.produce(Token::False)),
                        "return" => return Some(self.produce(Token::Return)),
                        "fn" => return Some(self.produce(Token::Fn)),
                        other => return Some(self.produce(Token::Identifier(other))),
                    }
                }

                // TODO: issue a lex error rather than panicking
                other => panic!("Unknown char in lex: {:?}", other),
            }
        }
    }
}

/// Wraps an `Iterator` and provides exactly two items of lookahead. This is enough to implement a
/// wide variety of lexers and parsers.
struct PeekingIter<I>
where
    I: Iterator,
    I::Item: Clone,
{
    inner: I,
    peek: Option<I::Item>,
    peek_next: Option<I::Item>,
}

impl<I> PeekingIter<I>
where
    I: Iterator,
    I::Item: Clone,
{
    pub fn new(inner: I) -> PeekingIter<I> {
        PeekingIter { inner, peek: None, peek_next: None }
    }

    pub fn peek(&mut self) -> Option<I::Item> {
        if let Some(peek) = &self.peek {
            Some(peek.clone())
        } else {
            let next = self.inner.next()?;
            self.peek = Some(next.clone());
            Some(next)
        }
    }

    pub fn peek_next(&mut self) -> Option<I::Item> {
        assert!(self.peek.is_some());

        if let Some(peek) = &self.peek_next {
            Some(peek.clone())
        } else {
            let next = self.inner.next()?;
            self.peek_next = Some(next.clone());
            Some(next)
        }
    }
}

impl<I> Iterator for PeekingIter<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already peeked forward, use that
        if let Some(peeked) = self.peek.take() {
            // Bump the 2nd lookahead item up a space
            self.peek = self.peek_next.take();

            Some(peeked)
        } else {
            self.inner.next()
        }
    }
}
