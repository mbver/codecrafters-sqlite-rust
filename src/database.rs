use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
};

use crate::record::{Cell, SchemaRecord, read_cell};

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
    pub fn schema_cell_count(&mut self) -> io::Result<u16> {
        self.reader.seek(SeekFrom::Start(100))?;

        let mut page_header = [0u8; 8];
        self.reader.read_exact(&mut page_header)?;

        Ok(u16::from_be_bytes([page_header[3], page_header[4]]))
    }
    pub fn schema_cell_pointers(&mut self, cell_count: u16) -> io::Result<Vec<usize>> {
        self.reader.seek(SeekFrom::Start(108))?;

        let mut pointers = Vec::with_capacity(cell_count as usize);

        for _ in 0..cell_count {
            let mut buffer = [0u8; 2];
            self.reader.read_exact(&mut buffer)?;

            let pointer = u16::from_be_bytes(buffer);
            pointers.push(pointer as usize);
        }

        Ok(pointers)
    }
    pub fn schema_cells(&mut self) -> io::Result<Vec<Cell>> {
        let cell_count = self.schema_cell_count()?;
        let pointers = self.schema_cell_pointers(cell_count)?;

        let mut cells = Vec::with_capacity(cell_count as usize);

        for pointer in pointers {
            self.reader.seek(SeekFrom::Start(pointer as u64))?;

            let cell = read_cell(&mut self.reader)?;
            cells.push(cell);
        }

        Ok(cells)
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
}
