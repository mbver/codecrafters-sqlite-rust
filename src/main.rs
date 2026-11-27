use std::cmp::Ordering;
use std::io;
use std::sync::{Arc, Mutex};

use crate::database::Database;
use crate::record::{Cell, Value};
use crate::sql::ast::{
    ComparisonOperator, Expression, Operand, SelectColumns, SelectStatement, Statement,
};
use crate::sql::parse_sql;
use crate::table::Table;
use anyhow::{Result, bail};

mod database;
mod page;
mod record;
mod sql;
mod table;

type PreparedFilter = (usize, ComparisonOperator, Value);

fn main() -> Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }

    // Parse command and act accordingly
    let command = &args[2];
    let database = Arc::new(Mutex::new(Database::open(&args[1])?));

    match command.as_str() {
        ".dbinfo" => {
            println!("database page size: {}", database.lock().unwrap().page_size);
            println!(
                "number of tables: {}",
                database.lock().unwrap().table_count()
            );
        }
        ".tables" => {
            let table_names = database.lock().unwrap().table_names()?;

            for name in table_names {
                print!("{name} ");
            }

            println!();
        }
        query => match parse_sql(query)? {
            Statement::CreateTable(_) | Statement::CreateIndex(_) => {
                bail!("create statements are unsupported")
            }
            Statement::Select(select) => {
                execute_select(database.clone(), select)?;
            }
        },
    }

    Ok(())
}

fn execute_select(database: Arc<Mutex<Database>>, select: SelectStatement) -> io::Result<()> {
    let table = Table::load(database.clone(), &select.table_name)?;
    // Resolve selected columns.
    let column_indices = match select.columns {
        SelectColumns::Count => {
            println!("{}", table.row_count()?);
            return Ok(());
        }

        SelectColumns::All => (0..table.column_count()).collect(),
        SelectColumns::Columns(column_names) => column_names
            .iter()
            .map(|name| table.column_index(name))
            .collect::<io::Result<Vec<_>>>()?,
    };

    let filter = prepare_filter(&table, select.where_clause)?;

    // check if any index in the table match the where_clause column
    let indexed = match &filter {
        Some((column_index, ComparisonOperator::Equal, expected)) => {
            table
                .index_for(*column_index)
                .map(|index| (index, expected)) // carry expected value with index
        }
        _ => None,
    };
    // if there's a matching index, use index scan instead of table search
    if let Some((index, expected)) = indexed {
        for cell in table.scan_index(index, expected)? {
            print_row(&cell, &column_indices, table.rowid_column_index())?;
        }
        return Ok(());
    }
    // full table scan with a predicate
    let predicate = |cell: &Cell| -> io::Result<bool> {
        match &filter {
            None => Ok(true),
            Some((column_index, operator, expected)) => {
                // if the column is an alias to row_id, use row_id
                let actual = if table.rowid_column_index() == Some(*column_index) {
                    Value::Integer(cell.row_id as i64) // use your record value enum variant
                } else {
                    cell.record
                        .values
                        .get(*column_index)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "WHERE column index out of bounds",
                            )
                        })?
                        .clone()
                };
                Ok(matches_comparison(&actual, operator, expected))
            }
        }
    };
    let mut scan = table.scan(predicate);
    while let Some(cells) = scan.leaf_page_cells()? {
        for cell in cells {
            print_row(&cell, &column_indices, table.rowid_column_index())?;
        }
    }
    Ok(())
}

fn prepare_filter(
    table: &Table,
    where_clause: Option<Expression>,
) -> io::Result<Option<PreparedFilter>> {
    match where_clause {
        Some(Expression::Comparison {
            left: Operand::Column(column_name),
            operator,
            right: Operand::Literal(literal),
        }) => {
            let column_index = table.column_index(&column_name)?;

            let expected = Value::try_from(literal)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

            Ok(Some((column_index, operator, expected)))
        }

        None => Ok(None),

        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported WHERE expression",
        )),
    }
}

fn matches_comparison(actual: &Value, operator: &ComparisonOperator, expected: &Value) -> bool {
    let ordering = actual.compare(expected);

    match operator {
        ComparisonOperator::Equal => actual == expected,
        ComparisonOperator::NotEqual => actual != expected,
        ComparisonOperator::LessThan => ordering == Some(Ordering::Less),
        ComparisonOperator::LessThanOrEqual => {
            matches!(ordering, Some(Ordering::Less | Ordering::Equal))
        }
        ComparisonOperator::GreaterThan => ordering == Some(Ordering::Greater),
        ComparisonOperator::GreaterThanOrEqual => {
            matches!(ordering, Some(Ordering::Greater | Ordering::Equal))
        }
    }
}

fn print_row(cell: &Cell, column_indices: &[usize], rowid_col: Option<usize>) -> io::Result<()> {
    let values = column_indices
        .iter()
        .map(|&index| {
            if rowid_col == Some(index) {
                Ok(cell.row_id.to_string())
            } else {
                cell.record
                    .values
                    .get(index)
                    .map(|value| value.to_string())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Selected column index out of bounds",
                        )
                    })
            }
        })
        .collect::<io::Result<Vec<_>>>()?;

    println!("{}", values.join("|"));
    Ok(())
}
