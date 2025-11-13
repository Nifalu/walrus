mod db;
mod commands;
mod queries;
mod display;

use clap::{Parser, Subcommand, ValueEnum};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "walrus")]
#[command(about = "Lightweight time tracking", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
pub enum Period {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Subcommand)]
enum Commands {
    Start { topic: Option<String> },
    Stop { topic: Option<String> },
    Show {
        #[arg(short = 'n', long, default_value = "1")]
        count: usize,
        #[arg(short = 'p', long)]
        period: Option<Period>,
    },
    List {
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },
    Add {
        topic: String,
        #[arg(short = 's', long, value_name = "DD.MM.YYYY HH:MM")]
        start: String,
        #[arg(short = 'e', long, value_name = "DD.MM.YYYY HH:MM")]
        end: String,
    },
    Edit {
        id: i64,
        #[arg(short = 't', long)]
        topic: Option<String>,
        #[arg(short = 's', long, value_name = "DD.MM.YYYY HH:MM")]
        start: Option<String>,
        #[arg(short = 'e', long, value_name = "DD.MM.YYYY HH:MM")]
        end: Option<String>,
    },
    Delete { id: i64 },
    Export,
    Reset,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = db::init_db()?;

    match cli.command {
        Commands::Start { topic } => commands::start(&conn, topic)?,
        Commands::Stop { topic } => match topic {
            Some(t) => commands::stop_topic(&conn, &t)?,
            None => commands::stop(&conn)?,
        },
        Commands::Show { count, period } => commands::show(&conn, count, period)?,
        Commands::List { count } => commands::list(&conn, count)?,
        Commands::Add { topic, start, end } => commands::add(&conn, topic, start, end)?,
        Commands::Edit { id, topic, start, end } => commands::edit(&conn, id, topic, start, end)?,
        Commands::Delete { id } => commands::delete(&conn, id)?,
        Commands::Export => commands::export(&conn)?,
        Commands::Reset => commands::reset(&conn)?,
    }

    Ok(())
}