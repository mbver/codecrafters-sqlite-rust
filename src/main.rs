use crate::database::Database;
use anyhow::{Result, bail};

mod database;
mod record;

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
        query if let Some(table_name) = count_query_table_name(query) => {
            println!("{}", database.table_row_count(table_name)?);
        }
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}

fn count_query_table_name(query: &str) -> Option<&str> {
    let query = query.trim();
    let table_name = strip_prefix_ignore_ascii_case(query, "SELECT COUNT(*) FROM ")?;
    let table_name = table_name.trim();
    if table_name.is_empty() {
        None
    } else {
        Some(table_name)
    }
}

fn strip_prefix_ignore_ascii_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len())
        .filter(|part| part.eq_ignore_ascii_case(prefix))?;

    s.get(prefix.len()..)
}
