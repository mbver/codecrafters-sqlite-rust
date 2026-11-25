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
    page_1: RawPage, // TODO: WHAT IF PAGE 1 IS AN INTERNAL NODE, NOT A LEAF NODE?
}

impl Database {
    pub fn open(path: &str) -> io::Result<Self> {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(16))?;
        let mut buf = [0u8; 2];
        file.read_exact(&mut buf)?;
        let raw = u16::from_be_bytes(buf);
        let page_size = if raw == 1 { 65536 } else { raw as u32 };
        let page_1 = Self::load_page_internal(&mut file, page_size as u64, 1)?;
        Ok(Self {
            reader: file,
            page_size,
            page_1,
        })
    }

    pub fn table_count(&mut self) -> u16 {
        self.page_1.header.cell_count
    }

    pub fn schema_cells(&mut self) -> io::Result<Vec<Cell>> {
        self.page_1.leaf_cells()
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

    fn load_page_internal(
        reader: &mut File,
        page_size: u64,
        page_number: u32,
    ) -> io::Result<RawPage> {
        if page_number == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid page number",
            ));
        }
        reader.seek(SeekFrom::Start((page_number as u64 - 1) * page_size))?;
        let mut bytes = vec![0u8; page_size as usize];
        reader.read_exact(&mut bytes)?;
        RawPage::parse(bytes, page_number)
    }

    pub fn load_page(&mut self, page_number: u32) -> io::Result<RawPage> {
        Self::load_page_internal(&mut self.reader, self.page_size as u64, page_number)
    }

    pub fn read_page(&mut self, page_number: u32) -> io::Result<Page> {
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
