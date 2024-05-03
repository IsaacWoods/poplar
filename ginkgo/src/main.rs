use ginkgo::lex::Lex;
use std::io::{self, Write};

fn main() -> io::Result<()> {
    // TODO: repl
    println!("Hello, world!");

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
    }
}
