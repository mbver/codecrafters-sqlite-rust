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

    // Identifiers and literals
    Identifier(String),
    Number(String),

    // Symbols
    LeftParen,
    RightParen,
    Comma,
    Semicolon,

    Eof,
}
