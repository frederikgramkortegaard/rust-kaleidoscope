// Standard library
use std::collections::HashMap;

// Our crate
use crate::ast::{Expr, Function};
use crate::externs::FfiRegistry;
use crate::lexer::Token;
use crate::parser::ParserContext;

// Inkwell
use inkwell::{
    builder::Builder, context::Context, module::Module, values::BasicMetadataValueEnum,
    values::BasicValueEnum, values::FloatValue, values::FunctionValue, values::PointerValue,
};

pub type CGResult<'ctx> = Result<Option<BasicValueEnum<'ctx>>, String>;

pub struct CodegenContext<'ctx> {
    pub context: &'ctx Context,
    pub builder: Builder<'ctx>,
    pub module: Module<'ctx>,
    pub vars: HashMap<String, PointerValue<'ctx>>,
    main_entry: inkwell::basic_block::BasicBlock<'ctx>,
    last_result: Option<BasicValueEnum<'ctx>>,
}

impl<'ctx> CodegenContext<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let builder = context.create_builder();
        let module = context.create_module(module_name);

        // Create main() function upfront to hold all top-level expressions
        let f64_type = context.f64_type();
        let main_fn_type = f64_type.fn_type(&[], false);
        let main_func = module.add_function("main", main_fn_type, None);
        let main_entry = context.append_basic_block(main_func, "entry");
        builder.position_at_end(main_entry);

        CodegenContext {
            context,
            builder,
            module,
            vars: HashMap::new(),
            main_entry,
            last_result: None,
        }
    }

    pub fn create_entryblock_alloc(
        &mut self,
        f: &FunctionValue,
        name: String,
    ) -> Result<PointerValue<'ctx>, String> {
        let entry = f.get_last_basic_block().unwrap();

        let entry_builder = self.context.create_builder();

        if let Some(first) = entry.get_first_instruction() {
            entry_builder.position_before(&first);
        } else {
            entry_builder.position_at_end(entry);
        }

        match entry_builder.build_alloca(self.context.f64_type(), name.as_str()) {
            Ok(r) => Ok(r),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn codegen_top_level_expr(&mut self, expr: &Expr) -> Result<(), String> {
        if let Some(result) = expr.codegen(self)? {
            self.last_result = Some(result);
            // For expressions with control flow, we need to ensure we're back in a valid position
            // The builder might be in a different block (like afterloop), so we stay there
        }
        Ok(())
    }

    pub fn codegen_function(&mut self, func: &Function) -> Result<(), String> {
        // Codegen regular function (this repositions the builder)
        func.codegen(self)?;

        // Reposition builder back to main's entry for next top-level expr
        self.builder.position_at_end(self.main_entry);
        Ok(())
    }

    pub fn codegen(
        &mut self,
        parser: &ParserContext,
        ffi_registry: &FfiRegistry,
        execution_engine: &inkwell::execution_engine::ExecutionEngine,
    ) -> Result<(), String> {
        // First, find the last top-level expression to use as main's return value
        let last_top_level = parser
            .functions
            .iter()
            .rposition(|f| f.name == "_top_level_expr");

        for (i, f) in parser.functions.iter().enumerate() {
            if f.name == "_top_level_expr" {
                // Only codegen the last top-level expression into main
                if Some(i) == last_top_level {
                    self.codegen_top_level_expr(&f.body)?;
                }
            } else {
                // Codegen regular function
                self.codegen_function(f)?;

                // If this is an extern, register it with JIT if available in FFI registry
                if matches!(f.body, Expr::None) {
                    if let Some(func_ptr) = ffi_registry.get(&f.name) {
                        let llvm_func = self.module.get_function(&f.name).unwrap();
                        execution_engine.add_global_mapping(&llvm_func, func_ptr);
                    }
                }
            }
        }

        // Finalize main function with return statement
        self.finalize()?;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), String> {
        // Add return statement to main with the last result
        // The builder is positioned wherever the last expression left it
        if let Some(ret_val) = self.last_result {
            self.builder
                .build_return(Some(&ret_val))
                .map_err(|e| format!("Failed to build return: {}", e))?;
        } else {
            // No top-level expressions, return 0.0 from entry
            self.builder.position_at_end(self.main_entry);
            let zero = self.context.f64_type().const_float(0.0);
            self.builder
                .build_return(Some(&zero))
                .map_err(|e| format!("Failed to build return: {}", e))?;
        }
        Ok(())
    }
}

impl Function {
    pub fn codegen(&self, cg: &mut CodegenContext) -> Result<(), String> {
        // Check if function already exists (skip redefinition)
        if cg.module.get_function(self.name.as_str()).is_some() {
            return Ok(());
        }

        // Create function signature
        let f64 = cg.context.f64_type();
        let param_types = vec![f64.into(); self.args.len()];

        let fn_ty = f64.fn_type(&param_types, false);
        let func = cg.module.add_function(self.name.as_str(), fn_ty, None);

        // Externs have no body - just the function declaration, so we're done
        if matches!(self.body, Expr::None) {
            return Ok(());
        }

        // Generate function body
        let entry = cg.context.append_basic_block(func, "entry");
        cg.builder.position_at_end(entry);

        // Set up parameters in the symbol table
        cg.vars.clear();
        for (p, name) in func.get_param_iter().zip(self.args.iter()) {
            p.set_name(name);
            let d = cg.create_entryblock_alloc(&func, name.clone())?;
            cg.builder.build_store(d, p).map_err(|e| e.to_string())?;
            cg.vars.insert(name.clone(), d);
        }
        if let Some(ret_val) = self.body.codegen(cg)? {
            cg.builder
                .build_return(Some(&ret_val))
                .map_err(|e| format!("Failed to build return: {}", e))?;
        } else {
            cg.builder
                .build_return(None)
                .map_err(|e| format!("Failed to build empty return: {}", e))?;
        }
        Ok(())
    }
}

impl Expr {
    pub fn codegen<'ctx>(&self, cg: &mut CodegenContext<'ctx>) -> CGResult<'ctx> {
        match self {
            Expr::For {
                ident,
                start,
                end,
                step,
                body,
            } => {
                let f = cg.builder.get_insert_block().unwrap().get_parent().unwrap();
                let alloc = cg.create_entryblock_alloc(&f, ident.clone())?;

                // Emit start value
                let start_val = start.codegen(cg)?.unwrap();
                cg.builder
                    .build_store(alloc, start_val)
                    .map_err(|e| e.to_string())?;

                // Get current function
                let f = cg.builder.get_insert_block().unwrap().get_parent().unwrap();

                // Create loop block and branch to it
                let loop_bb = cg.context.append_basic_block(f, "loop");
                cg.builder
                    .build_unconditional_branch(loop_bb)
                    .map_err(|e| format!("Failed to branch to loop: {}", e))?;

                // Position in loop block
                cg.builder.position_at_end(loop_bb);

                // Shadow the variable
                let old_val = cg.vars.get(ident).cloned();
                cg.vars.insert(ident.clone(), alloc);

                // Generate body
                body.codegen(cg)?;

                // Compute step value
                let step_val = match step {
                    Some(s) => s.codegen(cg)?.unwrap(),
                    None => cg.context.f64_type().const_float(1.0).into(),
                }
                .into_float_value();

                // Compute end condition
                let end_cond_val = end.codegen(cg)?.unwrap();

                let cur_var = cg
                    .builder
                    .build_load(cg.context.f64_type(), alloc, ident)
                    .map_err(|e| format!("Failed to build load: {}", e))?
                    .into_float_value();
                let next_var = cg
                    .builder
                    .build_float_add(cur_var, step_val, "nextvar")
                    .map_err(|e| format!("Failed to build add: {}", e))?;
                cg.builder
                    .build_store(alloc, next_var)
                    .map_err(|e| format!("Failed to build store: {}", e))?;

                let end_cond = cg
                    .builder
                    .build_float_compare(
                        inkwell::FloatPredicate::ONE,
                        end_cond_val.into_float_value(),
                        cg.context.f64_type().const_float(0.0),
                        "loopcond",
                    )
                    .map_err(|e| format!("Failed to build endcond: {}", e))?;

                // Create after-loop block
                let after_bb = cg.context.append_basic_block(f, "afterloop");

                // Conditional branch
                cg.builder
                    .build_conditional_branch(end_cond, loop_bb, after_bb)
                    .map_err(|e| format!("Failed to build cond branch: {}", e))?;

                // Position in after block
                cg.builder.position_at_end(after_bb);

                // Restore old variable
                if let Some(old) = old_val {
                    cg.vars.insert(ident.clone(), old);
                } else {
                    cg.vars.remove(ident);
                }

                // For loops always return 0.0
                Ok(Some(cg.context.f64_type().const_float(0.0).into()))
            }

            Expr::Var { varnames, body } => {
                let f = cg.builder.get_insert_block().unwrap().get_parent().unwrap();

                let mut old_bindings: Vec<(String, PointerValue)> = Vec::new();
                for (name, expr) in varnames {
                    let init_val = match expr {
                        Some(e) => e.codegen(cg)?.unwrap(),
                        None => cg.context.f64_type().const_float(0.0).into(),
                    };

                    let alloc = cg.create_entryblock_alloc(&f, name.clone())?;
                    cg.builder
                        .build_store(alloc, init_val)
                        .map_err(|e| e.to_string())?;

                    if let Some(val) = cg.vars.get(name).cloned() {
                        old_bindings.push((name.clone(), val));
                    }
                    cg.vars.insert(name.clone(), alloc);
                }

                let bval = body.codegen(cg)?.unwrap();
                for  (name, val) in old_bindings {
                    cg.vars.insert(name, val);
                }

                Ok(Some(bval))
            }
            Expr::Unary { op, left } => {
                let operand = left
                    .codegen(cg)?
                    .ok_or_else(|| "Operand produced no value".to_string())?
                    .into_float_value();

                let func_name = format!("unary{}", op);
                let func = cg
                    .module
                    .get_function(&func_name)
                    .ok_or_else(|| format!("Unknown unary operator: {}", func_name))?;

                let args = [operand.into()];
                let result = cg
                    .builder
                    .build_call(func, &args, "unop")
                    .map_err(|e| format!("Failed to build call: {}", e))?
                    .try_as_basic_value()
                    .left()
                    .ok_or("Function call didn't return a value")?;

                Ok(Some(result))
            }
            Expr::If {
                condition,
                then,
                els,
            } => {
                // Setup Conditional Phi
                let v = condition.codegen(cg)?.unwrap().into_float_value();
                let condv = cg
                    .builder
                    .build_float_compare(
                        inkwell::FloatPredicate::ONE,
                        v,
                        cg.context.f64_type().const_float(0.0),
                        "ifcond",
                    )
                    .map_err(|e| format!("Failed to build condv: {}", e))?;

                let f = cg.builder.get_insert_block().unwrap().get_parent().unwrap();
                let thenbb = cg.context.append_basic_block(f, "then");
                let elsebb = cg.context.append_basic_block(f, "else");
                let mergebb = cg.context.append_basic_block(f, "ifcont");

                cg.builder
                    .build_conditional_branch(condv, thenbb, elsebb)
                    .map_err(|e| format!("Failed to build conditional branch: {}", e))?;

                // Then Block
                cg.builder.position_at_end(thenbb);
                let then_val = then.codegen(cg)?.unwrap();
                cg.builder.build_unconditional_branch(mergebb).unwrap();
                let then_end_bb = cg.builder.get_insert_block().unwrap();

                // Else Block
                cg.builder.position_at_end(elsebb);
                let els_val = els.codegen(cg)?.unwrap();
                cg.builder.build_unconditional_branch(mergebb).unwrap();
                let else_end_bb = cg.builder.get_insert_block().unwrap();

                // Merge Bloock
                cg.builder.position_at_end(mergebb);
                let phi = cg
                    .builder
                    .build_phi(cg.context.f64_type(), "iftmp")
                    .unwrap();
                phi.add_incoming(&[(&then_val, then_end_bb), (&els_val, else_end_bb)]);

                Ok(Some(phi.as_basic_value()))
            }

            Expr::Call { identifier, args } => {
                let callee: FunctionValue = cg
                    .module
                    .get_function(identifier.as_str())
                    .ok_or_else(|| format!("Unknown function: {}", identifier))?;
                let mut cargs: Vec<BasicMetadataValueEnum> = Vec::new();
                for arg in args {
                    let val = arg
                        .codegen(cg)?
                        .ok_or_else(|| {
                            format!(
                                "Can not codegen argument in call to function: {:?}, {:?}",
                                arg,
                                identifier.as_str()
                            )
                        })?
                        .into_float_value();
                    cargs.push(val.into());
                }
                let call = cg.builder.build_call(callee, &cargs, "calltmp").unwrap();
                let ret: FloatValue = call.try_as_basic_value().left().unwrap().into_float_value();
                Ok(Some(ret.into()))
            }
            Expr::Number(value) => Ok(Some(cg.context.f64_type().const_float(*value).into())),
            Expr::Variable(name) => {
                let val = cg
                    .vars
                    .get(name)
                    .ok_or_else(|| format!("Unknown variable: {}", name))?;

                match cg
                    .builder
                    .build_load(cg.context.f64_type(), *val, name.as_str())
                {
                    Ok(b) => Ok(Some(b)),
                    Err(e) => Err(e.to_string()),
                }
            }
            Expr::BinOp { left, op, right } => {
                // For assignments we don't want to codegen the LHS so it's a special case

                match (op, left.as_ref()) {
                    // If it's an assignment, we don't want to generate the LHS, we just want to
                    // generate the variable
                    (Token::Assign(_), Expr::Variable(s)) => {
                        let val = right
                            .codegen(cg)?
                            .ok_or_else(|| "Right operand produced no value".to_string())?
                            .into_float_value();
                        let var = cg.vars.get(s).cloned().unwrap();

                        cg.builder
                            .build_store(var, val)
                            .map_err(|e| e.to_string())?;
                        return Ok(Some(val.into()));
                    }
                    _ => {}
                };

                let lhs = left
                    .codegen(cg)?
                    .ok_or_else(|| "Left operand produced no value".to_string())?
                    .into_float_value();

                let rhs = right
                    .codegen(cg)?
                    .ok_or_else(|| "Right operand produced no value".to_string())?
                    .into_float_value();

                let result = match op {
                    Token::Plus(c)
                    | Token::Minus(c)
                    | Token::Star(c)
                    | Token::Slash(c)
                    | Token::Less(c)
                    | Token::Greater(c)
                    | Token::Bang(c)
                    | Token::Pipe(c)
                    | Token::Ampersand(c)
                    | Token::Caret(c)
                    | Token::Percent(c)
                    | Token::Dollar(c)
                    | Token::At(c)
                    | Token::Tilde(c) => {
                        if let Some(func) = cg.module.get_function(&format!("binary{}", c)) {
                            // User-defined binary operator - call the function
                            let args = [lhs.into(), rhs.into()];
                            cg.builder
                                .build_call(func, &args, "binop")
                                .map_err(|e| format!("Failed to build call: {}", e))?
                                .try_as_basic_value()
                                .left()
                                .ok_or("Function call didn't return a value")?
                        } else {
                            // Not user-defined, check if it's a built-in operator
                            match op {
                                Token::Plus(_) => cg
                                    .builder
                                    .build_float_add(lhs, rhs, "addtmp")
                                    .map_err(|e| format!("Failed to build add: {}", e))?
                                    .into(),
                                Token::Minus(_) => cg
                                    .builder
                                    .build_float_sub(lhs, rhs, "subtmp")
                                    .map_err(|e| format!("Failed to build sub: {}", e))?
                                    .into(),
                                Token::Star(_) => cg
                                    .builder
                                    .build_float_mul(lhs, rhs, "multmp")
                                    .map_err(|e| format!("Failed to build mul: {}", e))?
                                    .into(),
                                Token::Slash(_) => cg
                                    .builder
                                    .build_float_div(lhs, rhs, "divtmp")
                                    .map_err(|e| format!("Failed to build div: {}", e))?
                                    .into(),
                                Token::Less(_) => {
                                    let cmp = cg
                                        .builder
                                        .build_float_compare(
                                            inkwell::FloatPredicate::ULT,
                                            lhs,
                                            rhs,
                                            "cmptmp",
                                        )
                                        .map_err(|e| format!("Failed to build less than: {}", e))?;
                                    cg.builder
                                        .build_unsigned_int_to_float(
                                            cmp,
                                            cg.context.f64_type(),
                                            "booltmp",
                                        )
                                        .map_err(|e| {
                                            format!("Failed to convert bool to float: {}", e)
                                        })?
                                        .into()
                                }
                                Token::Greater(_) => {
                                    let cmp = cg
                                        .builder
                                        .build_float_compare(
                                            inkwell::FloatPredicate::UGT,
                                            lhs,
                                            rhs,
                                            "cmptmp",
                                        )
                                        .map_err(|e| {
                                            format!("Failed to build greater than: {}", e)
                                        })?;
                                    cg.builder
                                        .build_unsigned_int_to_float(
                                            cmp,
                                            cg.context.f64_type(),
                                            "booltmp",
                                        )
                                        .map_err(|e| {
                                            format!("Failed to convert bool to float: {}", e)
                                        })?
                                        .into()
                                }
                                _ => {
                                    return Err(format!("Unknown binary operator: {:?}", op));
                                }
                            }
                        }
                    }
                    _ => return Err(format!("Unknown token type: {:?}", op)),
                };
                Ok(Some(result))
            }

            _ => Err(format!("Unhandled expression: {:?}", self)),
        }
    }
}
