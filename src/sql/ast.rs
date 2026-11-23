#[derive(Debug)]
pub enum Statement {
    CreateTable(CreateTable),
    // Select(Select),
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
