// Standard library
use std::collections::HashMap;

// Our crate
use crate::ast::{Expr, Function};
use crate::lexer::Token;

// Inkwell
use inkwell::{
    builder::Builder, context::Context, module::Module, values::BasicMetadataValueEnum,
    values::BasicValueEnum, values::FloatValue, values::FunctionValue,
};

pub type CGResult<'ctx> = Result<Option<BasicValueEnum<'ctx>>, String>;

impl Function {
    pub fn codegen<'lctx>(
        &self,
        context: &'lctx Context,
        builder: &mut Builder<'lctx>,
        module: &mut Module<'lctx>,
        vars: &mut HashMap<String, BasicValueEnum<'lctx>>,
    ) -> Result<(), String> {
        // Check if function already exists (skip redefinition)
        if module.get_function(self.name.as_str()).is_some() {
            return Ok(());
        }

        // Create function signature
        let f64 = context.f64_type();
        let param_types = vec![f64.into(); self.args.len()];

        let fn_ty = f64.fn_type(&param_types, false);
        let func = module.add_function(self.name.as_str(), fn_ty, None);

        // Set up parameters in the symbol table
        vars.clear();
        for (p, name) in func.get_param_iter().zip(self.args.iter()) {
            p.set_name(name);
            vars.insert(name.clone(), p);
        }

        // Externs have no body - just the function declaration, so we're done
        if matches!(self.body, Expr::None) {
            return Ok(());
        }

        // Generate function body
        let entry = context.append_basic_block(func, "entry");
        builder.position_at_end(entry);

        if let Some(ret_val) = self.body.codegen(context, builder, module, vars)? {
            builder
                .build_return(Some(&ret_val))
                .map_err(|e| format!("Failed to build return: {}", e))?;
        } else {
            builder
                .build_return(None)
                .map_err(|e| format!("Failed to build empty return: {}", e))?;
        }
        Ok(())
    }
}

impl Expr {
    pub fn codegen<'lctx>(
        &self,
        context: &'lctx Context,
        builder: &mut Builder<'lctx>,
        module: &Module<'lctx>,
        vars: &mut HashMap<String, BasicValueEnum<'lctx>>,
    ) -> CGResult<'lctx> {
        match self {
            Expr::For {
                ident,
                start,
                end,
                step,
                body,
            } => {
                // Emit start value
                let start_val = start.codegen(context, builder, module, vars)?.unwrap();

                // Get current function and preheader block
                let f = builder.get_insert_block().unwrap().get_parent().unwrap();
                let preheader_bb = builder.get_insert_block().unwrap();

                // Create loop block and branch to it
                let loop_bb = context.append_basic_block(f, "loop");
                builder
                    .build_unconditional_branch(loop_bb)
                    .map_err(|e| format!("Failed to branch to loop: {}", e))?;

                // Position in loop block, then create PHI
                builder.position_at_end(loop_bb);
                let phi = builder.build_phi(context.f64_type(), ident).unwrap();
                phi.add_incoming(&[(&start_val, preheader_bb)]);

                // Shadow the variable
                let old_val = vars.get(ident).cloned();
                vars.insert(ident.clone(), phi.as_basic_value());

                // Generate body
                body.codegen(context, builder, module, vars)?;

                // Compute step value
                let step_val = match step {
                    Some(s) => s.codegen(context, builder, module, vars)?.unwrap(),
                    None => context.f64_type().const_float(1.0).into(),
                };

                // Compute next iteration value
                let next_var = builder
                    .build_float_add(
                        phi.as_basic_value().into_float_value(),
                        step_val.into_float_value(),
                        "nextvar",
                    )
                    .map_err(|e| format!("Failed to build nextvar: {}", e))?;

                // Compute end condition
                let end_cond_val = end.codegen(context, builder, module, vars)?.unwrap();
                let end_cond = builder
                    .build_float_compare(
                        inkwell::FloatPredicate::ONE,
                        end_cond_val.into_float_value(),
                        context.f64_type().const_float(0.0),
                        "loopcond",
                    )
                    .map_err(|e| format!("Failed to build endcond: {}", e))?;

                // Get the block after body
                let loop_end_bb = builder.get_insert_block().unwrap();

                // Create after-loop block
                let after_bb = context.append_basic_block(f, "afterloop");

                // Conditional branch
                builder
                    .build_conditional_branch(end_cond, loop_bb, after_bb)
                    .map_err(|e| format!("Failed to build cond branch: {}", e))?;

                // Add backedge to PHI
                phi.add_incoming(&[(&next_var, loop_end_bb)]);

                // Position in after block
                builder.position_at_end(after_bb);

                // Restore old variable
                if let Some(old) = old_val {
                    vars.insert(ident.clone(), old);
                } else {
                    vars.remove(ident);
                }

                // For loops always return 0.0
                Ok(Some(context.f64_type().const_float(0.0).into()))
            }

            Expr::If {
                condition,
                then,
                els,
            } => {
                // Setup Conditional Phi
                let v = condition
                    .codegen(context, builder, module, vars)?
                    .unwrap()
                    .into_float_value();
                let condv = builder
                    .build_float_compare(
                        inkwell::FloatPredicate::ONE,
                        v,
                        context.f64_type().const_float(0.0),
                        "ifcond",
                    )
                    .map_err(|e| format!("Failed to build condv: {}", e))?;

                let f = builder.get_insert_block().unwrap().get_parent().unwrap();
                let thenbb = context.append_basic_block(f, "then");
                let elsebb = context.append_basic_block(f, "else");
                let mergebb = context.append_basic_block(f, "ifcont");

                builder
                    .build_conditional_branch(condv, thenbb, elsebb)
                    .map_err(|e| format!("Failed to build conditional branch: {}", e))?;

                // Then Block
                builder.position_at_end(thenbb);
                let then_val = then.codegen(context, builder, module, vars)?.unwrap();
                builder.build_unconditional_branch(mergebb).unwrap();

                // Else Block
                builder.position_at_end(elsebb);
                let els_val = els.codegen(context, builder, module, vars)?.unwrap();
                builder.build_unconditional_branch(mergebb).unwrap();

                // Merge Bloock
                builder.position_at_end(mergebb);
                let phi = builder.build_phi(context.f64_type(), "iftmp").unwrap();
                phi.add_incoming(&[(&then_val, thenbb), (&els_val, elsebb)]);

                Ok(Some(phi.as_basic_value()))
            }

            Expr::Call { identifier, args } => {
                let callee: FunctionValue = module.get_function(identifier.as_str()).unwrap();
                let mut cargs: Vec<BasicMetadataValueEnum> = Vec::new();
                for arg in args {
                    let val = arg
                        .codegen(context, builder, module, vars)?
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
                let call = builder.build_call(callee, &cargs, "calltmp").unwrap();
                let ret: FloatValue = call.try_as_basic_value().left().unwrap().into_float_value();
                Ok(Some(ret.into()))
            }
            Expr::Number(value) => Ok(Some(context.f64_type().const_float(*value).into())),
            Expr::Variable(name) => {
                let val = vars
                    .get(name)
                    .ok_or_else(|| format!("Unknown variable: {}", name))?;
                Ok(Some(*val))
            }
            Expr::BinOp { left, op, right } => {
                let lhs = left
                    .codegen(context, builder, module, vars)?
                    .ok_or_else(|| "Left operand produced no value".to_string())?
                    .into_float_value();
                let rhs = right
                    .codegen(context, builder, module, vars)?
                    .ok_or_else(|| "Right operand produced no value".to_string())?
                    .into_float_value();
                let result = match op {
                    Token::Plus => builder
                        .build_float_add(lhs, rhs, "addtmp")
                        .map_err(|e| format!("Failed to build add: {}", e))?
                        .into(),
                    Token::Minus => builder
                        .build_float_sub(lhs, rhs, "subtmp")
                        .map_err(|e| format!("Failed to build sub: {}", e))?
                        .into(),
                    Token::Star => builder
                        .build_float_mul(lhs, rhs, "multmp")
                        .map_err(|e| format!("Failed to build mul: {}", e))?
                        .into(),
                    Token::Slash => builder
                        .build_float_div(lhs, rhs, "divtmp")
                        .map_err(|e| format!("Failed to build div: {}", e))?
                        .into(),
                    Token::Less => {
                        let cmp = builder
                            .build_float_compare(inkwell::FloatPredicate::ULT, lhs, rhs, "cmptmp")
                            .map_err(|e| format!("Failed to build less than: {}", e))?;
                        builder
                            .build_unsigned_int_to_float(cmp, context.f64_type(), "booltmp")
                            .map_err(|e| format!("Failed to convert bool to float: {}", e))?
                            .into()
                    }
                    Token::Greater => {
                        let cmp = builder
                            .build_float_compare(inkwell::FloatPredicate::UGT, lhs, rhs, "cmptmp")
                            .map_err(|e| format!("Failed to build greater than: {}", e))?;
                        builder
                            .build_unsigned_int_to_float(cmp, context.f64_type(), "booltmp")
                            .map_err(|e| format!("Failed to convert bool to float: {}", e))?
                            .into()
                    }
                    _ => return Err(format!("Unhandled operator: {:?}", op)),
                };
                Ok(Some(result))
            }

            _ => Err(format!("Unhandled expression: {:?}", self)),
        }
    }
}
