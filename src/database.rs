use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
};

use crate::record::{Cell, SchemaRecord, Value, read_cell};

pub struct Database {
    reader: BufReader<File>,
}

impl Database {
    pub fn open(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;

        Ok(Self {
            reader: BufReader::new(file),
        })
    }

    pub fn page_size(&mut self) -> io::Result<u16> {
        self.reader.seek(SeekFrom::Start(16))?;

        let mut buffer = [0u8; 2];
        self.reader.read_exact(&mut buffer)?;

        Ok(u16::from_be_bytes(buffer))
    }

    pub fn table_count(&mut self) -> io::Result<u16> {
        self.reader.seek(SeekFrom::Start(103))?;

        let mut buffer = [0u8; 2];
        self.reader.read_exact(&mut buffer)?;

        Ok(u16::from_be_bytes(buffer))
    }
    pub fn page_cell_count(&mut self, page_number: u32) -> io::Result<u16> {
        let page_size = self.page_size()? as u64;
        let page_offset = (page_number as u64 - 1) * page_size;

        let header_offset = if page_number == 1 {
            page_offset + 100
        } else {
            page_offset
        };

        self.reader.seek(SeekFrom::Start(header_offset))?;

        let mut page_header = [0u8; 8];
        self.reader.read_exact(&mut page_header)?;

        Ok(u16::from_be_bytes([page_header[3], page_header[4]]))
    }
    pub fn page_cell_pointers(
        &mut self,
        page_number: u32,
        cell_count: u16,
    ) -> io::Result<Vec<usize>> {
        let page_size = self.page_size()? as u64;
        let page_offset = (page_number as u64 - 1) * page_size;

        let header_offset = if page_number == 1 {
            page_offset + 100
        } else {
            page_offset
        };

        let pointer_offset = header_offset + 8;

        self.reader.seek(SeekFrom::Start(pointer_offset))?;

        let mut pointers = Vec::with_capacity(cell_count as usize);

        for _ in 0..cell_count {
            let mut buffer = [0u8; 2];
            self.reader.read_exact(&mut buffer)?;

            let pointer = u16::from_be_bytes(buffer);
            pointers.push(pointer as usize);
        }

        Ok(pointers)
    }
    pub fn page_cells(&mut self, page_number: u32) -> io::Result<Vec<Cell>> {
        if page_number == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid page number",
            ));
        }

        let cell_count = self.page_cell_count(page_number)?;
        let pointers = self.page_cell_pointers(page_number, cell_count)?;

        let page_size = self.page_size()? as u64;
        let page_offset = (page_number as u64 - 1) * page_size;

        let mut cells = Vec::with_capacity(cell_count as usize);

        for pointer in pointers {
            let cell_offset = page_offset + pointer as u64;

            self.reader.seek(SeekFrom::Start(cell_offset))?;
            cells.push(read_cell(&mut self.reader)?);
        }

        Ok(cells)
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
    pub fn table_row_count(&mut self, table_name: &str) -> io::Result<u16> {
        let rootpage = self.table_rootpage(table_name)?;
        self.page_cell_count(rootpage)
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

        let create_table = match statement {
            crate::sql::ast::Statement::CreateTable(create_table) => create_table,
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
    pub fn table_cells(&mut self, table_name: &str) -> io::Result<Vec<Cell>> {
        let rootpage = self.table_rootpage(table_name)?;
        self.page_cells(rootpage)
    }
    pub fn table_column_values(
        &mut self,
        table_name: &str,
        column_names: &[&str],
    ) -> io::Result<Vec<Vec<Value>>> {
        let column_indices = column_names
            .iter()
            .map(|name| self.table_column_index(table_name, name))
            .collect::<io::Result<Vec<_>>>()?; // collect Result<X> into Result<Vec<X>>

        let cells = self.table_cells(table_name)?;

        let mut rows = Vec::with_capacity(cells.len());

        for cell in cells {
            let mut values = Vec::with_capacity(column_indices.len());

            for &column_index in &column_indices {
                let value = cell
                    .record
                    .values
                    .get(column_index)
                    .cloned()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Column index {} is missing from record", column_index),
                        )
                    })?;

                values.push(value);
            }

            rows.push(values);
        }

        Ok(rows)
    }
}
