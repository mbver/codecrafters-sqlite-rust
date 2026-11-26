use std::{cmp::Ordering, io};

use crate::{
    database::Database,
    page::{IndexCell, PageKind},
    record::{SchemaRecord, Value},
    sql::{ast::Statement, parse_sql},
};

// index.rs
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

    pub fn lookup(&self, db: &mut Database, key: &Value) -> io::Result<Vec<i64>> {
        let mut out = Vec::new();
        Self::search(db, self.root_page, key, &mut out)?;
        Ok(out)
    }

    fn search(db: &mut Database, page_no: u32, key: &Value, out: &mut Vec<i64>) -> io::Result<()> {
        let page = db.load_page(page_no)?;
        let cells = page.index_cells()?;

        let order = |c: &IndexCell| {
            c.values
                .first()
                .and_then(|v| v.compare(key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Incomparable index key"))
        };
        let rowid = |c: &IndexCell| match c.values.last() {
            Some(Value::Integer(r)) => Ok(*r),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Index entry missing rowid",
            )),
        };

        match page.header.kind {
            PageKind::LeafIndex => {
                for c in &cells {
                    match order(c)? {
                        Ordering::Less => continue,
                        Ordering::Equal => out.push(rowid(c)?),
                        Ordering::Greater => break,
                    }
                }
            }
            PageKind::InteriorIndex => {
                for c in &cells {
                    let child = c.left_child.expect("interior cell has left child");
                    match order(c)? {
                        Ordering::Less => continue,
                        Ordering::Equal => {
                            Self::search(db, child, key, out)?; // smaller rowids, same key
                            out.push(rowid(c)?); // the cell is itself a match
                        }
                        Ordering::Greater => {
                            // Left child may still hold matches; nothing after it can.
                            return Self::search(db, child, key, out);
                        }
                    }
                }
                let right = page
                    .header
                    .right_most
                    .expect("interior page has right-most");
                Self::search(db, right, key, out)?;
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
