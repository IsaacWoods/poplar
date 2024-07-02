use ginkgo::{
    interpreter::{Interpreter, Value},
    parse::Parser,
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
    let mut interpreter = Interpreter::new();

    /*
     * TODO: things to experiment with: `gc-arena` crate for garbage-collected values long-term +
     * `rustyline` for a decent REPL interface (either wholesale or being inspired by it (at least
     * for the Poplar version this will probs be required))
     */

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

    let mut rl = Editor::new().unwrap();
    rl.set_helper(Some(ReplHelper { validator: MatchingBracketValidator::new() }));
    // TODO: can load history here if wanted

    // TODO: we want a much more advanced REPL than this, to the point where it'll definitely need
    // intimate support from the parser itself. It'll be useful in a range of settings (thinking
    // Poplar's shell), so we can include at least some of it in the library portion.
    //
    // XXX: the answer, it turns out, is to do validation outside the parser (maybe by using simple
    // rules from a token stream out of a lexer) to tell when input is incomplete (i.e. when we're
    // inside a structure or construct). We can either build this around `rustyline`, or use our
    // own thing as we may well need it anyway to use from inside Poplar (I'm not sure how much of
    // VT100 we want to emulate?).
    loop {
        let line = rl.readline("> ");
        match line {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();

                let parser = Parser::new(&line);
                let stmts = parser.parse().unwrap();

                for statement in stmts {
                    if let Some(result) = interpreter.eval_stmt(statement) {
                        println!("Result: {:?}", result);
                    }
                }
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
