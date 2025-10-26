use crate::ast::{Expr, Function};
use crate::lexer::{LexerContext, Token};

fn get_precedence(tok: &Token) -> i8 {
    match tok {
        Token::Plus | Token::Minus => 10,
        Token::Star | Token::Slash => 20,
        _ => -1,
    }
}

// Parse the RHS of a binary expression, given the current LHS and minimum precedence
fn parse_binop_rhs(
    expr_prec: i8,
    mut lhs: Box<Expr>,
    lexer: &mut LexerContext,
) -> Result<Box<Expr>, String> {
    loop {
        // Peek the next token to see if it's a binary operator
        let op = match lexer.peek_token() {
            tok @ Token::Plus | tok @ Token::Minus | tok @ Token::Star | tok @ Token::Slash => tok,
            _ => return Ok(lhs), // no more operators, return current LHS
        };

        let tok_prec = get_precedence(&op);

        // If this operator binds less tightly than the current expression, return LHS
        if tok_prec < expr_prec {
            
            return Ok(lhs);
        }

        lexer.consume_assert_next_token(op.clone())?;

        // Parse the RHS
        let mut rhs = Box::new(parse_expression(lexer)?);

        // Check the next operator's precedence for right-associativity
        let next_prec = get_precedence(&lexer.peek_token());

        if tok_prec < next_prec {
            rhs = parse_binop_rhs(tok_prec + 1, rhs, lexer)?;
        }

        // Merge LHS and RHS
        lhs = Box::new(Expr::BinOp {
            left: lhs,
            op,
            right: rhs,
        });

    }

}

fn parse_expression(lexer: &mut LexerContext) -> Result<Expr, String> {
    let token = lexer.next_token();

    // Handle some of the basic experssion types that don't need their own helper methods for
    // parsing
    let expr = match token {
        // Parens Expression, eat it.
        Token::LParen => parse_expression(lexer),

        // Number Literals
        Token::Number(v) => Ok(Expr::Number(v)),

        // Either Expr::Variable or Expr::Call
        Token::Identifier(name) => {
            let next = lexer.peek_token();

            if let Token::LParen = next {
                lexer.consume_assert_next_token(Token::LParen)?;
                let e = parse_expression(lexer);
                lexer.consume_assert_next_token(Token::RParen)?;
                e
            } else {
                Ok(Expr::Variable(name))
            }
        }

        _ => Err(String::from("Failed to parse expression")),
    }?;

    parse_binop_rhs(0, Box::new(expr), lexer).map(|b| *b)
}

fn parse_top_level_expression(lexer: &mut LexerContext) -> Result<Function, String> {
    // @NOTE : This is a horrible way to handle top-level expressions, but since this is following
    // Kaleidescope https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/LangImpl02.html at least
    // semi-truthfully, that's how we're going to do it as well.
    let f = Function {
        name: String::from("_top_level_expr"),
        args: Vec::new(),
        body: parse_expression(lexer)?,
    };
    println!("Parsed top level expr {:?}", f);
    Ok(f)
}

fn parse_function_definition(lexer: &mut LexerContext) -> Result<Function, String> {
    let mut v = parse_proto(lexer)?;
    v.body = parse_expression(lexer)?;
    Ok(v)
}
fn parse_proto(lexer: &mut LexerContext) -> Result<Function, String> {
    let name = lexer.consume_assert_next_token(Token::Identifier(String::new()))?;
    let name_string = match name {
        Token::Identifier(s) => s,
        _ => return Err("Expected Identifier for function name".to_string()),
    };

    let _ = lexer.consume_assert_next_token(Token::LParen)?; // Skip Starting parens

    let mut args: Vec<String> = Vec::new();

    loop {
        let tok = lexer.next_token();

        match tok {
            Token::Identifier(s) => args.push(s),
            Token::RParen => break,
            _ => {
                return Err(format!(
                    "Unexpected token found while parsing args {:?}",
                    tok
                ))
            }
        }
    }

    let f = Function {
        name: name_string,
        args,
        body: Expr::None,
    };
    println!("Parsed function proto {:?}", f);
    Ok(f)
}

pub fn parse(lexer: &mut LexerContext) -> Result<(Vec<Function>, Expr), String> {
    let mut fvec: Vec<Function> = Vec::new();

    loop {
        let tok = lexer.next_token();
        match tok {
            Token::Def => {
                fvec.push(
                    parse_function_definition(lexer).map_err(|e| {
                        format!("Failed to parse function at token {:?}: {}", tok, e)
                    })?,
                );
            }
            Token::Extern => fvec.push(parse_proto(lexer)?),
            Token::Eof => break,

            _ => panic!("Unhandled Token in parser: {:?}", tok),
        }
    }
    Ok((fvec, Expr::None))
}
