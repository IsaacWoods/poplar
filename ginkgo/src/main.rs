use ginkgo::{lex::Lex, parse::Parser};
use std::io::{self, Write};

fn main() -> io::Result<()> {
    loop {
        print!("> ");
        io::stdout().flush()?;

        // Get a line of input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let lex = Lex::new(&input);
        for token in lex {
            println!("Token: {:?}", token);
        }

        let parser = Parser::new(&input);
        parser.parse().unwrap();
    }
}
