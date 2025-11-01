#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Eof,
    Def,
    Extern,
    Identifier(String),
    Number(f64),
    LParen(char),
    RParen(char),
    Plus(char),
    Minus(char),
    Star(char),
    Slash(char),
    Comma(char),
    Less(char),
    Greater(char),
    If,
    Else,
    Var,
    Then,
    For,
    In,
    Assign(char),
    Bang(char),
    Pipe(char),
    Ampersand(char),
    Caret(char),
    Percent(char),
    Dollar(char),
    At(char),
    Tilde(char),
    Binary(char),
    Unary(char),
}

pub struct LexerContext {
    tokens: Vec<Token>,
    position: usize,
}

impl LexerContext {
    pub fn new() -> Self {
        LexerContext {
            tokens: Vec::new(),
            position: 0,
        }
    }

    pub fn lex(&mut self, input: &str) {
        let mut tokens = Vec::new();
        let mut cursor = 0;

        while cursor < input.len() {
            let remaining = &input[cursor..];
            let mut chars = remaining.chars();
            let cchar = match chars.next() {
                Some(c) => c,
                None => break,
            };

            // Skip whitespace
            if cchar.is_whitespace() {
                cursor += cchar.len_utf8();
                continue;
            }

            // Skip line comments
            if cchar == '#' {
                while cursor < input.len() {
                    let c = input[cursor..].chars().next().unwrap();
                    cursor += c.len_utf8();
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }

            // Single character tokens
            let token = match cchar {
                '(' => Some(Token::LParen(cchar)),
                ')' => Some(Token::RParen(cchar)),
                '+' => Some(Token::Plus(cchar)),
                ',' => Some(Token::Comma(cchar)),
                '-' => Some(Token::Minus(cchar)),
                '/' => Some(Token::Slash(cchar)),
                '*' => Some(Token::Star(cchar)),
                '>' => Some(Token::Greater(cchar)),
                '<' => Some(Token::Less(cchar)),
                '=' => Some(Token::Assign(cchar)),
                '!' => Some(Token::Bang(cchar)),
                '|' => Some(Token::Pipe(cchar)),
                '&' => Some(Token::Ampersand(cchar)),
                '^' => Some(Token::Caret(cchar)),
                '%' => Some(Token::Percent(cchar)),
                '$' => Some(Token::Dollar(cchar)),
                '@' => Some(Token::At(cchar)),
                '~' => Some(Token::Tilde(cchar)),
                _ => None,
            };

            if let Some(tok) = token {
                println!("TOK: {:?}", tok);
                tokens.push(tok);
                cursor += cchar.len_utf8();
                continue;
            }

            // Numbers
            if cchar.is_ascii_digit() {
                let start = cursor;
                cursor += cchar.len_utf8();
                let mut has_dot = false;

                while cursor < input.len() {
                    let c = input[cursor..].chars().next().unwrap();
                    if c.is_ascii_digit() {
                        cursor += c.len_utf8();
                    } else if c == '.' && !has_dot {
                        has_dot = true;
                        cursor += c.len_utf8();
                    } else {
                        break;
                    }
                }

                let nval = input[start..cursor].parse::<f64>().unwrap();
                println!("TOK: {:?}", Token::Number(nval));
                tokens.push(Token::Number(nval));
                continue;
            }

            // Identifiers and keywords
            if cchar.is_alphabetic() {
                let start = cursor;
                cursor += cchar.len_utf8();

                while cursor < input.len() {
                    let c = input[cursor..].chars().next().unwrap();
                    if c.is_alphanumeric() {
                        cursor += c.len_utf8();
                    } else {
                        break;
                    }
                }

                let ident = &input[start..cursor];
                let tok = match ident {
                    "extern" => Token::Extern,
                    "var" => Token::Var,
                    "def" => Token::Def,
                    "if" => Token::If,
                    "else" => Token::Else,
                    "then" => Token::Then,
                    "for" => Token::For,
                    "in" => Token::In,
                    "binary" => {
                        if cursor >= input.len() {
                            panic!("Expected a char after unary identifier")
                        };
                        cursor += 1;
                        Token::Binary(input.chars().nth(cursor - 1).unwrap())
                    }
                    "unary" => {
                        if cursor >= input.len() {
                            panic!("Expected a char after unary identifier")
                        };
                        cursor += 1;
                        Token::Unary(input.chars().nth(cursor - 1).unwrap())
                    }
                    _ => {
                        println!("{:?}", ident);
                        Token::Identifier(ident.to_string())
                    }
                };
                println!("TOK: {:?}", tok);
                tokens.push(tok);
                continue;
            }

            // Unknown character - skip it
            cursor += cchar.len_utf8();
        }

        println!("TOK: {:?}", Token::Eof);
        tokens.push(Token::Eof);
        self.tokens = tokens;
    }

    pub fn next_token(&mut self) -> Token {
        if self.position < self.tokens.len() {
            let tok = self.tokens[self.position].clone();
            self.position += 1;
            tok
        } else {
            Token::Eof
        }
    }

    pub fn peek_token(&self) -> Token {
        if self.position < self.tokens.len() {
            self.tokens[self.position].clone()
        } else {
            Token::Eof
        }
    }

    pub fn consume_assert_next_token(&mut self, expected: Token) -> Result<Token, String> {
        let tok = self.next_token();
        if std::mem::discriminant(&tok) == std::mem::discriminant(&expected) {
            Ok(tok)
        } else {
            Err(format!("Expected {:?}, got {:?}", expected, tok))
        }
    }

    pub fn consume_opt_next_token(&mut self, expected: Token) -> Result<Option<Token>, String> {
        let tok = self.peek_token();
        if std::mem::discriminant(&tok) == std::mem::discriminant(&expected) {
            let t = self.next_token();
            Ok(Some(t))
        } else {
            Ok(None)
        }
    }
}
