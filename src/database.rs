use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
};

use crate::{
    page::{Page, PageKind, RawPage},
    record::{Cell, SchemaRecord},
};

pub struct Database {
    reader: File,
    pub page_size: u32,
}

impl Database {
    pub fn open(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = file;
        reader.seek(SeekFrom::Start(16))?;
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        let raw = u16::from_be_bytes(buf);
        let page_size = if raw == 1 { 65536 } else { raw as u32 };
        Ok(Self { reader, page_size })
    }

    pub fn table_count(&mut self) -> io::Result<u16> {
        Ok(self.load_page(1)?.header.cell_count)
    }

    pub fn schema_cells(&mut self) -> io::Result<Vec<Cell>> {
        self.page_cells(1)
    }
    pub fn table_names(&mut self) -> io::Result<Vec<String>> {
        let cells = self.schema_cells()?;

        let mut names = Vec::with_capacity(cells.len());

        for cell in cells {
            let record = SchemaRecord::try_from(cell.record)?;
            names.push(record.tbl_name);
        }

        Ok(names)
    }
    pub fn table_schema(&mut self, table_name: &str) -> io::Result<SchemaRecord> {
        let cells = self.schema_cells()?;

        for cell in cells {
            let record = SchemaRecord::try_from(cell.record)?;

            if record.record_type == "table" && record.name == table_name {
                return Ok(record);
            }
        }

        Err(io::Error::from(io::ErrorKind::NotFound))
    }
    pub fn table_rootpage(&mut self, table_name: &str) -> io::Result<u32> {
        let schema = self.table_schema(table_name)?;
        Ok(schema.rootpage as u32)
    }

    pub fn table_column_index(&mut self, table_name: &str, column_name: &str) -> io::Result<usize> {
        let schema = self.table_schema(table_name)?;

        let sql = schema.sql.as_deref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing CREATE TABLE SQL")
        })?;

        let statement = crate::sql::parse_sql(sql)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let crate::sql::ast::Statement::CreateTable(create_table) = statement else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid create table statement",
            ));
        };

        let mut index = 0;

        for element in create_table.elements {
            if let crate::sql::ast::TableElement::ColumnDefinition(column) = element {
                if column.name.eq_ignore_ascii_case(column_name) {
                    return Ok(index);
                }

                index += 1;
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Column '{}' not found in table '{}'",
                column_name, table_name
            ),
        ))
    }

    pub fn scan_table<F>(&mut self, table_name: &str, predicate: F) -> io::Result<TableScan<'_, F>>
    where
        F: FnMut(&Cell) -> io::Result<bool>,
    {
        let root = self.table_rootpage(table_name)?;
        Ok(TableScan {
            db: self,
            stack: vec![root],
            predicate,
        })
    }

    pub fn table_row_count(&mut self, table_name: &str) -> io::Result<u64> {
        let mut stack = vec![self.table_rootpage(table_name)?];
        let mut rows = 0u64;
        while let Some(page_number) = stack.pop() {
            let page = self.load_page(page_number)?;
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

    fn load_page(&mut self, page_number: u32) -> io::Result<RawPage> {
        if page_number == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid page number",
            ));
        }
        self.reader.seek(SeekFrom::Start(
            (page_number as u64 - 1) * self.page_size as u64,
        ))?;
        let mut bytes = vec![0u8; self.page_size as usize];
        self.reader.read_exact(&mut bytes)?;
        RawPage::parse(bytes, page_number)
    }

    pub fn page_cells(&mut self, page_number: u32) -> io::Result<Vec<Cell>> {
        self.load_page(page_number)?.leaf_cells()
    }

    fn read_page(&mut self, page_number: u32) -> io::Result<Page> {
        let page = self.load_page(page_number)?;
        match page.header.kind {
            PageKind::LeafTable => Ok(Page::Leaf(page.leaf_cells()?)),
            PageKind::InteriorTable => Ok(Page::Interior(page.children()?)),
            kind => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported page kind: {kind:?}"),
            )),
        }
    }
}

pub struct TableScan<'a, F> {
    db: &'a mut Database,
    stack: Vec<u32>,
    predicate: F,
}

impl<F: FnMut(&Cell) -> io::Result<bool>> TableScan<'_, F> {
    /// Advances to the next leaf and returns its matching cells (possibly empty);
    /// `None` once the tree is exhausted.
    pub fn leaf_page_cells(&mut self) -> io::Result<Option<Vec<Cell>>> {
        while let Some(page) = self.stack.pop() {
            match self.db.read_page(page)? {
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
