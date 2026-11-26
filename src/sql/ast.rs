#[derive(Debug)]
pub enum Statement {
    CreateTable(CreateTable),
    Select(SelectStatement),
    // Insert(Insert),
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct CreateTable {
    pub table_name: String,
    pub elements: Vec<TableElement>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum TableElement {
    ColumnDefinition(ColumnDefinition),
    TableConstraint(TableConstraint),
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: Option<String>,
    pub constraints: Vec<ColumnConstraint>,
}
#[derive(Debug)]
pub enum ColumnConstraint {
    PrimaryKey,
    NotNull,
    Unique,
    AutoIncrement,
    // Check(Expression),
    // Default(Expression),
    // References(References),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum TableConstraint {
    PrimaryKey(Vec<String>),
    Unique(Vec<String>),
    // Check(Expression),
    // ForeignKey {
    //     columns: Vec<String>,
    //     references: References,
    // },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub columns: SelectColumns,
    pub table_name: String,
    pub where_clause: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectColumns {
    All,
    Count,
    Columns(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Comparison {
        left: Operand,
        operator: ComparisonOperator,
        right: Operand,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Column(String),
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Number(String),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}
