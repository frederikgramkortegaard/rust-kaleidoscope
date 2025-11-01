use crate::lexer::Token;

#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Variable(String),
    BinOp {
        left: Box<Expr>,
        op: Token,
        right: Box<Expr>,
    },
    Call {
        identifier: String,
        args: Vec<Expr>,
    },
    If {
        condition: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    For {
        ident: String,
        start: Box<Expr>,
        end: Box<Expr>,
        step: Option<Box<Expr>>,
        body: Box<Expr>,
    },
    Unary {
        op: char,
        left: Box<Expr>,
    },
    Var {
        varnames: Vec<(String, Option<Expr>)>,
        body: Box<Expr>,
    },
    None,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub args: Vec<String>,
    pub body: Expr,
    pub is_operator: bool,
    pub precedence: Option<f64>,
}
