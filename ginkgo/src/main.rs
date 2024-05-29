use ginkgo::{
    interpreter::{Interpreter, Value},
    parse::Parser,
};
use std::{
    io::{self, Write},
    path::Path,
};

fn main() -> io::Result<()> {
    let mut interpreter = Interpreter::new();

    interpreter.define_native_function("print", |params| {
        assert!(params.len() == 1);
        let value = params.get(0).unwrap();
        println!("PRINT: {:?}", value);
        Value::Unit
    });

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
            if let Some(result) = interpreter.eval_stmt(statement) {
                println!("Result: {:?}", result);
            }
        }
    }
}
