pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;
use inkwell::{context::Context, values::BasicValueEnum, values::PointerValue};
use lexer::LexerContext;
use parser::parse;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, ErrorKind, Read};

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

    let context = Context::create();
    let mut module = context.create_module("main");
    let mut builder = context.create_builder();
    let mut vars: HashMap<String, BasicValueEnum> = HashMap::new();
    if let Ok(funcs) = parse(&mut lexer) {
        for f in funcs {
            f.codegen(&context, &mut builder, &mut module, &mut vars)
                .map_err(|e: String| io::Error::new(ErrorKind::Other, e))?;
        }
    }

    println!("{}", module.print_to_string().to_string());
    Ok(())
}
