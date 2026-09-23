use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::time::Duration;

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
    // Wait briefly instead of failing outright when another walrus process
    // (e.g. a tmux hook reconcile) currently holds the write lock.
    conn.busy_timeout(Duration::from_secs(5))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY,
            topic TEXT,
            start_time TEXT NOT NULL,
            end_time TEXT
        )",
        [],
    )?;

    // Migration: enforce at most one open (end_time IS NULL) session per
    // topic. Databases from before this guarantee can contain duplicate open
    // rows created by racing walrus processes: keep the newest per topic,
    // drop the rest, then enforce the invariant with a partial unique index.
    conn.execute(
        "DELETE FROM sessions
         WHERE end_time IS NULL
           AND id NOT IN (
               SELECT MAX(id) FROM sessions WHERE end_time IS NULL GROUP BY topic
           )",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_one_open_per_topic
         ON sessions(topic)
         WHERE end_time IS NULL",
        [],
    )?;

    if is_new {
        println!("Database created at: {}", db_path.display());
    }

    Ok(conn)
}