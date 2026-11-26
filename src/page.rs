use std::io;

use crate::record::{Cell, Value, read_cell, read_record, read_varint};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageKind {
    InteriorIndex, // 0x02
    InteriorTable, // 0x05
    LeafIndex,     // 0x0a
    LeafTable,     // 0x0d
}

impl PageKind {
    pub fn from_byte(byte: u8) -> io::Result<Self> {
        match byte {
            0x02 => Ok(Self::InteriorIndex),
            0x05 => Ok(Self::InteriorTable),
            0x0a => Ok(Self::LeafIndex),
            0x0d => Ok(Self::LeafTable),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported page type: {other:#x}"),
            )),
        }
    }

    pub fn header_size(self) -> usize {
        match self {
            Self::InteriorIndex | Self::InteriorTable => 12,
            Self::LeafIndex | Self::LeafTable => 8,
        }
    }
}

pub struct PageHeader {
    pub kind: PageKind,
    pub cell_count: u16,
    pub right_most: Option<u32>,
    pub header_start: usize, // 0, or 100 on page 1; cell pointers are relative to byte 0
}

pub struct RawPage {
    bytes: Vec<u8>,
    pub header: PageHeader,
}

impl RawPage {
    pub fn parse(bytes: Vec<u8>, page_number: u32) -> io::Result<Self> {
        let header_start = if page_number == 1 { 100 } else { 0 };
        let h = bytes
            .get(header_start..header_start + 12)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Short page"))?;

        let kind = PageKind::from_byte(h[0])?;
        let cell_count = u16::from_be_bytes([h[3], h[4]]);
        let right_most = match kind {
            PageKind::InteriorTable | PageKind::InteriorIndex => {
                Some(u32::from_be_bytes([h[8], h[9], h[10], h[11]]))
            }
            _ => None,
        };
        let header = PageHeader {
            kind,
            cell_count,
            right_most,
            header_start,
        };
        Ok(Self { bytes, header })
    }

    fn cell_pointers(&self) -> io::Result<Vec<usize>> {
        let start = self.header.header_start + self.header.kind.header_size();
        let end = start + self.header.cell_count as usize * 2;
        let raw = self.bytes.get(start..end).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Cell pointer array out of bounds",
            )
        })?;
        Ok(raw
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
            .collect())
    }

    pub fn leaf_cells(&self) -> io::Result<Vec<Cell>> {
        if self.header.kind != PageKind::LeafTable {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected a leaf table page",
            ));
        }
        let pointers = self.cell_pointers()?;
        let mut cells = Vec::with_capacity(pointers.len());
        for p in pointers {
            let mut cell_bytes = self.bytes.get(p..).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Cell offset out of bounds")
            })?;
            cells.push(read_cell(&mut cell_bytes)?); // &[u8] implements Read
        }
        Ok(cells)
    }

    pub fn children(&self) -> io::Result<Vec<u32>> {
        let pointers = self.cell_pointers()?;
        let mut children = Vec::with_capacity(pointers.len() + 1);
        for p in pointers {
            let b = self.bytes.get(p..p + 4).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Child pointer out of bounds")
            })?;
            children.push(u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        }
        children.push(self.header.right_most.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Interior page without right-most pointer",
            )
        })?);
        Ok(children)
    }
    pub fn index_cells(&self) -> io::Result<Vec<IndexCell>> {
        let interior = match self.header.kind {
            PageKind::InteriorIndex => true,
            PageKind::LeafIndex => false,
            _ => return Err(invalid("Expected an index page")),
        };
        self.cell_pointers()?
            .into_iter()
            .map(|p| {
                let mut offset = p;
                let left_child = if interior {
                    let b = self
                        .bytes
                        .get(offset..offset + 4)
                        .ok_or_else(|| invalid("Short index cell"))?;
                    offset += 4;
                    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                } else {
                    None
                };
                let size = read_varint(&self.bytes, &mut offset)? as usize;
                let payload = self
                    .bytes
                    .get(offset..offset + size)
                    .ok_or_else(|| invalid("Index payload out of bounds"))?;
                Ok(IndexCell {
                    left_child,
                    values: read_record(payload)?.values,
                })
            })
            .collect()
    }

    /// (left child, key) per cell. Left child holds rowids <= key.
    pub fn table_interior_cells(&self) -> io::Result<Vec<(u32, i64)>> {
        self.cell_pointers()?
            .into_iter()
            .map(|p| {
                let b = self
                    .bytes
                    .get(p..p + 4)
                    .ok_or_else(|| invalid("Short interior cell"))?;
                let child = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                let mut offset = p + 4;
                Ok((child, read_varint(&self.bytes, &mut offset)? as i64))
            })
            .collect()
    }
}

pub enum Page {
    Leaf(Vec<Cell>),
    Interior(Vec<u32>), // children in key order, right-most last
}

pub struct IndexCell {
    pub left_child: Option<u32>, // Some on interior pages
    pub values: Vec<Value>,      // indexed columns..., rowid last
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}
