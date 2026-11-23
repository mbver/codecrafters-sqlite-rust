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
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}
