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
    None,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub args: Vec<String>,
    pub body: Expr,
}
