use ginkgo::{interpreter::Interpreter, parse::Parser};
use std::{
    io::{self, Write},
    path::Path,
};

fn main() -> io::Result<()> {
    let mut interpreter = Interpreter::new();

    // If we were passed a path, load and run that file
    if std::env::args().count() > 1 {
        let path = std::env::args().nth(1).unwrap();
        println!("Executing script at {}", path);

        let source = std::fs::read_to_string(Path::new(&path)).unwrap();
        let parser = Parser::new(&source);
        let output = parser.parse().unwrap();

        for statement in output {
            interpreter.eval_stmt(statement);
        }
    }

    loop {
        print!("> ");
        io::stdout().flush()?;

        // Get a line of input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let parser = Parser::new(&input);
        let stmts = parser.parse().unwrap();

        for statement in stmts {
            interpreter.eval_stmt(statement);
        }
    }
}
