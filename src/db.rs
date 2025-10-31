use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub fn get_db_path() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .expect("Could not find local data directory")
        .join("walrus");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&data_dir).expect("Could not create data directory");

    data_dir.join("walrus.db")
}

pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path();
    let is_new = !db_path.exists();

    let conn = Connection::open(&db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY,
            topic TEXT,
            start_time TEXT NOT NULL,
            end_time TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS target (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            hours REAL NOT NULL,
            target_date TEXT
        )",
        [],
    )?;

    if is_new {
        println!("Database created at: {}", db_path.display());
    }
    Ok(conn)
}