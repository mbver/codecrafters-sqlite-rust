use std::{cmp::Ordering, io};

use crate::{
    database::Database,
    page::PageKind,
    record::{SchemaRecord, Value},
    sql::{ast::Statement, parse_sql},
};

#[allow(dead_code)]
pub struct Index {
    pub name: String,
    pub root_page: u32,
    columns: Vec<String>,
}

impl Index {
    /// `Ok(None)` for rows that aren't usable indexes (non-index rows, or
    /// `sqlite_autoindex_*`, whose `sql` is NULL).
    pub fn from_schema(record: &SchemaRecord) -> io::Result<Option<Self>> {
        if record.record_type != "index" {
            return Ok(None);
        }
        let Some(sql) = record.sql.as_deref() else {
            return Ok(None);
        };
        let statement =
            parse_sql(sql).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let Statement::CreateIndex(create) = statement else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid create index statement",
            ));
        };
        Ok(Some(Self {
            name: record.name.clone(),
            root_page: record.rootpage as u32,
            columns: create.columns,
        }))
    }

    pub fn leading_column(&self) -> Option<&str> {
        self.columns.first().map(String::as_str)
    }

    // find all row_ids in the index that matches key (key_rowid_min to key_rowid_max)
    pub fn lookup(&self, db: &mut Database, key: &Value) -> io::Result<Vec<i64>> {
        let mut out = Vec::new();
        Self::search(db, self.root_page, key, &mut out)?;
        Ok(out)
    }
    // find all row_ids in an index page that holds key value
    fn search(db: &mut Database, page_no: u32, key: &Value, out: &mut Vec<i64>) -> io::Result<()> {
        let page = db.load_page(page_no)?;
        let cells = page.index_cells()?;

        match page.header.kind {
            PageKind::LeafIndex => {
                for c in &cells {
                    match c.compare(key)? {
                        Ordering::Less => continue,
                        Ordering::Equal => out.push(c.row_id()?),
                        Ordering::Greater => break,
                    }
                }
            }
            // find all index_cells that from key_rowid_min to key_rowid_max
            PageKind::InteriorIndex => {
                for c in &cells {
                    let left_child = c.left_child.expect("interior cell has left child");
                    match c.compare(key)? {
                        Ordering::Less => continue,
                        Ordering::Equal => {
                            Self::search(db, left_child, key, out)?; // smaller rowids, same key
                            out.push(c.row_id()?); // the cell is itself a match
                        }
                        Ordering::Greater => {
                            // Left child may still hold matches; nothing after it can.
                            return Self::search(db, left_child, key, out);
                        }
                    }
                }
                let right_child = page
                    .header
                    .right_most
                    .expect("interior page has right-most");
                Self::search(db, right_child, key, out)?;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Not an index page",
                ));
            }
        }
        Ok(())
    }
}
