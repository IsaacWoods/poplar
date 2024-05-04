use ginkgo::{interpreter::Interpreter, parse::Parser};
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut interpreter = Interpreter::new();

    loop {
        print!("> ");
        io::stdout().flush()?;

        // Get a line of input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let parser = Parser::new(&input);
        let ast = parser.parse().unwrap();

        println!("Result: {:?}", interpreter.eval(&ast));
    }
}
