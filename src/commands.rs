use rusqlite::{Connection, OptionalExtension};
use anyhow::Result;
use chrono::{Local, NaiveDate, DateTime, Duration, Datelike, TimeZone};

pub fn start(conn: &Connection, topic: Option<String>) -> Result<()> {
    // Check if there's already an active session
    let active: Option<i64> = conn.query_row(
        "SELECT id FROM sessions WHERE end_time IS NULL",
        [],
        |row| row.get(0),
    ).optional()?;

    if let Some(_) = active {
        anyhow::bail!("Session already active! Stop it first with 'walrus stop'");
    }

    // Start new session
    let now = Local::now().to_rfc3339();
    let topic_value = topic.as_deref().unwrap_or("default");
    conn.execute(
        "INSERT INTO sessions (topic, start_time) VALUES (?1, ?2)",
        [Some(topic_value), Some(&now)],
    )?;

    match topic {
        Some(t) => println!("⏱️  Started tracking: {}", t),
        None => println!("⏱️  Started tracking"),
    }

    Ok(())
}

pub fn stop(conn: &Connection) -> Result<()> {
    // Find active session
    let active: Option<i64> = conn.query_row(
        "SELECT id FROM sessions WHERE end_time IS NULL",
        [],
        |row| row.get(0),
    ).optional()?;

    match active {
        Some(id) => {
            let now = Local::now().to_rfc3339();
            conn.execute(
                "UPDATE sessions SET end_time = ?1 WHERE id = ?2",
                [&now, &id.to_string()],
            )?;
            println!("Stopped tracking");
            show_recent_sessions(conn, 1)?;
            Ok(())
        }
        None => {
            anyhow::bail!("No active session to stop");
        }
    }
}

pub fn show(conn: &Connection, count: usize, month: bool, week: bool) -> Result<()> {
    // 1. Check for active session
    let active: Option<(i64, String, String)> = conn.query_row(
        "SELECT id, topic, start_time FROM sessions WHERE end_time IS NULL",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional()?;

    if let Some((_, topic, start_time)) = active {
        let start = DateTime::parse_from_rfc3339(&start_time)?;
        let now = Local::now();
        let duration = now.signed_duration_since(start);
        let hours = duration.num_seconds() as f64 / 3600.0;

        println!();
        println!("🟢 Active session: {} ({:.2}h)", topic, hours);

    }

    // 2. Show aggregated stats if -m or -w flag
    if month || week {
        show_period_stats(conn, count, month)?;
        println!();
    } else {
        show_recent_sessions(conn, count)?;
    }
    Ok(())
}

pub fn reset(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM sessions", [])?;
    println!("🗑️  All data cleared");
    Ok(())
}

pub fn export(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT topic, start_time, end_time
         FROM sessions
         WHERE end_time IS NOT NULL
         ORDER BY start_time ASC"
    )?;

    let sessions = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    // Create filename with timestamp
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("walrus_export_{}.csv", timestamp);

    // Write CSV
    let mut writer = std::fs::File::create(&filename)?;
    use std::io::Write;

    writeln!(writer, "start,end,duration (hours),topic")?;

    for session in sessions {
        let (topic, start_str, end_str) = session?;
        let start = DateTime::parse_from_rfc3339(&start_str)?;
        let end = DateTime::parse_from_rfc3339(&end_str)?;
        let duration = end.signed_duration_since(start);
        let hours = duration.num_seconds() as f64 / 3600.0;

        writeln!(
            writer,
            "{},{},{:.2},{}",
            start.format("%Y-%m-%d %H:%M:%S"),
            end.format("%Y-%m-%d %H:%M:%S"),
            hours,
            topic
        )?;
    }

    println!("📤 Exported to: {}", filename);

    Ok(())
}

pub fn add(conn: &Connection, topic: String, start: String, end: String) -> Result<()> {
    // Parse datetime strings (DD.MM.YYYY HH:MM format)
    let start_dt = parse_datetime(&start)?;
    let end_dt = parse_datetime(&end)?;

    if end_dt <= start_dt {
        anyhow::bail!("End time must be after start time");
    }

    conn.execute(
        "INSERT INTO sessions (topic, start_time, end_time) VALUES (?1, ?2, ?3)",
        rusqlite::params![topic, start_dt, end_dt],
    )?;

    let duration = (end_dt.parse::<DateTime<chrono::FixedOffset>>()?)
        .signed_duration_since(start_dt.parse::<DateTime<chrono::FixedOffset>>()?);
    let hours = duration.num_seconds() as f64 / 3600.0;

    println!("✅ Added session: {} ({:.2}h)", topic, hours);

    Ok(())
}

pub fn list(conn: &Connection, count: usize) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, topic, start_time, end_time
         FROM sessions
         ORDER BY start_time DESC
         LIMIT ?1"
    )?;

    let sessions = stmt.query_map([count], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    println!("{:<5} {:<20} {:<20} {:<20} {:>10}", "ID", "Topic", "Start", "End", "Duration");
    println!("{}", "─".repeat(80));

    for session in sessions {
        let (id, topic, start_str, end_str) = session?;
        let start = DateTime::parse_from_rfc3339(&start_str)?;

        if let Some(end_s) = end_str {
            let end = DateTime::parse_from_rfc3339(&end_s)?;
            let duration = end.signed_duration_since(start);
            let hours = duration.num_seconds() as f64 / 3600.0;

            println!(
                "{:<5} {:<20} {:<20} {:<20} {:>9.2}h",
                id,
                topic,
                start.format("%d.%m.%Y %H:%M"),
                end.format("%d.%m.%Y %H:%M"),
                hours
            );
        } else {
            println!(
                "{:<5} {:<20} {:<20} {:<20} {:>10}",
                id,
                topic,
                start.format("%d.%m.%Y %H:%M"),
                "ACTIVE",
                "-"
            );
        }
    }

    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let rows = conn.execute("DELETE FROM sessions WHERE id = ?1", [id])?;

    if rows == 0 {
        anyhow::bail!("Session with ID {} not found", id);
    }

    println!("🗑️  Deleted session {}", id);

    Ok(())
}

pub fn edit(conn: &Connection, id: i64, topic: Option<String>, start: Option<String>, end: Option<String>) -> Result<()> {
    // Check if session exists
    let exists: bool = conn.query_row(
        "SELECT 1 FROM sessions WHERE id = ?1",
        [id],
        |_| Ok(true),
    ).optional()?.unwrap_or(false);

    if !exists {
        anyhow::bail!("Session with ID {} not found", id);
    }

    // Update topic if provided
    if let Some(t) = topic {
        conn.execute("UPDATE sessions SET topic = ?1 WHERE id = ?2", rusqlite::params![t, id])?;
    }

    // Update start time if provided
    if let Some(s) = start {
        let start_dt = parse_datetime(&s)?;
        conn.execute("UPDATE sessions SET start_time = ?1 WHERE id = ?2", rusqlite::params![start_dt, id])?;
    }

    // Update end time if provided
    if let Some(e) = end {
        let end_dt = parse_datetime(&e)?;
        conn.execute("UPDATE sessions SET end_time = ?1 WHERE id = ?2", rusqlite::params![end_dt, id])?;
    }

    println!("✏️  Updated session {}", id);

    Ok(())
}

// Helper function to parse datetime
fn parse_datetime(s: &str) -> Result<String> {
    let dt = chrono::NaiveDateTime::parse_from_str(s, "%d.%m.%Y %H:%M")
        .map_err(|_| anyhow::anyhow!("Invalid datetime format. Use DD.MM.YYYY HH:MM"))?;

    // Convert to local timezone and RFC3339
    let local_dt = Local.from_local_datetime(&dt).single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous datetime"))?;

    Ok(local_dt.to_rfc3339())
}




fn show_recent_sessions(conn: &Connection, count: usize) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT topic, start_time, end_time
         FROM sessions
         WHERE end_time IS NOT NULL
         ORDER BY start_time DESC
         LIMIT ?1"
    )?;

    let sessions = stmt.query_map([count], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    println!();
    println!("{:<20} {:<20} {:<20} {:>10}", "Topic", "Start", "End", "Duration");
    println!("{}", "─".repeat(75));

    for session in sessions {
        let (topic, start_str, end_str) = session?;
        let start = DateTime::parse_from_rfc3339(&start_str)?;
        let end = DateTime::parse_from_rfc3339(&end_str)?;
        let duration = end.signed_duration_since(start);
        let hours = duration.num_seconds() as f64 / 3600.0;

        println!(
            "{:<20} {:<20} {:<20} {:>9.2}h",
            topic,
            start.format("%d.%m.%Y %H:%M"),
            end.format("%d.%m.%Y %H:%M"),
            hours
        );
    }

    Ok(())
}


fn show_period_stats(conn: &Connection, periods: usize, is_month: bool) -> Result<()> {
    let now = Local::now();
    let mut grand_total_by_topic: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    println!();
    for i in 0..periods {
        let (start, end, label) = if is_month {
            let months_back = i as i32;
            let target_date = if months_back == 0 {
                now.date_naive()
            } else {
                let year = now.year();
                let month = now.month() as i32;
                let new_month = ((month - 1 - months_back).rem_euclid(12)) + 1;
                let new_year = year + (month - 1 - months_back).div_euclid(12);
                NaiveDate::from_ymd_opt(new_year, new_month as u32, 1).unwrap()
            };

            let start = target_date.with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap();
            let end = if i == 0 {
                now.naive_local()
            } else {
                // Next month's first day
                let next_month = if target_date.month() == 12 {
                    NaiveDate::from_ymd_opt(target_date.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(target_date.year(), target_date.month() + 1, 1).unwrap()
                };
                next_month.and_hms_opt(0, 0, 0).unwrap()
            };

            (start, end, target_date.format("%B %Y").to_string())
        } else {
            let days_back = (i * 7) as i64;
            let week_start = (now - Duration::days(days_back + now.weekday().num_days_from_monday() as i64))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let week_end = if i == 0 {
                now.naive_local()
            } else {
                week_start + Duration::days(7)
            };

            let label = format!("Week {} ({} - {})",
                                week_start.format("%W"),
                                week_start.format("%d.%m"),
                                week_end.format("%d.%m.%Y")
            );


            (week_start, week_end, label)
        };

        // Get hours grouped by topic for this period
        let mut stmt = conn.prepare(
            "SELECT topic, SUM((julianday(end_time) - julianday(start_time)) * 24) as hours
             FROM sessions
             WHERE end_time IS NOT NULL
               AND start_time >= ?1
               AND start_time < ?2
             GROUP BY topic
             ORDER BY hours DESC"
        )?;

        let topics = stmt.query_map(rusqlite::params![start.to_string(), end.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;

        println!("📅 {}", label);

        let mut period_total = 0.0;
        for topic_result in topics {
            let (topic, hours) = topic_result?;
            period_total += hours;
            *grand_total_by_topic.entry(topic.clone()).or_insert(0.0) += hours;
            println!("   {:<20} {:>8.2}h", topic, hours);
        }

        println!("   {}", "─".repeat(30));
        println!("   {:<20} {:>8.2}h", "Total", period_total);

        if i < periods - 1 {
            println!();
        }
    }

    // Show grand total if multiple periods
    if periods > 1 {
        println!();
        println!("{}", "━".repeat(33));
        println!("🎯 Grand Total:");

        let mut sorted_topics: Vec<_> = grand_total_by_topic.iter().collect();
        sorted_topics.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());

        let overall_total: f64 = sorted_topics.iter().map(|(_, h)| *h).sum();

        for (topic, hours) in sorted_topics {
            println!("   {:<20} {:>8.2}h", topic, hours);
        }
        println!("   {}", "─".repeat(30));
        println!("   {:<20} {:>8.2}h", "Total", overall_total);
    }

    Ok(())
}