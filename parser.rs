use crate::ast::{Expr, Function};
use crate::lexer::{LexerContext, Token};
use std::collections::HashMap;

pub struct ParserContext {
    pub functions: Vec<Function>,
    pub binop_precedence: HashMap<char, i8>,
}

impl ParserContext {
    pub fn new() -> Self {
        let mut binop_precedence = HashMap::new();

        // Initialize built-in binary operators with their precedence
        binop_precedence.insert('=', 2);
        binop_precedence.insert('<', 10);
        binop_precedence.insert('>', 10);
        binop_precedence.insert('+', 20);
        binop_precedence.insert('-', 20);
        binop_precedence.insert('*', 40);
        binop_precedence.insert('/', 40);

        ParserContext {
            functions: Vec::new(),
            binop_precedence,
        }
    }

    pub fn parse(&mut self, lexer: &mut LexerContext) -> Result<(), String> {
        loop {
            let tok = lexer.peek_token();
            match tok {
                Token::Def => {
                    let f = self.parse_function_definition(lexer)?;
                    self.functions.push(f);
                }
                Token::Extern => {
                    let f = self.parse_extern(lexer)?;
                    self.functions.push(f);
                }
                Token::Eof => break,

                // Top level expression
                _ => {
                    let f = self.parse_top_level_expression(lexer)?;
                    self.functions.push(f);
                }
            }
        }
        Ok(())
    }

    fn get_precedence(&self, tok: &Token) -> i8 {
        // Extract the character from the token
        let op_char = match tok {
            Token::Less(c)
            | Token::Greater(c)
            | Token::Plus(c)
            | Token::Minus(c)
            | Token::Star(c)
            | Token::Slash(c)
            | Token::Assign(c)
            | Token::Bang(c)
            | Token::Pipe(c)
            | Token::Ampersand(c)
            | Token::Caret(c)
            | Token::Percent(c)
            | Token::Dollar(c)
            | Token::At(c)
            | Token::Tilde(c) => *c,
            _ => return -1,
        };

        // Look up the precedence in the table (handles both built-in and user-defined)
        self.binop_precedence.get(&op_char).copied().unwrap_or(-1)
    }

    // Parse the RHS of a binary expression, given the current LHS and minimum precedence
    fn parse_binop_rhs(
        &self,
        expr_prec: i8,
        mut lhs: Box<Expr>,
        lexer: &mut LexerContext,
    ) -> Result<Box<Expr>, String> {
        loop {
            // Peek the next token to see if it's a binary operator
            let peeked = lexer.peek_token();
            let tok_prec = self.get_precedence(&peeked);

            // If this operator binds less tightly than the current expression, return LHS
            if tok_prec < expr_prec {
                return Ok(lhs);
            }

            let op = lexer.next_token();

            // Parse the primary expression after the binary operator
            let mut rhs = Box::new(self.parse_unary(lexer)?);

            // Check the next operator's precedence for right-associativity
            let next_prec = self.get_precedence(&lexer.peek_token());

            if tok_prec < next_prec {
                rhs = self.parse_binop_rhs(tok_prec + 1, rhs, lexer)?;
            }

            // Merge LHS and RHS
            lhs = Box::new(Expr::BinOp {
                left: lhs,
                op,
                right: rhs,
            });
        }
    }

    // Parse primary expressions - identifiers, numbers, parens exprs, function calls
    fn parse_primary(&self, lexer: &mut LexerContext) -> Result<Expr, String> {
        let token = lexer.peek_token();

        match token {
            // Parens Expression - parse full expression inside
            Token::LParen(_) => {
                lexer.consume_assert_next_token(Token::LParen('('))?;
                let expr = self.parse_expression(lexer)?;
                lexer.consume_assert_next_token(Token::RParen(')'))?;
                Ok(expr)
            }

            // Local Var Decls
            Token::Var => {
                lexer.consume_assert_next_token(Token::Var)?;
                let mut pairs: Vec<(String, Option<Expr>)> = Vec::new();

                // Getr the list of identifiers we're declaring (and potentially initializing)
                while matches!(lexer.peek_token(), Token::Identifier(_)) {
                    let ident = match lexer.next_token() {
                        Token::Identifier(s) => s,
                        _ => unreachable!(),
                    };

                    let init = if matches!(lexer.peek_token(), Token::Assign(_)) {
                        lexer.next_token();
                        Some(self.parse_expression(lexer)?)
                    } else {
                        None
                    };

                    pairs.push((ident, init));

                    if !matches!(lexer.peek_token(), Token::Comma(',')) {
                        break;
                    }
                    lexer.next_token();
                }

                // Now we're ready to parse the body
                lexer.consume_assert_next_token(Token::In)?;
                let body = self.parse_expression(lexer)?;

                Ok(Expr::Var {
                    varnames: pairs,
                    body: Box::new(body),
                })
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
                if let Token::LParen(_) = next {
                    lexer.consume_assert_next_token(Token::LParen('('))?;

                    // Parse Arguments if any exists
                    let mut args = Vec::new();
                    if !matches!(lexer.peek_token(), Token::RParen(_)) {
                        args.push(self.parse_expression(lexer)?);
                        while matches!(lexer.peek_token(), Token::Comma(_)) {
                            lexer.consume_assert_next_token(Token::Comma(','))?;
                            if matches!(lexer.peek_token(), Token::RParen(_)) {
                                break;
                            } // allow trailing comma
                            args.push(self.parse_expression(lexer)?);
                        }
                    }

                    lexer.consume_assert_next_token(Token::RParen(')'))?;
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
                let condition = Box::new(self.parse_expression(lexer)?);
                lexer.consume_assert_next_token(Token::Then)?;
                let then = Box::new(self.parse_expression(lexer)?);
                lexer.consume_assert_next_token(Token::Else)?;
                let els = Box::new(self.parse_expression(lexer)?);

                Ok(Expr::If {
                    condition,
                    then,
                    els,
                })
            }

            Token::For => {
                lexer.consume_assert_next_token(Token::For)?;

                let ident: String = match self.parse_primary(lexer)? {
                    Expr::Variable(s) => s,
                    x => Err(format!("Expected Identifier in for-loop but got {:?}", x))?,
                };

                lexer.consume_assert_next_token(Token::Assign('='))?;
                let start = Box::new(self.parse_expression(lexer)?);
                lexer.consume_assert_next_token(Token::Comma(','))?;
                let end = Box::new(self.parse_expression(lexer)?);

                let mut step: Option<Box<Expr>> = None;
                if matches!(lexer.peek_token(), Token::Comma(_)) {
                    lexer.consume_assert_next_token(Token::Comma(','))?;
                    step = Some(Box::new(self.parse_expression(lexer)?));
                }

                lexer.consume_assert_next_token(Token::In)?;
                let body = Box::new(self.parse_expression(lexer)?);
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

    fn parse_unary(&self, lexer: &mut LexerContext) -> Result<Expr, String> {
        // (  )  ,  are all reserved
        match lexer.peek_token() {
            Token::Plus(c)
            | Token::Minus(c)
            | Token::Star(c)
            | Token::Slash(c)
            | Token::Less(c)
            | Token::Greater(c)
            | Token::Assign(c)
            | Token::Bang(c)
            | Token::Pipe(c)
            | Token::Ampersand(c)
            | Token::Caret(c)
            | Token::Percent(c)
            | Token::Dollar(c)
            | Token::At(c)
            | Token::Tilde(c) => {
                lexer.next_token();
                Ok(Expr::Unary {
                    op: c,
                    left: Box::new(self.parse_unary(lexer)?),
                })
            }
            _ => self.parse_primary(lexer),
        }
    }

    // Parse full expressions with binary operators
    fn parse_expression(&self, lexer: &mut LexerContext) -> Result<Expr, String> {
        let expr = self.parse_unary(lexer)?;
        self.parse_binop_rhs(0, Box::new(expr), lexer).map(|b| *b)
    }

    fn parse_top_level_expression(&self, lexer: &mut LexerContext) -> Result<Function, String> {
        // @NOTE : This is a horrible way to handle top-level expressions, but since this is following
        // Kaleidescope https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/LangImpl02.html at least
        // semi-truthfully, that's how we're going to do it as well.

        let f = Function {
            name: String::from("_top_level_expr"),
            args: Vec::new(),
            body: self.parse_expression(lexer)?,
            is_operator: false,
            precedence: None,
        };

        println!("Parsed top level expr {:?}", f);
        Ok(f)
    }

    fn parse_function_definition(&mut self, lexer: &mut LexerContext) -> Result<Function, String> {
        lexer.consume_opt_next_token(Token::Def)?;
        let mut v = self.parse_proto(lexer)?;
        v.body = self.parse_expression(lexer)?;
        Ok(v)
    }

    fn parse_extern(&mut self, lexer: &mut LexerContext) -> Result<Function, String> {
        lexer.consume_opt_next_token(Token::Extern)?;
        self.parse_proto(lexer)
    }

    fn parse_proto(&mut self, lexer: &mut LexerContext) -> Result<Function, String> {
        let mut precedence: Option<f64> = None;
        let mut operator_kind: Option<Token> = None;
        let name = match lexer.next_token() {
            // If it's a binary or unary, it means it is a user-defined overload
            tok @ Token::Binary(c) | tok @ Token::Unary(c) => {
                // this next token maybe be the precedence level (if they specified one)
                precedence = match lexer.peek_token() {
                    Token::Number(n) => {
                        lexer.next_token();
                        Some(n)
                    }
                    _ => None,
                };
                operator_kind = Some(tok.clone());

                let prefix = if matches!(tok, Token::Binary(_)) {
                    "binary"
                } else {
                    "unary"
                };
                format!("{}{}", prefix, c)
            }
            // Otherwise it's just a regular function name
            Token::Identifier(s) => s,
            _ => Err(String::from("Failed to parse identifier or binary/unary"))?,
        };

        let _ = lexer.consume_assert_next_token(Token::LParen('('))?; // Skip Starting parens

        let mut args = Vec::new();
        loop {
            match lexer.next_token() {
                Token::Identifier(s) => args.push(s),
                Token::RParen(_) => break,
                tok => {
                    return Err(format!(
                        "Unexpected token found while parsing args {:?}",
                        tok
                    ))
                }
            }
        }

        // Argument size validation for user-defined operators
        match &operator_kind {
            Some(Token::Binary(c)) => {
                assert_eq!(
                    args.len(),
                    2,
                    "Binary operators require exactly 2 arguments"
                );
                // Register binary operator in precedence table
                let prec = precedence.unwrap_or(30.0) as i8; // Default precedence is 30
                self.binop_precedence.insert(*c, prec);
            }
            Some(Token::Unary(_)) => {
                assert_eq!(args.len(), 1, "Unary operators require exactly 1 argument")
            }
            _ => {}
        }

        let f = Function {
            name,
            args,
            body: Expr::None,
            is_operator: operator_kind.is_some(),
            precedence,
        };
        println!("Parsed function proto {:?}", f);
        Ok(f)
    }
}
