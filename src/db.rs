use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub fn get_db_path() -> PathBuf {
    PathBuf::from("walrus.db")
}

pub fn init_db() -> Result<Connection> {
    let conn = Connection::open(get_db_path())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (\
            id INTEGER PRIMARY KEY,\
            topic TEXT,\
            start_time TEXT NOT NULL,\
            end_time TEXT\
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS target (\
            id INTEGER PRIMARY KEY CHECK (id = 1),\
            hours REAL NOT NULL,\
            target_date TEXT\
        )",
        [],
    )?;

    Ok(conn)
}