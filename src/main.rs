mod db;
mod commands;

use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "walrus")]
#[command(about = "A lightweight time tracking tool 🦭", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {

    Start {
        topic: Option<String>,
    },

    Stop,

    Show {
        #[arg(short = 'n', long, default_value = "3")]
        count: usize,

        #[arg(short = 'm', long)]
        month: bool,

        #[arg(short = 'w', long)]
        week: bool,
    },

    Export,

    Reset,

    Add {
        topic: String,

        #[arg(short = 's', long, value_name = "DD.MM.YYYY HH:MM")]
        start: String,

        #[arg(short = 'e', long, value_name = "DD.MM.YYYY HH:MM")]
        end: String,
    },

    List {
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },

    Delete {
        id: i64,
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = db::init_db()?;

    match cli.command {

        Commands::Start { topic } => {
            commands::start(&conn, topic)?;
        }
        Commands::Stop => {
            commands::stop(&conn)?;
        }
        Commands::Show { count, month, week } => {
            commands::show(&conn, count, month, week)?;
        }
        Commands::Export => {
            commands::export(&conn)?;
        }
        Commands::Reset => {
            commands::reset(&conn)?;
        }

        Commands::Add { topic, start, end } => {
            commands::add(&conn, topic, start, end)?;
        }
        Commands::List { count } => {
            commands::list(&conn, count)?;
        }
        Commands::Delete { id } => {
            commands::delete(&conn, id)?;
        }
        Commands::Edit { id, topic, start, end } => {
            commands::edit(&conn, id, topic, start, end)?;
        }
    }

    Ok(())
}