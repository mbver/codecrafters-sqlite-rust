use std::{
    io,
    sync::{Arc, Mutex},
};

use crate::{
    database::Database,
    page::{Page, PageKind},
    record::{Cell, SchemaRecord},
    sql::ast::ColumnDefinition,
};

pub struct Table {
    name: String,
    database: Arc<Mutex<Database>>,
    schema: SchemaRecord,
    column_definitions: Vec<ColumnDefinition>,
}

impl Table {
    pub fn load(database: Arc<Mutex<Database>>, table_name: &str) -> io::Result<Self> {
        println!("============ loading table");
        let cells = {
            let mut db = database.lock().unwrap();
            db.schema_cells()?
        };
        println!("============ schema cells");
        let mut schema_record = None;
        for cell in cells {
            let record = SchemaRecord::try_from(cell.record)?;

            if record.record_type == "table" && record.name == table_name {
                schema_record = Some(record);
                break;
            }
        }
        println!("============ schema record");
        let Some(schema) = schema_record else {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        };
        let sql = schema.sql.as_deref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing CREATE TABLE SQL")
        })?;
        println!("============ sql ");
        let statement = crate::sql::parse_sql(sql)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let crate::sql::ast::Statement::CreateTable(create_table) = statement else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid create table statement",
            ));
        };
        println!("============ parse sql ");
        let mut column_definitions = Vec::new();
        for element in create_table.elements {
            if let crate::sql::ast::TableElement::ColumnDefinition(column) = element {
                column_definitions.push(column);
            }
        }
        println!("============ column def");
        Ok(Self {
            name: table_name.to_string(),
            database,
            schema,
            column_definitions,
        })
    }
    pub fn root_page(&self) -> u32 {
        self.schema.rootpage as u32
    }
    pub fn column_index(&self, column_name: &str) -> io::Result<usize> {
        for (i, ColumnDefinition { name, .. }) in self.column_definitions.iter().enumerate() {
            if column_name == name {
                return Ok(i);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Column '{}' not found in table '{}'",
                column_name, self.name
            ),
        ))
    }
    pub fn column_count(&self) -> usize {
        self.column_definitions.len()
    }
    pub fn row_count(&self) -> io::Result<u64> {
        let mut stack = vec![self.root_page()];
        let mut rows = 0u64;
        let mut db = self.database.lock().unwrap();
        while let Some(page_number) = stack.pop() {
            let page = db.load_page(page_number)?;
            match page.header.kind {
                PageKind::LeafTable => rows += page.header.cell_count as u64,
                PageKind::InteriorTable => stack.extend(page.children()?),
                kind => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Unsupported page kind: {kind:?}"),
                    ));
                }
            }
        }
        Ok(rows)
    }
    pub fn scan<F>(&self, predicate: F) -> TableScan<F>
    where
        F: FnMut(&Cell) -> io::Result<bool>,
    {
        TableScan {
            db: self.database.clone(),
            stack: vec![self.root_page()],
            predicate,
        }
    }
}

pub struct TableScan<F> {
    db: Arc<Mutex<Database>>,
    stack: Vec<u32>,
    predicate: F,
}

impl<F: FnMut(&Cell) -> io::Result<bool>> TableScan<F> {
    /// Advances to the next leaf and returns its matching cells (possibly empty);
    /// `None` once the tree is exhausted.
    pub fn leaf_page_cells(&mut self) -> io::Result<Option<Vec<Cell>>> {
        let mut db = self.db.lock().unwrap();
        while let Some(page) = self.stack.pop() {
            match db.read_page(page)? {
                Page::Leaf(cells) => {
                    let mut matched = Vec::with_capacity(cells.len());
                    for cell in cells {
                        if (self.predicate)(&cell)? {
                            matched.push(cell);
                        }
                    }
                    return Ok(Some(matched));
                }
                Page::Interior(children) => self.stack.extend(children.into_iter().rev()),
            }
        }
        Ok(None)
    }
}
