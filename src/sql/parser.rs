use super::ast::{
    ColumnConstraint, ColumnDefinition, CreateTable, Statement, TableConstraint, TableElement,
};
use super::token::Token;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<Statement, String> {
        let statement = self.statement()?;

        if self.matches(&Token::Semicolon) {
            self.advance();
        }

        self.consume(&Token::Eof, "Expected end of statement")?;

        Ok(statement)
    }

    fn statement(&mut self) -> Result<Statement, String> {
        match self.peek() {
            Token::Create => Ok(Statement::CreateTable(self.create_table_stmt()?)),
            _ => Err(format!("Unexpected token: {:?}", self.peek())),
        }
    }

    fn create_table_stmt(&mut self) -> Result<CreateTable, String> {
        self.consume(&Token::Create, "Expected CREATE")?;
        self.consume(&Token::Table, "Expected TABLE")?;

        // Optional IF NOT EXISTS.
        if self.matches(&Token::If) {
            self.advance();
            self.consume(&Token::Not, "Expected NOT after IF")?;
            self.consume(&Token::Exists, "Expected EXISTS after IF NOT")?;
        }

        let table_name = self.identifier()?;

        self.consume(&Token::LeftParen, "Expected '(' after table name")?;

        let mut elements = vec![self.table_element()?];

        while self.matches(&Token::Comma) {
            self.advance();
            elements.push(self.table_element()?);
        }

        self.consume(&Token::RightParen, "Expected ')' after table elements")?;

        Ok(CreateTable {
            table_name,
            elements,
        })
    }

    fn table_element(&mut self) -> Result<TableElement, String> {
        match self.peek() {
            Token::Primary | Token::Unique => {
                Ok(TableElement::TableConstraint(self.table_constraint()?))
            }
            Token::Identifier(_) => Ok(TableElement::ColumnDefinition(self.column_def()?)),
            _ => Err(format!(
                "Expected column definition or table constraint, found {:?}",
                self.peek()
            )),
        }
    }

    fn column_def(&mut self) -> Result<ColumnDefinition, String> {
        let name = self.identifier()?;

        // Optional type name.
        let data_type = if matches!(self.peek(), Token::Identifier(_)) {
            Some(self.type_name()?)
        } else {
            None
        };

        let mut constraints = Vec::new();

        while matches!(self.peek(), Token::Primary | Token::Not | Token::Unique) {
            constraints.push(self.column_constraint()?);
        }

        Ok(ColumnDefinition {
            name,
            data_type,
            constraints,
        })
    }

    fn type_name(&mut self) -> Result<String, String> {
        let mut name = self.identifier()?;

        // Optional type parameters, e.g. VARCHAR(255), DECIMAL(10, 2).
        if self.matches(&Token::LeftParen) {
            self.advance();

            let first = self.number()?;
            name.push_str(&format!("({first}"));

            if self.matches(&Token::Comma) {
                self.advance();

                let second = self.number()?;
                name.push_str(&format!(", {second}"));
            }

            self.consume(&Token::RightParen, "Expected ')' after type parameters")?;

            name.push(')');
        }

        Ok(name)
    }

    fn column_constraint(&mut self) -> Result<ColumnConstraint, String> {
        match self.peek() {
            Token::Primary => {
                self.advance();
                self.consume(&Token::Key, "Expected KEY after PRIMARY")?;
                Ok(ColumnConstraint::PrimaryKey)
            }
            Token::Not => {
                self.advance();
                self.consume(&Token::Null, "Expected NULL after NOT")?;
                Ok(ColumnConstraint::NotNull)
            }
            Token::Unique => {
                self.advance();
                Ok(ColumnConstraint::Unique)
            }
            _ => Err(format!(
                "Expected column constraint, found {:?}",
                self.peek()
            )),
        }
    }

    fn table_constraint(&mut self) -> Result<TableConstraint, String> {
        match self.peek() {
            Token::Primary => {
                self.advance();
                self.consume(&Token::Key, "Expected KEY after PRIMARY")?;

                self.consume(&Token::LeftParen, "Expected '(' after PRIMARY KEY")?;

                let columns = self.identifier_list()?;

                self.consume(&Token::RightParen, "Expected ')' after primary key columns")?;

                Ok(TableConstraint::PrimaryKey(columns))
            }
            Token::Unique => {
                self.advance();

                self.consume(&Token::LeftParen, "Expected '(' after UNIQUE")?;

                let columns = self.identifier_list()?;

                self.consume(&Token::RightParen, "Expected ')' after unique columns")?;

                Ok(TableConstraint::Unique(columns))
            }
            _ => Err(format!(
                "Expected table constraint, found {:?}",
                self.peek()
            )),
        }
    }

    fn identifier_list(&mut self) -> Result<Vec<String>, String> {
        let mut identifiers = vec![self.identifier()?];

        while self.matches(&Token::Comma) {
            self.advance();
            identifiers.push(self.identifier()?);
        }

        Ok(identifiers)
    }

    fn identifier(&mut self) -> Result<String, String> {
        match self.advance() {
            Token::Identifier(name) => Ok(name),
            token => Err(format!("Expected identifier, found {:?}", token)),
        }
    }

    fn number(&mut self) -> Result<String, String> {
        match self.advance() {
            Token::Number(value) => Ok(value),
            token => Err(format!("Expected number, found {:?}", token)),
        }
    }

    fn consume(&mut self, expected: &Token, message: &str) -> Result<(), String> {
        if self.matches(expected) {
            self.advance();
            Ok(())
        } else {
            Err(format!("{message}, found {:?}", self.peek()))
        }
    }

    fn matches(&self, expected: &Token) -> bool {
        self.peek() == expected
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();

        if !self.is_at_end() {
            self.current += 1;
        }

        token
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }
}
