use std::cmp::Ordering;
use std::io;

use crate::database::Database;
use crate::record::Value;
use crate::sql::ast::{
    ComparisonOperator, Expression, Operand, SelectColumns, SelectStatement, Statement,
};
use crate::sql::parse_sql;
use anyhow::{Result, bail};

mod database;
mod page;
mod record;
mod sql;

type PreparedFilter = (usize, ComparisonOperator, Value);

fn main() -> Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }
    let mut database = Database::open(&args[1])?;

    // Parse command and act accordingly
    let command = &args[2];
    match command.as_str() {
        ".dbinfo" => {
            println!("database page size: {}", database.page_size()?);
            println!("number of tables: {}", database.table_count()?);
        }
        ".tables" => {
            let table_names = database.table_names()?;

            for name in table_names {
                print!("{name} ");
            }

            println!();
        }
        query => match parse_sql(query)? {
            Statement::CreateTable(_) => bail!("create table is unsupported"),
            Statement::Select(select) => {
                execute_select(&mut database, select)?;
            }
        },
    }

    Ok(())
}

fn execute_select(database: &mut Database, select: SelectStatement) -> io::Result<()> {
    let cells = database.table_cells(&select.table_name)?;

    // Resolve selected columns.
    let column_indices = match select.columns {
        SelectColumns::Count => {
            println!("{}", database.table_row_count(&select.table_name)?);
            return Ok(());
        }

        SelectColumns::All => {
            let column_count = cells
                .first()
                .map(|cell| cell.record.values.len())
                .unwrap_or(0);

            (0..column_count).collect()
        }
        SelectColumns::Columns(column_names) => column_names
            .iter()
            .map(|name| database.table_column_index(&select.table_name, name))
            .collect::<io::Result<Vec<_>>>()?,
    };

    // Resolve the WHERE column once, not for every record.
    let filter = prepare_filter(database, &select.table_name, select.where_clause)?;
    for cell in cells {
        let record = &cell.record;

        // Apply WHERE before projecting selected columns.
        if let Some((column_index, operator, expected)) = &filter {
            let actual = record.values.get(*column_index).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "WHERE column index out of bounds",
                )
            })?;

            if !matches_comparison(&actual, operator, expected) {
                continue;
            }
        }

        let values = column_indices
            .iter()
            .map(|&index| {
                record
                    .values
                    .get(index)
                    .map(|value| value.to_string())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Selected column index out of bounds",
                        )
                    })
            })
            .collect::<io::Result<Vec<_>>>()?;

        println!("{}", values.join("|"));
    }

    Ok(())
}

fn prepare_filter(
    database: &mut Database,
    table_name: &str,
    where_clause: Option<Expression>,
) -> io::Result<Option<PreparedFilter>> {
    match where_clause {
        Some(Expression::Comparison {
            left: Operand::Column(column_name),
            operator,
            right: Operand::Literal(literal),
        }) => {
            let column_index = database.table_column_index(table_name, &column_name)?;

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
