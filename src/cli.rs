use clap::{ArgAction, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "walrus")]
#[command(about = "Lightweight time tracking", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Clone, ValueEnum)]
pub enum Period {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Subcommand)]
pub enum Commands {
    Start { topic: Option<String> },
    Stop { topic: Option<String> },
    Show {
        #[arg(short = 'n', long, default_value = "1")]
        count: usize,
        #[arg(short = 'p', long)]
        period: Option<Period>,
        #[arg(short = 't', long, action = ArgAction::Append)]
        topic: Vec<String>,
    },
    List {
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
        #[arg(short = 't', long, action = ArgAction::Append)]
        topic: Vec<String>,
    },
    Topics,
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
    Drop { id: i64 },
    DropTopic { topic: String },
    Export {
        #[arg(short = 't', long, action = ArgAction::Append)]
        topic: Vec<String>,
        #[arg(short = 'p', long)]
        period: Option<Period>,
    },
    Reset,
}
