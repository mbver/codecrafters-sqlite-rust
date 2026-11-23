use super::token::Token;

pub struct Scanner {
    source: Vec<char>,
    current: usize,
}

impl Scanner {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            current: 0,
        }
    }

    pub fn scan_tokens(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.skip_whitespace();

            if self.is_at_end() {
                break;
            }

            tokens.push(self.scan_token()?);
        }

        tokens.push(Token::Eof);

        Ok(tokens)
    }

    fn scan_token(&mut self) -> Result<Token, String> {
        let ch = self.advance();

        match ch {
            '(' => Ok(Token::LeftParen),
            ')' => Ok(Token::RightParen),
            ',' => Ok(Token::Comma),
            ';' => Ok(Token::Semicolon),

            'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(ch),

            '0'..='9' => self.scan_number(ch),

            _ => Err(format!("Unexpected character: '{}'", ch)),
        }
    }

    fn scan_identifier(&mut self, first: char) -> Result<Token, String> {
        let mut identifier = String::from(first);

        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                identifier.push(self.advance());
            } else {
                break;
            }
        }

        let token = match identifier.to_ascii_uppercase().as_str() {
            "CREATE" => Token::Create,
            "TABLE" => Token::Table,
            "IF" => Token::If,
            "EXISTS" => Token::Exists,
            "PRIMARY" => Token::Primary,
            "KEY" => Token::Key,
            "NOT" => Token::Not,
            "NULL" => Token::Null,
            "UNIQUE" => Token::Unique,
            _ => Token::Identifier(identifier),
        };

        Ok(token)
    }

    fn scan_number(&mut self, first: char) -> Result<Token, String> {
        let mut number = String::from(first);

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                number.push(self.advance());
            } else {
                break;
            }
        }

        Ok(Token::Number(number))
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.source[self.current];
        self.current += 1;
        ch
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.current).copied()
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }
}
