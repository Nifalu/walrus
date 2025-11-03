use rusqlite::{Connection, OptionalExtension};
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Local, TimeZone};

pub struct Session {
    pub id: i64,
    pub topic: String,
    pub start: DateTime<chrono::FixedOffset>,
    pub end: Option<DateTime<chrono::FixedOffset>>,
}

pub struct PeriodStats {
    pub label: String,
    pub topics: Vec<(String, f64)>,
}

pub fn get_active_session(conn: &Connection) -> Result<Option<Session>> {
    let result = conn.query_row(
        "SELECT id, topic, start_time FROM sessions WHERE end_time IS NULL",
        [],
        |row| {
            let id: i64 = row.get(0)?;
            let topic: String = row.get(1)?;
            let start_str: String = row.get(2)?;
            Ok((id, topic, start_str))
        },
    ).optional()?;

    if let Some((id, topic, start_str)) = result {
        let start = DateTime::parse_from_rfc3339(&start_str)?;
        Ok(Some(Session { id, topic, start, end: None }))
    } else {
        Ok(None)
    }
}

pub fn get_sessions(conn: &Connection, limit: usize) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, topic, start_time, end_time
         FROM sessions
         ORDER BY start_time DESC
         LIMIT ?1"
    )?;

    let sessions = stmt.query_map([limit], |row| {
        let id: i64 = row.get(0)?;
        let topic: String = row.get(1)?;
        let start_str: String = row.get(2)?;
        let end_str: Option<String> = row.get(3)?;
        Ok((id, topic, start_str, end_str))
    })?;

    let mut result = Vec::new();
    for session in sessions {
        let (id, topic, start_str, end_str) = session?;
        let start = DateTime::parse_from_rfc3339(&start_str)?;
        let end = end_str.map(|s| DateTime::parse_from_rfc3339(&s)).transpose()?;
        result.push(Session { id, topic, start, end });
    }

    Ok(result)
}

pub fn get_all_sessions_for_export(conn: &Connection) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, topic, start_time, end_time
         FROM sessions
         WHERE end_time IS NOT NULL
         ORDER BY start_time ASC"
    )?;

    let sessions = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let topic: String = row.get(1)?;
        let start_str: String = row.get(2)?;
        let end_str: String = row.get(3)?;
        Ok((id, topic, start_str, end_str))
    })?;

    let mut result = Vec::new();
    for session in sessions {
        let (id, topic, start_str, end_str) = session?;
        let start = DateTime::parse_from_rfc3339(&start_str)?;
        let end = DateTime::parse_from_rfc3339(&end_str)?;
        result.push(Session { id, topic, start, end: Some(end) });
    }

    Ok(result)
}

pub fn get_period_stats(
    conn: &Connection,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<Vec<(String, f64)>> {
    // Convert NaiveDateTime to timezone-aware DateTime in RFC3339 format
    // to match the format stored in the database
    let start_dt = Local.from_local_datetime(&start).single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous start datetime"))?;
    let end_dt = Local.from_local_datetime(&end).single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous end datetime"))?;

    let start_rfc3339 = start_dt.to_rfc3339();
    let end_rfc3339 = end_dt.to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT topic, SUM((julianday(end_time) - julianday(start_time)) * 24) as hours
         FROM sessions
         WHERE end_time IS NOT NULL
           AND start_time >= ?1
           AND start_time < ?2
         GROUP BY topic
         ORDER BY hours DESC"
    )?;

    let topics = stmt.query_map(
        rusqlite::params![start_rfc3339, end_rfc3339],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    )?;

    let mut result = Vec::new();
    for topic in topics {
        result.push(topic?);
    }

    Ok(result)
}

pub fn start_session(conn: &Connection, topic: &str) -> Result<()> {
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sessions (topic, start_time) VALUES (?1, ?2)",
        [Some(topic), Some(&now)],
    )?;
    Ok(())
}

pub fn stop_session(conn: &Connection, id: i64) -> Result<()> {
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE sessions SET end_time = ?1 WHERE id = ?2",
        [&now, &id.to_string()],
    )?;
    Ok(())
}

pub fn delete_all_sessions(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM sessions", [])?;
    Ok(())
}

pub fn delete_session(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

pub fn session_exists(conn: &Connection, id: i64) -> Result<bool> {
    let exists: bool = conn.query_row(
        "SELECT 1 FROM sessions WHERE id = ?1",
        [id],
        |_| Ok(true),
    ).optional()?.unwrap_or(false);
    Ok(exists)
}

pub fn update_session_topic(conn: &Connection, id: i64, topic: &str) -> Result<()> {
    conn.execute("UPDATE sessions SET topic = ?1 WHERE id = ?2", rusqlite::params![topic, id])?;
    Ok(())
}

pub fn update_session_start(conn: &Connection, id: i64, start: &str) -> Result<()> {
    conn.execute("UPDATE sessions SET start_time = ?1 WHERE id = ?2", rusqlite::params![start, id])?;
    Ok(())
}

pub fn update_session_end(conn: &Connection, id: i64, end: &str) -> Result<()> {
    conn.execute("UPDATE sessions SET end_time = ?1 WHERE id = ?2", rusqlite::params![end, id])?;
    Ok(())
}

pub fn insert_session(conn: &Connection, topic: &str, start: &str, end: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (topic, start_time, end_time) VALUES (?1, ?2, ?3)",
        rusqlite::params![topic, start, end],
    )?;
    Ok(())
}

pub fn parse_datetime(s: &str) -> Result<String> {
    let dt = NaiveDateTime::parse_from_str(s, "%d.%m.%Y %H:%M")
        .map_err(|_| anyhow::anyhow!("Invalid datetime format. Use DD.MM.YYYY HH:MM"))?;

    let local_dt = Local.from_local_datetime(&dt).single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous datetime"))?;

    Ok(local_dt.to_rfc3339())
}