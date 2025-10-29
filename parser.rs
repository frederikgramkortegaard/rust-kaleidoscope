use crate::ast::{Expr, Function};
use crate::lexer::{LexerContext, Token};

fn get_precedence(tok: &Token) -> i8 {
    match tok {
        Token::Less | Token::Greater => 5,
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
            tok @ Token::Plus
            | tok @ Token::Minus
            | tok @ Token::Star
            | tok @ Token::Slash
            | tok @ Token::Less
            | tok @ Token::Greater => tok,
            _ => return Ok(lhs), // no more operators, return current LHS
        };

        let tok_prec = get_precedence(&op);

        // If this operator binds less tightly than the current expression, return LHS
        if tok_prec < expr_prec {
            return Ok(lhs);
        }

        lexer.consume_assert_next_token(op.clone())?;

        // Parse the primary RHS (just the next term, not a full expression)
        let mut rhs = Box::new(parse_primary(lexer)?);

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

// Parse primary expressions: identifiers, numbers, parenthesized expressions, function calls
fn parse_primary(lexer: &mut LexerContext) -> Result<Expr, String> {
    let token = lexer.peek_token();

    match token {
        // Parens Expression - parse full expression inside
        Token::LParen => {
            lexer.consume_assert_next_token(Token::LParen)?;
            parse_expression(lexer)
        }

        // Number Literals
        Token::Number(_) => {
            if let Token::Number(v) = lexer.next_token() {
                Ok(Expr::Number(v))
            } else {
                unreachable!("Peeked Number but got something else")
            }
        }

        // Either Expr::Variable or Expr::Call
        Token::Identifier(_) => {
            // Consume the identifier to get its name
            let name = if let Token::Identifier(n) = lexer.next_token() {
                n
            } else {
                unreachable!("Peeked Identifier but got something else")
            };

            let next = lexer.peek_token();

            // Expr::Call
            if let Token::LParen = next {
                lexer.consume_assert_next_token(Token::LParen)?;

                // Parse Arguments if any exists
                let mut args = Vec::new();
                if lexer.peek_token() != Token::RParen {
                    args.push(parse_expression(lexer)?);
                    while lexer.peek_token() == Token::Comma {
                        lexer.consume_assert_next_token(Token::Comma)?;
                        if lexer.peek_token() == Token::RParen {
                            break;
                        } // allow trailing comma
                        args.push(parse_expression(lexer)?);
                    }
                }

                lexer.consume_assert_next_token(Token::RParen)?;
                Ok(Expr::Call {
                    args,
                    identifier: name,
                })

            // Expr::Variable
            } else {
                Ok(Expr::Variable(name))
            }
        }

        // if-then-else
        Token::If => {
            lexer.consume_assert_next_token(Token::If)?;
            let condition = Box::new(parse_expression(lexer)?);
            lexer.consume_assert_next_token(Token::Then)?;
            let then = Box::new(parse_expression(lexer)?);
            lexer.consume_assert_next_token(Token::Else)?;
            let els = Box::new(parse_expression(lexer)?);

            Ok(Expr::If {
                condition,
                then,
                els,
            })
        }

        Token::For => {
            println!("ooo  {:?}", lexer.peek_token());
            lexer.consume_assert_next_token(Token::For)?;
            println!("uuu  {:?}", lexer.peek_token());

            let ident: String = match parse_expression(lexer)? {
                Expr::Variable(s) => s,
                x => Err(format!("Expected Identifier in for-loop but got {:?}", x))?,
            };

            lexer.consume_assert_next_token(Token::Assign)?;
            let start = Box::new(parse_expression(lexer)?);
            lexer.consume_assert_next_token(Token::Comma)?;
            let end = Box::new(parse_expression(lexer)?);

            let mut step: Option<Box<Expr>> = None;
            if lexer.peek_token() == Token::Comma {
                lexer.consume_assert_next_token(Token::Comma)?;
                step = Some(Box::new(parse_expression(lexer)?));
            }

            lexer.consume_assert_next_token(Token::In)?;
            let body = Box::new(parse_expression(lexer)?);
            Ok(Expr::For {
                ident,
                start,
                end,
                step,
                body,
            })
        }

        _ => Err(String::from("Failed to parse primary expression")),
    }
}

// Parse full expressions with binary operators
fn parse_expression(lexer: &mut LexerContext) -> Result<Expr, String> {
    let expr = parse_primary(lexer)?;
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
    lexer.consume_opt_next_token(Token::Def)?;
    let mut v = parse_proto(lexer)?;
    v.body = parse_expression(lexer)?;
    Ok(v)
}

fn parse_extern(lexer: &mut LexerContext) -> Result<Function, String> {
    lexer.consume_opt_next_token(Token::Extern)?;
    parse_proto(lexer)
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

pub fn parse(lexer: &mut LexerContext) -> Result<Vec<Function>, String> {
    let mut fvec: Vec<Function> = Vec::new();

    loop {
        let tok = lexer.peek_token();
        match tok {
            Token::Def => fvec.push(parse_function_definition(lexer)?),
            Token::Extern => fvec.push(parse_extern(lexer)?),
            Token::Eof => break,

            // Top level expression
            _ => fvec.push(parse_top_level_expression(lexer)?),
        }
    }
    Ok(fvec)
}
