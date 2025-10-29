pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;
use inkwell::{context::Context, values::BasicValueEnum, OptimizationLevel};
use lexer::LexerContext;
use parser::parse;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};

extern "C" fn putchard(x: f64) -> f64 {
    print!("{}", x as u8 as char);
    io::stdout().flush().unwrap();
    0.0
}

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
    let execution_engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to create JIT: {}", e)))?;

    // Register external functions for JIT
    execution_engine.add_global_mapping(&module.add_function(
        "putchard",
        context.f64_type().fn_type(&[context.f64_type().into()], false),
        None
    ), putchard as usize);

    let mut builder = context.create_builder();
    let mut vars: HashMap<String, BasicValueEnum> = HashMap::new();

    // Create main() function upfront to hold all top-level expressions
    let f64_type = context.f64_type();
    let main_fn_type = f64_type.fn_type(&[], false);
    let main_func = module.add_function("main", main_fn_type, None);
    let main_entry = context.append_basic_block(main_func, "entry");
    builder.position_at_end(main_entry);

    let mut last_result: Option<BasicValueEnum> = None;

    if let Ok(funcs) = parse(&mut lexer) {
        for f in funcs {
            if f.name == "_top_level_expr" {
                // Codegen top-level expression directly into main
                if let Some(result) = f
                    .body
                    .codegen(&context, &mut builder, &module, &mut vars)
                    .map_err(|e: String| io::Error::new(ErrorKind::Other, e))?
                {
                    last_result = Some(result);
                }
            } else {
                // Codegen regular function (this repositions the builder)
                f.codegen(&context, &mut builder, &mut module, &mut vars)
                    .map_err(|e: String| io::Error::new(ErrorKind::Other, e))?;

                // Reposition builder back to main's entry for next top-level expr
                builder.position_at_end(main_entry);
            }
        }
    }

    // Add return statement to main with the last result
    builder.position_at_end(main_entry);
    if let Some(ret_val) = last_result {
        builder.build_return(Some(&ret_val)).map_err(|e| {
            io::Error::new(ErrorKind::Other, format!("Failed to build return: {}", e))
        })?;
    } else {
        // No top-level expressions, return 0.0
        let zero = f64_type.const_float(0.0);
        builder.build_return(Some(&zero)).map_err(|e| {
            io::Error::new(ErrorKind::Other, format!("Failed to build return: {}", e))
        })?;
    }

    println!("{}", module.print_to_string().to_string());

    // Execute the main function via JIT
    unsafe {
        let main_fn = execution_engine.get_function::<unsafe extern "C" fn() -> f64>("main")
            .map_err(|e| io::Error::new(ErrorKind::Other, format!("Failed to get main: {}", e)))?;
        let result = main_fn.call();
        println!("\nResult: {}", result);
    }

    Ok(())
}
