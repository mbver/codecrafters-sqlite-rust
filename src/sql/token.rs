#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Create,
    Table,
    If,
    Exists,
    Primary,
    Key,
    Not,
    Null,
    Unique,
    Autoincrement,
    Index,
    On,
    // SELECT keywords
    Select,
    From,
    Where,

    // Identifiers and literals
    Identifier(String),
    Number(String),
    String(String),

    // Comparison operators
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Symbols
    Star,
    LeftParen,
    RightParen,
    Comma,
    Semicolon,

    Eof,
}
