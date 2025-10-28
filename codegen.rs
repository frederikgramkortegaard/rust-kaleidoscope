use crate::ast::{Expr, Function};
use crate::lexer::Token;
use inkwell::{builder::Builder, context::Context, module::Module, values::BasicValueEnum};
use std::collections::HashMap;

pub type CGResult<'ctx> = Result<Option<BasicValueEnum<'ctx>>, String>;

impl Function {
    pub fn codegen<'lctx>(
        &self,
        context: &'lctx Context,
        builder: &mut Builder<'lctx>,
        module: &mut Module<'lctx>,
        vars: &mut HashMap<String, BasicValueEnum<'lctx>>,
    ) -> Result<(), String> {
        // Check if we already have a function defined with this name
        if module.get_function(self.name.as_str()).is_some() {
            return Ok(());
        }

        let f64 = context.f64_type();
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum> =
            vec![f64.into(); self.args.len()];

        let fn_ty = f64.fn_type(&param_types, false);
        let func = module.add_function(self.name.as_str(), fn_ty, None);
        vars.clear();
        for (p, name) in func.get_param_iter().zip(self.args.iter()) {
            p.set_name(name);
            vars.insert(name.clone(), p);
        }

        // Externs, there is no body to generate code for
        if matches!(self.body, Expr::None) {
            return Ok(());
        }
        // create a block inside *this* function
        let entry = context.append_basic_block(func, "entry");
        builder.position_at_end(entry);

        if let Some(l) = self.body.codegen(context, builder, vars)? {
            builder.build_return(Some(&l)).unwrap();
        } else {
            builder.build_return(None).unwrap();
        }
        Ok(())
    }
}

impl Expr {
    pub fn codegen<'lctx>(
        &self,
        ctx: &'lctx Context,
        builder: &mut Builder<'lctx>,
        vars: &mut HashMap<String, BasicValueEnum<'lctx>>,
    ) -> CGResult<'lctx> {
        match self {
            Expr::Number(value) => Ok(Some(ctx.f64_type().const_float(*value).into())),
            Expr::Variable(name) => {
                let v = vars.get(name).cloned().unwrap();
                Ok(Some(v))
            }
            Expr::BinOp { left, op, right } => {
                let lhs = left
                    .codegen(ctx, builder, vars)?
                    .unwrap()
                    .into_float_value();
                let rhs = right
                    .codegen(ctx, builder, vars)?
                    .unwrap()
                    .into_float_value();
                let sum = match op {
                    Token::Plus => builder.build_float_add(lhs, rhs, "addtmp").unwrap().into(),
                    Token::Minus => builder.build_float_sub(lhs, rhs, "subtmp").unwrap().into(),
                    Token::Star => builder.build_float_mul(lhs, rhs, "multmp").unwrap().into(),
                    Token::Slash => builder.build_float_div(lhs, rhs, "divtmp").unwrap().into(),
                    _ => panic!("Unhandled operator {:?}", op),
                };
                Ok(Some(sum))
            }

            _ => panic!("Unhandled; {:?}", self),
        }
    }
}
