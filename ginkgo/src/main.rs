use ginkgo::{
    parse::Parser,
    vm::{Value, Vm},
};
use rustyline::{
    error::ReadlineError,
    validate::MatchingBracketValidator,
    Completer,
    Editor,
    Helper,
    Highlighter,
    Hinter,
    Validator,
};
use std::{io, path::Path};

fn main() -> io::Result<()> {
    /*
     * TODO: things to experiment with: `gc-arena` crate for garbage-collected values long-term +
     * `rustyline` for a decent REPL interface (either wholesale or being inspired by it (at least
     * for the Poplar version this will probs be required))
     * - miette for fancy diagnostic reporting
     */

    let mut vm = Vm::new();

    vm.define_native_fn("print", |args| {
        assert!(args.len() == 1);
        let value = args.get(0).unwrap();
        println!("PRINT: {:?}", value);
        Value::Unit
    });

    // If we were passed a path, load and run that file
    if std::env::args().count() > 1 {
        let path = std::env::args().nth(1).unwrap();
        println!("Executing script at {}", path);

        let source = std::fs::read_to_string(Path::new(&path)).unwrap();
        let parser = Parser::new(&source);
        let chunk = parser.parse().unwrap();
        vm.interpret(chunk);
    }

    let mut rl = Editor::new().unwrap();
    rl.set_helper(Some(ReplHelper { validator: MatchingBracketValidator::new() }));
    // TODO: can load history here if wanted

    loop {
        let line = rl.readline("> ");
        match line {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();

                let parser = Parser::new(&line);
                let chunk = parser.parse().unwrap();

                vm.interpret(chunk);
            }
            Err(ReadlineError::Interrupted) => {
                println!("Ctrl-C");
                break Ok(());
            }
            Err(ReadlineError::Eof) => {
                println!("Ctrl-D");
                break Ok(());
            }
            Err(err) => {
                panic!("Error: {:?}", err);
            }
        }
    }
}

// TODO: not sure I love using a derive-macro here. We could just impl it for good.
#[derive(Helper, Completer, Hinter, Highlighter, Validator)]
pub struct ReplHelper {
    #[rustyline(Validator)]
    validator: MatchingBracketValidator,
}
