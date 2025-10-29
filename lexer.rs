#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Eof,
    Def,
    Extern,
    Identifier(String),
    Number(f64),
    LParen,
    RParen,
    Plus,
    Minus,
    Star,
    Slash,
    Comma,
    Less,
    Greater,
    If,
    Else,
    Then,
    For,
    In,
    Assign,
    Binary,
    Unary,
    Bang,
    Pipe,
    Ampersand,
    Caret,
    Percent,
    Dollar,
    At,
    Tilde,
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
                '(' => Some(Token::LParen),
                ')' => Some(Token::RParen),
                '+' => Some(Token::Plus),
                ',' => Some(Token::Comma),
                '-' => Some(Token::Minus),
                '/' => Some(Token::Slash),
                '*' => Some(Token::Star),
                '>' => Some(Token::Greater),
                '<' => Some(Token::Less),
                '=' => Some(Token::Assign),
                '!' => Some(Token::Bang),
                '|' => Some(Token::Pipe),
                '&' => Some(Token::Ampersand),
                '^' => Some(Token::Caret),
                '%' => Some(Token::Percent),
                '$' => Some(Token::Dollar),
                '@' => Some(Token::At),
                '~' => Some(Token::Tilde),
                _ => None,
            };

            if let Some(tok) = token {
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
                    "def" => Token::Def,
                    "if" => Token::If,
                    "else" => Token::Else,
                    "then" => Token::Then,
                    "for" => Token::For,
                    "in" => Token::In,
                    "binary" => Token::Binary,
                    "unary" => Token::Unary,
                    _ => {
                        println!("{:?}", ident);
                        Token::Identifier(ident.to_string())
                    }
                };
                tokens.push(tok);
                continue;
            }

            // Unknown character - skip it
            cursor += cchar.len_utf8();
        }

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
