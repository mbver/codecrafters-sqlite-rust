use std::cmp::Ordering;
use std::io;

use crate::database::Database;
use crate::sql::ast::{
    ComparisonOperator, Expression, Literal, Operand, SelectColumns, SelectStatement, Statement,
};
use crate::sql::parse_sql;
use anyhow::{Result, bail};

mod database;
mod record;
mod sql;

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
        _ => bail!("Missing or invalid command passed: {}", command),
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
    let filter = match select.where_clause {
        Some(expression) => Some(prepare_filter(database, &select.table_name, expression)?),
        None => None,
    };

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

            if !matches_comparison(&actual.to_string(), operator, expected) {
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
    expression: Expression,
) -> io::Result<(usize, ComparisonOperator, String)> {
    match expression {
        Expression::Comparison {
            left: Operand::Column(column_name),
            operator,
            right: Operand::Literal(literal),
        } => {
            let column_index = database.table_column_index(table_name, &column_name)?;

            let expected = match literal {
                Literal::String(value) => value,
                Literal::Number(value) => value,
                Literal::Null => "NULL".to_string(),
            };

            Ok((column_index, operator, expected))
        }

        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported WHERE expression",
        )),
    }
}

fn matches_comparison(actual: &str, operator: &ComparisonOperator, expected: &str) -> bool {
    let ordering = actual.cmp(expected);

    match operator {
        ComparisonOperator::Equal => ordering == Ordering::Equal,
        ComparisonOperator::NotEqual => ordering != Ordering::Equal,
        ComparisonOperator::LessThan => ordering == Ordering::Less,
        ComparisonOperator::LessThanOrEqual => ordering != Ordering::Greater,
        ComparisonOperator::GreaterThan => ordering == Ordering::Greater,
        ComparisonOperator::GreaterThanOrEqual => ordering != Ordering::Less,
    }
}
