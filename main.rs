pub mod lexer;
pub mod ast;
pub mod parser;
use lexer::LexerContext;
use parser::parse;
use std::env;
use std::fs::File;
use std::io::{self, Read};

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut input = String::new();

    if args.len() > 1 {
        let filename = &args[1];
        let mut file = File::open(filename)?;
        file.read_to_string(&mut input)?;
    } else {
        io::stdin().read_to_string(&mut input)?;
    }

    let mut lexer = LexerContext::new(&input);
    let _ = parse(&mut lexer);

    Ok(())
}
