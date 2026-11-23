/* GRAMMAR
* statement :=
      create_table_stmt
    | select_stmt
    | insert_stmt

* create_table_stmt :=
    CREATE TABLE [IF NOT EXISTS]
    identifier
    "(" table_element ("," table_element)* ")"

* table_element :=
    column_def
    | table_constraint

* column_def :=
    identifier
    [type_name]
    column_constraint*

* column_constraint :=
    PRIMARY KEY
    | NOT NULL
    | UNIQUE
    | CHECK "(" expression ")"
    | DEFAULT expression
    | REFERENCES ...

* table_constraint :=
    PRIMARY KEY "(" identifier_list ")"
    | UNIQUE "(" identifier_list ")"
    | CHECK "(" expression ")"
    | FOREIGN KEY "(" identifier_list ")" REFERENCES ...
*/
pub mod ast;
pub mod parser;
pub mod scanner;
pub mod token;

use std::io;

use ast::Statement;
use parser::Parser;
use scanner::Scanner;

pub fn parse_sql(sql: &str) -> io::Result<Statement> {
    let mut scanner = Scanner::new(sql);
    let tokens = scanner
        .scan_tokens()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut parser = Parser::new(tokens);
    parser
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use crate::sql::parse_sql;

    #[test]
    fn parse_create_table() {
        let source = "
            CREATE TABLE apples (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price DECIMAL(10, 2),
                UNIQUE (name, price),
                PRIMARY KEY (id, name)
            );
        ";

        let statement = parse_sql(source).unwrap();

        println!("{:#?}", statement);
    }
}
