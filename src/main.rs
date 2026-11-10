mod cli;
mod db;
mod commands;
mod queries;
mod display;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = db::init_db()?;

    match cli.command {
        Commands::Start { topic } => commands::start(&conn, topic)?,
        Commands::Stop { topic } => match topic {
            Some(t) => commands::stop_topic(&conn, &t)?,
            None => commands::stop(&conn)?,
        },
        Commands::Show { count, period, topic } => commands::show(&conn, count, period, topic)?,
        Commands::List { count, topic } => commands::list(&conn, count, topic)?,
        Commands::Topics => commands::topics(&conn)?,
        Commands::Add { topic, start, end } => commands::add(&conn, topic, start, end)?,
        Commands::Edit { id, topic, start, end } => commands::edit(&conn, id, topic, start, end)?,
        Commands::Drop { id } => commands::drop(&conn, id)?,
        Commands::DropTopic { topic } => commands::drop_topic(&conn, &topic)?,
        Commands::Export { topic, period } => commands::export(&conn, topic, period)?,
        Commands::Reset => commands::reset(&conn)?,
    }

    Ok(())
}
