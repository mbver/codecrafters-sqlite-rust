use super::ast::{
    ColumnConstraint, ColumnDefinition, ComparisonOperator, CreateTable, Expression, Literal,
    Operand, SelectColumns, SelectStatement, Statement, TableConstraint, TableElement,
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
            Token::Select => Ok(Statement::Select(self.select_stmt()?)),
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

        while matches!(
            self.peek(),
            Token::Primary | Token::Not | Token::Unique | Token::Autoincrement
        ) {
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
            Token::Autoincrement => {
                self.advance();
                Ok(ColumnConstraint::AutoIncrement)
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
    fn select_stmt(&mut self) -> Result<SelectStatement, String> {
        self.consume(&Token::Select, "Expected SELECT")?;

        let columns = self.select_columns()?;

        self.consume(&Token::From, "Expected FROM after select columns")?;

        let table_name = self.identifier()?;

        let where_clause = if self.matches(&Token::Where) {
            Some(self.where_clause()?)
        } else {
            None
        };

        Ok(SelectStatement {
            columns,
            table_name,
            where_clause,
        })
    }
    fn select_columns(&mut self) -> Result<SelectColumns, String> {
        if self.matches(&Token::Star) {
            self.advance();
            return Ok(SelectColumns::All);
        }

        // COUNT(*)
        if matches!(
            self.peek(),
            Token::Identifier(name) if name.eq_ignore_ascii_case("COUNT")
        ) {
            self.advance();

            self.consume(&Token::LeftParen, "Expected '(' after COUNT")?;
            self.consume(&Token::Star, "Expected '*' inside COUNT")?;
            self.consume(&Token::RightParen, "Expected ')' after COUNT(*)")?;

            return Ok(SelectColumns::Count);
        }

        let mut columns = vec![self.identifier()?];

        while self.matches(&Token::Comma) {
            self.advance();
            columns.push(self.identifier()?);
        }

        Ok(SelectColumns::Columns(columns))
    }
    fn where_clause(&mut self) -> Result<Expression, String> {
        self.consume(&Token::Where, "Expected WHERE")?;

        self.expression()
    }
    fn expression(&mut self) -> Result<Expression, String> {
        self.comparison()
    }
    fn comparison(&mut self) -> Result<Expression, String> {
        let left = self.operand()?;

        let operator = self.comparison_operator()?;

        let right = self.operand()?;

        Ok(Expression::Comparison {
            left,
            operator,
            right,
        })
    }
    fn comparison_operator(&mut self) -> Result<ComparisonOperator, String> {
        let operator = match self.advance() {
            Token::Equal => ComparisonOperator::Equal,
            Token::NotEqual => ComparisonOperator::NotEqual,
            Token::LessThan => ComparisonOperator::LessThan,
            Token::LessThanOrEqual => ComparisonOperator::LessThanOrEqual,
            Token::GreaterThan => ComparisonOperator::GreaterThan,
            Token::GreaterThanOrEqual => ComparisonOperator::GreaterThanOrEqual,
            token => {
                return Err(format!("Expected comparison operator, found {:?}", token));
            }
        };

        Ok(operator)
    }
    fn operand(&mut self) -> Result<Operand, String> {
        match self.peek() {
            Token::Identifier(_) => Ok(Operand::Column(self.identifier()?)),
            Token::String(_) => Ok(Operand::Literal(self.string()?)),
            Token::Number(_) => Ok(Operand::Literal(self.number_literal()?)),
            Token::Null => {
                self.advance();
                Ok(Operand::Literal(Literal::Null))
            }
            token => Err(format!("Expected operand, found {:?}", token)),
        }
    }
    fn string(&mut self) -> Result<Literal, String> {
        match self.advance() {
            Token::String(value) => Ok(Literal::String(value)),
            token => Err(format!("Expected string, found {:?}", token)),
        }
    }

    fn number_literal(&mut self) -> Result<Literal, String> {
        match self.advance() {
            Token::Number(value) => Ok(Literal::Number(value)),
            token => Err(format!("Expected number, found {:?}", token)),
        }
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
