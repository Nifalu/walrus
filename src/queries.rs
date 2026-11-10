use rusqlite::{Connection, OptionalExtension, ToSql};
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

/// Read the (id, topic, start_time, end_time) columns of a sessions row.
/// Timestamps are stored as RFC 3339 strings; parse_session_row turns them
/// into datetimes.
fn read_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, String, Option<String>)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

/// Turn a raw session row into a Session, parsing the RFC 3339 timestamps.
fn parse_session_row((id, topic, start, end): (i64, String, String, Option<String>)) -> Result<Session> {
    Ok(Session {
        id,
        topic,
        start: DateTime::parse_from_rfc3339(&start)?,
        end: end.map(|e| DateTime::parse_from_rfc3339(&e)).transpose()?,
    })
}

/// Sessions shorter than this are dropped entirely when stopped.
const MIN_SESSION_SECS: f64 = 60.0;

/// Result of a stop operation: how many sessions were closed normally
/// (end_time set) and how many were dropped for being under one minute.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StopOutcome {
    pub stopped: usize,
    pub dropped: usize,
}

impl StopOutcome {
    pub fn total(&self) -> usize {
        self.stopped + self.dropped
    }
}

pub fn get_active_sessions(conn: &Connection) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(
        // end_time is selected for uniform row mapping; it is NULL by the
        // WHERE clause, so Session::end is always None here.
        "SELECT id, topic, start_time, end_time FROM sessions WHERE end_time IS NULL ORDER BY start_time DESC"
    )?;

    let sessions = stmt
        .query_map([], read_session_row)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(parse_session_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

/// Sessions with the most recent starts first, at most `limit` of them.
/// An empty `topics` slice means no topic filter.
pub fn get_sessions(conn: &Connection, limit: usize, topics: &[String]) -> Result<Vec<Session>> {
    let topic_clause = if topics.is_empty() {
        String::new()
    } else {
        format!("WHERE topic IN ({})", topic_placeholders(topics))
    };
    let limit_idx = topics.len() + 1;

    let mut stmt = conn.prepare(&format!(
        "SELECT id, topic, start_time, end_time
         FROM sessions
         {topic_clause}
         ORDER BY start_time DESC
         LIMIT ?{limit_idx}"
    ))?;

    let limit_i64 = limit as i64;
    let mut params = topic_params(topics);
    params.push(&limit_i64);

    let sessions = stmt
        .query_map(rusqlite::params_from_iter(params), read_session_row)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(parse_session_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

/// Finished sessions oldest first, for CSV export. An empty `topics` slice
/// means no topic filter.
pub fn get_all_sessions_for_export(conn: &Connection, topics: &[String]) -> Result<Vec<Session>> {
    let topic_clause = if topics.is_empty() {
        String::new()
    } else {
        format!(" AND topic IN ({})", topic_placeholders(topics))
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT id, topic, start_time, end_time
         FROM sessions
         WHERE end_time IS NOT NULL{topic_clause}
         ORDER BY start_time ASC"
    ))?;

    let sessions = stmt
        .query_map(rusqlite::params_from_iter(topic_params(topics)), read_session_row)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(parse_session_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

fn topic_placeholders(topics: &[String]) -> String {
    std::iter::repeat("?").take(topics.len()).collect::<Vec<_>>().join(", ")
}

fn topic_params<'a>(topics: &'a [String]) -> Vec<&'a dyn ToSql> {
    topics.iter().map(|topic| topic as &dyn ToSql).collect()
}

/// Convert a naive local datetime to an RFC 3339 string for SQL string
/// comparisons. Errors on DST-gap or ambiguous local times.
fn local_to_rfc3339(dt: NaiveDateTime, what: &str) -> Result<String> {
    let local = Local.from_local_datetime(&dt).single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous {} datetime", what))?;
    Ok(local.to_rfc3339())
}

/// Hours per topic for sessions starting in [start, end), most time first.
/// An empty `topics` slice means no topic filter.
pub fn get_period_stats(
    conn: &Connection,
    start: NaiveDateTime,
    end: NaiveDateTime,
    topics: &[String],
) -> Result<Vec<(String, f64)>> {
    let start_rfc3339 = local_to_rfc3339(start, "start")?;
    let end_rfc3339 = local_to_rfc3339(end, "end")?;

    let topic_clause = if topics.is_empty() {
        String::new()
    } else {
        format!(" AND topic IN ({})", topic_placeholders(topics))
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT topic, SUM((julianday(end_time) - julianday(start_time)) * 24) as hours
         FROM sessions
         WHERE end_time IS NOT NULL
           AND start_time >= ?1
           AND start_time < ?2{topic_clause}
         GROUP BY topic
         ORDER BY hours DESC"
    ))?;

    let mut params: Vec<&dyn ToSql> = vec![&start_rfc3339, &end_rfc3339];
    params.extend(topic_params(topics));

    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;

    let result = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(result)
}

pub fn get_all_topics(conn: &Connection) -> Result<Vec<(String, i64, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT topic, COUNT(*) as sessions,
                SUM(CASE WHEN end_time IS NOT NULL
                         THEN (julianday(end_time) - julianday(start_time)) * 24
                         ELSE 0.0 END) as hours
         FROM sessions
         GROUP BY topic
         ORDER BY hours DESC"
    )?;

    let topics = stmt.query_map([], |row| {
        let topic: String = row.get(0)?;
        let sessions: i64 = row.get(1)?;
        let hours: f64 = row.get(2)?;
        Ok((topic, sessions, hours))
    })?;

    let result = topics.collect::<Result<Vec<_>, _>>()?;

    Ok(result)
}

/// Insert a new open session. Atomic: check and insert happen in one
/// statement, so two racing walrus processes cannot both open the same topic
/// (one inserts, the other's NOT EXISTS sees it and inserts nothing). Returns
/// false if the topic was already active.
pub fn start_session(conn: &Connection, topic: &str) -> Result<bool> {
    let now = Local::now().to_rfc3339();
    let result = conn.execute(
        "INSERT INTO sessions (topic, start_time)
         SELECT ?1, ?2
         WHERE NOT EXISTS (
             SELECT 1 FROM sessions WHERE topic = ?1 AND end_time IS NULL
         )",
        [Some(topic), Some(&now)],
    );

    let inserted = match result {
        Ok(n) => n,
        // The partial unique index tripped in a race after all: another
        // process opened this topic between check and insert.
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::ConstraintViolation => 0,
        Err(e) => return Err(e.into()),
    };

    Ok(inserted > 0)
}

/// Close the open session with the given id. Sessions shorter than
/// MIN_SESSION_SECS are deleted instead of being saved with a tiny duration.
pub fn stop_session(conn: &Connection, id: i64) -> Result<StopOutcome> {
    let now = Local::now().to_rfc3339();
    // ?1 appears twice: SQLite numbered parameters are reused, so the same
    // 'now' timestamp decides the duration and becomes the end_time.
    let dropped = conn.execute(
        "DELETE FROM sessions
         WHERE id = ?1 AND end_time IS NULL
           AND (julianday(?2) - julianday(start_time)) * 86400.0 < ?3",
        rusqlite::params![id, now, MIN_SESSION_SECS],
    )?;
    let stopped = conn.execute(
        "UPDATE sessions SET end_time = ?1 WHERE id = ?2 AND end_time IS NULL",
        [&now, &id.to_string()],
    )?;
    Ok(StopOutcome { stopped, dropped })
}

/// Close every open session of a topic (idempotent), so duplicate open rows
/// cannot survive a stop. Sessions shorter than MIN_SESSION_SECS are deleted
/// instead of being saved with a tiny duration.
pub fn stop_all_open_sessions(conn: &Connection, topic: &str) -> Result<StopOutcome> {
    let now = Local::now().to_rfc3339();
    let dropped = conn.execute(
        "DELETE FROM sessions
         WHERE topic = ?1 AND end_time IS NULL
           AND (julianday(?2) - julianday(start_time)) * 86400.0 < ?3",
        rusqlite::params![topic, now, MIN_SESSION_SECS],
    )?;
    let stopped = conn.execute(
        "UPDATE sessions SET end_time = ?1 WHERE topic = ?2 AND end_time IS NULL",
        rusqlite::params![now, topic],
    )?;
    Ok(StopOutcome { stopped, dropped })
}

pub fn delete_all_sessions(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM sessions", [])?;
    Ok(())
}

pub fn delete_session(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

pub fn count_sessions_by_topic(conn: &Connection, topic: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE topic = ?1",
        [topic],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

pub fn delete_sessions_by_topic(conn: &Connection, topic: &str) -> Result<usize> {
    let rows = conn.execute("DELETE FROM sessions WHERE topic = ?1", [topic])?;
    Ok(rows)
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

pub fn get_sessions_with_calculated_hours(conn: &Connection, limit: usize) -> Result<Vec<(Session, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, topic, start_time, end_time,
                CASE WHEN end_time IS NOT NULL
                     THEN (julianday(end_time) - julianday(start_time)) * 24
                     ELSE 0.0 END as hours
         FROM sessions
         ORDER BY start_time DESC
         LIMIT ?1"
    )?;

    let sessions = stmt
        .query_map([limit], |row| {
            Ok((read_session_row(row)?, row.get::<_, f64>(4)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(raw, hours)| parse_session_row(raw).map(|s| (s, hours)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

pub fn get_sessions_with_calculated_hours_by_topics(conn: &Connection, limit: usize, topics: &[String]) -> Result<Vec<(Session, f64)>> {
    if topics.is_empty() {
        return get_sessions_with_calculated_hours(conn, limit);
    }

    let query = format!(
        "SELECT id, topic, start_time, end_time,
                CASE WHEN end_time IS NOT NULL
                     THEN (julianday(end_time) - julianday(start_time)) * 24
                     ELSE 0.0 END as hours
         FROM sessions
         WHERE topic IN ({})
         ORDER BY start_time DESC
         LIMIT ?{}",
        topic_placeholders(topics),
        topics.len() + 1
    );

    let mut stmt = conn.prepare(&query)?;
    let limit_i64 = limit as i64;
    let mut params = topic_params(topics);
    params.push(&limit_i64);

    let sessions = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok((read_session_row(row)?, row.get::<_, f64>(4)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(raw, hours)| parse_session_row(raw).map(|s| (s, hours)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

/// Total finished hours per topic, most time first. An empty `topics`
/// slice means no topic filter.
pub fn get_all_time_totals(conn: &Connection, topics: &[String]) -> Result<Vec<(String, f64)>> {
    let topic_clause = if topics.is_empty() {
        String::new()
    } else {
        format!(" AND topic IN ({})", topic_placeholders(topics))
    };

    let query = format!(
        "SELECT topic, SUM((julianday(end_time) - julianday(start_time)) * 24) as hours
         FROM sessions
         WHERE end_time IS NOT NULL{topic_clause}
         GROUP BY topic
         ORDER BY hours DESC"
    );
    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(topic_params(topics)), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;

    let result = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE sessions (
                id INTEGER PRIMARY KEY,
                topic TEXT,
                start_time TEXT NOT NULL,
                end_time TEXT
            )",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn get_period_stats_by_topics_filters_multiple_topics() {
        let conn = setup_conn();
        let s1 = Local.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).single().unwrap();
        let e1 = Local.with_ymd_and_hms(2026, 10, 1, 10, 0, 0).single().unwrap();
        let s2 = Local.with_ymd_and_hms(2026, 10, 1, 10, 0, 0).single().unwrap();
        let e2 = Local.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).single().unwrap();
        let s3 = Local.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).single().unwrap();
        let e3 = Local.with_ymd_and_hms(2026, 10, 1, 15, 0, 0).single().unwrap();
        conn.execute(
            "INSERT INTO sessions (topic, start_time, end_time) VALUES
             (?1, ?2, ?3),
             (?4, ?5, ?6),
             (?7, ?8, ?9)",
            rusqlite::params![
                "cvlt", s1.to_rfc3339(), e1.to_rfc3339(),
                "home", s2.to_rfc3339(), e2.to_rfc3339(),
                "work", s3.to_rfc3339(), e3.to_rfc3339(),
            ],
        ).unwrap();

        let start = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let topics = vec!["cvlt".to_string(), "home".to_string()];

        let stats = get_period_stats(&conn, start, end, &topics).unwrap();

        assert_eq!(stats.len(), 2);
        assert!(stats.iter().all(|(topic, _)| topic == "cvlt" || topic == "home"));
    }

    #[test]
    fn get_period_stats_with_empty_topics_returns_all_topics() {
        let conn = setup_conn();
        let s1 = Local.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).single().unwrap();
        let e1 = Local.with_ymd_and_hms(2026, 10, 1, 10, 0, 0).single().unwrap();
        let s2 = Local.with_ymd_and_hms(2026, 10, 1, 10, 0, 0).single().unwrap();
        let e2 = Local.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).single().unwrap();
        conn.execute(
            "INSERT INTO sessions (topic, start_time, end_time) VALUES (?1, ?2, ?3), (?4, ?5, ?6)",
            rusqlite::params![
                "cvlt", s1.to_rfc3339(), e1.to_rfc3339(),
                "work", s2.to_rfc3339(), e2.to_rfc3339(),
            ],
        ).unwrap();

        let start = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap().and_hms_opt(0, 0, 0).unwrap();

        // Empty filter must behave like "no filter", not "match nothing".
        let stats = get_period_stats(&conn, start, end, &[]).unwrap();

        assert_eq!(stats.len(), 2);
        assert!(stats.iter().all(|(topic, _)| topic == "cvlt" || topic == "work"));
    }

    #[test]
    fn get_sessions_with_calculated_hours_by_topics_filters_multiple_topics() {
        let conn = setup_conn();
        let s1 = Local.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).single().unwrap();
        let e1 = Local.with_ymd_and_hms(2026, 10, 1, 10, 0, 0).single().unwrap();
        let s2 = Local.with_ymd_and_hms(2026, 10, 2, 9, 0, 0).single().unwrap();
        let e2 = Local.with_ymd_and_hms(2026, 10, 2, 10, 30, 0).single().unwrap();
        let s3 = Local.with_ymd_and_hms(2026, 10, 3, 9, 0, 0).single().unwrap();
        let e3 = Local.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).single().unwrap();
        conn.execute(
            "INSERT INTO sessions (topic, start_time, end_time) VALUES
             (?1, ?2, ?3),
             (?4, ?5, ?6),
             (?7, ?8, ?9)",
            rusqlite::params![
                "cvlt", s1.to_rfc3339(), e1.to_rfc3339(),
                "home", s2.to_rfc3339(), e2.to_rfc3339(),
                "work", s3.to_rfc3339(), e3.to_rfc3339(),
            ],
        ).unwrap();

        let topics = vec!["cvlt".to_string(), "home".to_string()];
        let sessions = get_sessions_with_calculated_hours_by_topics(&conn, 10, &topics).unwrap();

        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().all(|(session, _)| session.topic == "cvlt" || session.topic == "home"));
    }
}