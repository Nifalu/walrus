use rusqlite::Connection;
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone};
use crate::{queries, display};
use crate::cli::Period;

pub fn start(conn: &Connection, topic: Option<String>) -> Result<()> {
    let topic_value = topic.as_deref().unwrap_or("default");

    // Atomic: refuses to open a second session for an already-active topic,
    // even when two walrus processes race (see queries::start_session).
    if !queries::start_session(conn, topic_value)? {
        anyhow::bail!("Session for '{}' is already active! Stop it first with 'walrus stop {}'", topic_value, topic_value);
    }

    match topic {
        Some(t) => println!("Started: {}", t),
        None => println!("Started tracking"),
    }

    Ok(())
}

pub fn stop(conn: &Connection) -> Result<()> {
    let active_sessions = queries::get_active_sessions(conn)?;

    if active_sessions.is_empty() {
        anyhow::bail!("No active session to stop");
    } else if active_sessions.len() > 1 {
        // Multiple active sessions - user must specify which one to stop
        println!("Multiple active sessions found:");
        for session in &active_sessions {
            println!("  {} - {}", session.id, session.topic);
        }
        anyhow::bail!("Please specify which session to stop using: walrus stop <topic>");
    } else {
        // Exactly one active session - stop it
        let outcome = queries::stop_session(conn, active_sessions[0].id)?;

        if outcome.dropped > 0 {
            println!("Session under 1 minute - dropped");
            return Ok(());
        }

        println!("Stopped tracking");
        let sessions = queries::get_sessions(conn, 1, &[])?;
        display::print_sessions(&sessions, false);

        Ok(())
    }
}

pub fn stop_topic(conn: &Connection, topic: &str) -> Result<()> {
    // Close ALL open sessions of the topic: duplicates from earlier races or
    // manual starts must not survive a stop.
    let outcome = queries::stop_all_open_sessions(conn, topic)?;

    if outcome.total() == 0 {
        anyhow::bail!("No active session for '{}' to stop", topic);
    }

    println!("Stopped tracking");
    if outcome.stopped > 1 {
        println!("({} sessions)", outcome.stopped);
    }
    if outcome.dropped > 0 {
        println!("(dropped {} session{} under 1 minute)", outcome.dropped, if outcome.dropped > 1 { "s" } else { "" });
    }
    let sessions = queries::get_sessions(conn, 1, &[])?;
    display::print_sessions(&sessions, false);

    Ok(())
}

pub fn show(conn: &Connection, count: usize, period: Option<Period>, topic: Vec<String>) -> Result<()> {
    // List every open session: several can legitimately be active at once
    // (e.g. one per attached tmux session).
    display::print_active_sessions(&queries::get_active_sessions(conn)?);

    match period {
        Some(period) => show_periods(conn, count, &topic, period)?,
        None => {
            if topic.is_empty() {
                let sessions = queries::get_sessions(conn, count, &topic)?;
                display::print_sessions(&sessions, false);
            } else {
                let totals = queries::get_all_time_totals(conn, &topic)?;
                let grand_total: f64 = totals.iter().map(|(_, h)| *h).sum();
                println!("\nAll time");
                for (topic_name, hours) in totals {
                    println!("  {:<20} {:>8.2}h", topic_name, hours);
                }
                println!("  {}", "─".repeat(30));
                println!("  {:<20} {:>8.2}h", "Total", grand_total);
                println!();
            }
        }
    }

    Ok(())
}

pub fn list(conn: &Connection, count: usize, topic: Vec<String>) -> Result<()> {
    let sessions = queries::get_sessions(conn, count, &topic)?;
    display::print_sessions(&sessions, true);
    Ok(())
}

pub fn topics(conn: &Connection) -> Result<()> {
    let topics = queries::get_all_topics(conn)?;
    display::print_topics(&topics);
    Ok(())
}

/// Prompt the user to type 'confirm'; returns true if they did.
fn confirm() -> Result<bool> {
    use std::io::{self, Write};

    print!("Type 'confirm' to proceed: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim() == "confirm")
}

pub fn reset(conn: &Connection) -> Result<()> {
    println!("WARNING: This will delete ALL your time tracking data!");
    println!("This action cannot be undone.");

    if !confirm()? {
        println!("Reset cancelled");
        return Ok(());
    }

    queries::delete_all_sessions(conn)?;
    println!("All data cleared");
    Ok(())
}

pub fn drop(conn: &Connection, id: i64) -> Result<()> {
    if !queries::delete_session(conn, id)? {
        anyhow::bail!("Session with ID {} not found", id);
    }
    println!("Dropped session {}", id);
    Ok(())
}

pub fn drop_topic(conn: &Connection, topic: &str) -> Result<()> {
    let count = queries::count_sessions_by_topic(conn, topic)?;
    if count == 0 {
        anyhow::bail!("No sessions found for topic '{}'", topic);
    }

    println!("This will delete {} session(s) for topic '{}'.", count, topic);

    if !confirm()? {
        println!("Cancelled");
        return Ok(());
    }

    let deleted = queries::delete_sessions_by_topic(conn, topic)?;
    println!("Dropped {} session(s) for topic '{}'", deleted, topic);
    Ok(())
}

pub fn export(conn: &Connection, topic_filter: Vec<String>, period: Option<Period>) -> Result<()> {
    let sessions = queries::get_all_sessions_for_export(conn, &topic_filter)?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("walrus_export_{}.csv", timestamp);

    let mut writer = std::fs::File::create(&filename)?;
    use std::io::Write;

    writeln!(writer, "start,end,duration (hours),topic")?;

    let range = period.map(compute_period_range).transpose()?;

    for session in sessions {
        if let Some(end) = session.end {
            if let Some((rs, re)) = &range {
                if session.start < *rs || session.start >= *re {
                    continue;
                }
            }

            let duration = end.signed_duration_since(session.start);
            let hours = duration.num_seconds() as f64 / 3600.0;

            writeln!(
                writer,
                "{},{},{:.2},{}",
                session.start.format("%Y-%m-%d %H:%M:%S"),
                end.format("%Y-%m-%d %H:%M:%S"),
                hours,
                session.topic
            )?;
        }
    }

    println!("Exported to: {}", filename);
    Ok(())
}

/// Bounds of the current `period` for export filtering. Export filters rows
/// in Rust rather than in SQL (unlike show, whose period queries filter in
/// get_period_stats); the bounds are the same ones show_periods uses for
/// i = 0. Returns aware datetimes for direct comparison with Session
/// timestamps, erroring on DST-gap or ambiguous local times instead of
/// panicking.
fn compute_period_range(period: Period) -> Result<(chrono::DateTime<chrono::FixedOffset>, chrono::DateTime<chrono::FixedOffset>)> {
    let now = Local::now();
    let (_, start, _) = period_bounds(period, &now, 0);

    let start = Local
        .from_local_datetime(&start)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Ambiguous start datetime"))?;

    Ok((start.into(), now.into()))
}

pub fn add(conn: &Connection, topic: String, start: String, end: String) -> Result<()> {
    let start_dt = queries::parse_datetime(&start)?;
    let end_dt = queries::parse_datetime(&end)?;

    if end_dt <= start_dt {
        anyhow::bail!("End time must be after start time");
    }

    queries::insert_session(conn, &topic, start_dt, end_dt)?;

    let hours = end_dt.signed_duration_since(start_dt).num_seconds() as f64 / 3600.0;

    println!("Added: {} ({:.2}h)", topic, hours);
    Ok(())
}

pub fn edit(conn: &Connection, id: i64, topic: Option<String>, start: Option<String>, end: Option<String>) -> Result<()> {
    if !queries::session_exists(conn, id)? {
        anyhow::bail!("Session with ID {} not found", id);
    }

    // Parse everything first so an invalid value aborts before any column
    // is written, instead of leaving a half-updated session behind.
    let start_dt = start.map(|s| queries::parse_datetime(&s)).transpose()?;
    let end_dt = end.map(|e| queries::parse_datetime(&e)).transpose()?;

    if let (Some(start), Some(end)) = (&start_dt, &end_dt) {
        if end <= start {
            anyhow::bail!("End time must be after start time");
        }
    }

    if let Some(t) = topic {
        queries::update_session_topic(conn, id, &t)?;
    }

    if let Some(start) = start_dt {
        queries::update_session_start(conn, id, start)?;
    }

    if let Some(end) = end_dt {
        queries::update_session_end(conn, id, end)?;
    }

    println!("Updated session {}", id);
    Ok(())
}

/// Fetch and print `count` consecutive period stats (days, weeks, ...).
fn show_periods(conn: &Connection, count: usize, topics: &[String], period: Period) -> Result<()> {
    let now = Local::now();
    let mut periods = Vec::new();

    for i in 0..count {
        let (label, start, end) = period_bounds(period, &now, i);
        let topics = queries::get_period_stats(conn, start, end, topics)?;
        periods.push(queries::PeriodStats { label, topics });
    }

    display::print_period_stats(&periods);
    Ok(())
}

/// Label and [start, end) bounds of the `i`-th `period` counting back from
/// `now` (i = 0 is the current period, which ends at `now`).
///
/// The remaining `unwrap()`s are on calendar construction with arguments
/// that are valid by construction (midnight, day 1, the div_euclid month
/// arithmetic); they cannot fail.
fn period_bounds(period: Period, now: &DateTime<Local>, i: usize) -> (String, NaiveDateTime, NaiveDateTime) {
    match period {
        Period::Day => {
            let day_start = (*now - Duration::days(i as i64))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let day_end = if i == 0 {
                now.naive_local()
            } else {
                day_start + Duration::days(1)
            };

            let label = match i {
                0 => "Today".to_string(),
                1 => "Yesterday".to_string(),
                _ => day_start.format("%A, %d.%m.%Y").to_string(),
            };

            (label, day_start, day_end)
        }
        Period::Week => {
            let days_back = (i * 7) as i64;
            let week_start = (*now - Duration::days(days_back + now.weekday().num_days_from_monday() as i64))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let week_end = if i == 0 {
                now.naive_local()
            } else {
                week_start + Duration::days(7)
            };

            let label = format!("Week {} ({} - {})",
                                week_start.format("%V"),
                                week_start.format("%d.%m"),
                                week_end.format("%d.%m.%Y")
            );

            (label, week_start, week_end)
        }
        Period::Month => {
            let months_back = i as i32;
            let month = now.month() as i32;
            let year = now.year();
            let first_of_month = NaiveDate::from_ymd_opt(
                year + (month - 1 - months_back).div_euclid(12),
                (((month - 1 - months_back).rem_euclid(12)) + 1) as u32,
                1,
            )
            .unwrap();

            let start = first_of_month.and_hms_opt(0, 0, 0).unwrap();
            let end = if i == 0 {
                now.naive_local()
            } else {
                let next_month = if first_of_month.month() == 12 {
                    NaiveDate::from_ymd_opt(first_of_month.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(first_of_month.year(), first_of_month.month() + 1, 1).unwrap()
                };
                next_month.and_hms_opt(0, 0, 0).unwrap()
            };

            (first_of_month.format("%B %Y").to_string(), start, end)
        }
        Period::Year => {
            let target_year = now.year() - i as i32;

            let start = NaiveDate::from_ymd_opt(target_year, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();

            let end = if i == 0 {
                now.naive_local()
            } else {
                NaiveDate::from_ymd_opt(target_year + 1, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            };

            (format!("{}", target_year), start, end)
        }
    }
}
