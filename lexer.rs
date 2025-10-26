use std::cmp;

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
}

pub struct LexerContext<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> LexerContext<'a> {
    pub fn new(input: &'a str) -> Self {
        LexerContext { input, cursor: 0 }
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.input[self.cursor..].chars().next()?;
        self.cursor += ch.len_utf8();
        Some(ch)
    }

    fn backtrack(&mut self, n: usize) {
        self.cursor = cmp::max(self.cursor - n, 0);
    }

    pub fn peek(&mut self) -> Option<char> {
        let c = self.next_char();
        self.backtrack(1);
        c
    }

    fn skip_to(&mut self, c: char) {
        while let Some(cchar) = self.next_char() {
            if cchar == c {
                break;
            }
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

    pub fn peek_token(&mut self) -> Token {
        let cursor_state = self.cursor;
        let tok = self.next_token();
        self.cursor = cursor_state;
        tok
    }
    pub fn next_token(&mut self) -> Token {
        while let Some(cchar) = self.next_char() {
            // Skip whitespace
            if cchar.is_whitespace() {
                continue;
            }

            // Skip line comments
            if cchar == '#' {
                self.skip_to('\n');
                continue;
            }

            match cchar {
                '(' => return Token::LParen,
                ')' => return Token::RParen,
                '+' => return Token::Plus,
                '-' => return Token::Minus,
                _ => {}
            }

            // Numbers
            if cchar.is_ascii_digit() {
                let mut buf = String::from(cchar);
                let mut _rc = false;
                while let Some(cchar) = self.next_char() {
                    if cchar.is_ascii_digit() {
                        buf.push(cchar);
                    } else if cchar == '.' && !_rc {
                        buf.push(cchar);
                        _rc = true;
                    } else {
                        self.backtrack(cchar.len_utf8());
                        break;
                    }
                }

                let nval = buf.trim().parse::<f64>().unwrap();
                return Token::Number(nval);
            }

            // Identifiers
            if cchar.is_alphabetic() {
                let mut buf = String::from(cchar);
                while let Some(cchar) = self.next_char() {
                    if cchar.is_alphanumeric() {
                        buf.push(cchar);
                    } else {
                        self.backtrack(1);
                        break;
                    }
                }

                return match buf.as_str() {
                    "extern" => Token::Extern,
                    "def" => Token::Def,
                    _ => Token::Identifier(buf),
                };
            }
        }

        // End of input
        Token::Eof
    }
}
