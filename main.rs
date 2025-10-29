pub mod ast;
pub mod codegen;
pub mod externs;
pub mod lexer;
pub mod parser;
use codegen::CodegenContext;
use externs::FfiRegistry;
use inkwell::{context::Context, OptimizationLevel};
use lexer::LexerContext;
use parser::ParserContext;
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

    // Lex the entire input into tokens
    let mut lexer = LexerContext::new();
    lexer.lex(&input);

    let mut parser = ParserContext::new();
    parser
        .parse(&mut lexer)
        .map_err(|e: String| io::Error::new(ErrorKind::Other, e))?;


    let context = Context::create();
    let mut cg = CodegenContext::new(&context, "main");
    let execution_engine = cg
        .module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to create JIT: {}", e)))?;

    let ffi_registry = FfiRegistry::new();
    cg.codegen(&parser, &ffi_registry, &execution_engine)
        .map_err(|e: String| io::Error::new(ErrorKind::Other, e))?;

    println!("{}", cg.module.print_to_string().to_string());

    // Execute the main function via JIT
    unsafe {
        let main_fn = execution_engine
            .get_function::<unsafe extern "C" fn() -> f64>("main")
            .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to get main: {}", e)))?;
        let result = main_fn.call();
        println!("\nResult: {}", result);
    }

    Ok(())
}
