use rusqlite::Connection;
use anyhow::Result;
use chrono::{Local, NaiveDate, Duration, Datelike};
use crate::{queries, display};
use crate::Period;

pub fn start(conn: &Connection, topic: Option<String>) -> Result<()> {
    if queries::get_active_session(conn)?.is_some() {
        anyhow::bail!("Session already active! Stop it first with 'walrus stop'");
    }

    let topic_value = topic.as_deref().unwrap_or("default");
    queries::start_session(conn, topic_value)?;

    match topic {
        Some(t) => println!("Started: {}", t),
        None => println!("Started tracking"),
    }

    Ok(())
}

pub fn stop(conn: &Connection) -> Result<()> {
    let active = queries::get_active_session(conn)?
        .ok_or_else(|| anyhow::anyhow!("No active session to stop"))?;

    queries::stop_session(conn, active.id)?;

    println!("Stopped tracking");
    let sessions = queries::get_sessions(conn, 1)?;
    display::print_sessions(&sessions, false);

    Ok(())
}

pub fn show(conn: &Connection, count: usize, period: Option<Period>) -> Result<()> {
    if let Some(active) = queries::get_active_session(conn)? {
        display::print_active_session(&active);
    }

    match period {
        Some(Period::Day) => show_days(conn, count)?,
        Some(Period::Week) => show_weeks(conn, count)?,
        Some(Period::Month) => show_months(conn, count)?,
        Some(Period::Year) => show_years(conn, count)?,
        None => {
            let sessions = queries::get_sessions(conn, count)?;
            display::print_sessions(&sessions, false);
        }
    }

    Ok(())
}

pub fn list(conn: &Connection, count: usize) -> Result<()> {
    let sessions = queries::get_sessions(conn, count)?;
    display::print_sessions(&sessions, true);
    Ok(())
}

pub fn reset(conn: &Connection) -> Result<()> {
    use std::io::{self, Write};

    println!("WARNING: This will delete ALL your time tracking data!");
    println!("This action cannot be undone.");
    print!("Type 'confirm' to proceed: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input != "confirm" {
        println!("Reset cancelled");
        return Ok(());
    }

    queries::delete_all_sessions(conn)?;
    println!("All data cleared");
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    if !queries::delete_session(conn, id)? {
        anyhow::bail!("Session with ID {} not found", id);
    }
    println!("Deleted session {}", id);
    Ok(())
}

pub fn export(conn: &Connection) -> Result<()> {
    let sessions = queries::get_all_sessions_for_export(conn)?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("walrus_export_{}.csv", timestamp);

    let mut writer = std::fs::File::create(&filename)?;
    use std::io::Write;

    writeln!(writer, "start,end,duration (hours),topic")?;

    for session in sessions {
        if let Some(end) = session.end {
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

pub fn add(conn: &Connection, topic: String, start: String, end: String) -> Result<()> {
    let start_dt = queries::parse_datetime(&start)?;
    let end_dt = queries::parse_datetime(&end)?;

    if end_dt <= start_dt {
        anyhow::bail!("End time must be after start time");
    }

    queries::insert_session(conn, &topic, &start_dt, &end_dt)?;

    let duration = end_dt.parse::<chrono::DateTime<chrono::FixedOffset>>()?
        .signed_duration_since(start_dt.parse::<chrono::DateTime<chrono::FixedOffset>>()?);
    let hours = duration.num_seconds() as f64 / 3600.0;

    println!("Added: {} ({:.2}h)", topic, hours);
    Ok(())
}

pub fn edit(conn: &Connection, id: i64, topic: Option<String>, start: Option<String>, end: Option<String>) -> Result<()> {
    if !queries::session_exists(conn, id)? {
        anyhow::bail!("Session with ID {} not found", id);
    }

    if let Some(t) = topic {
        queries::update_session_topic(conn, id, &t)?;
    }

    if let Some(s) = start {
        let start_dt = queries::parse_datetime(&s)?;
        queries::update_session_start(conn, id, &start_dt)?;
    }

    if let Some(e) = end {
        let end_dt = queries::parse_datetime(&e)?;
        queries::update_session_end(conn, id, &end_dt)?;
    }

    println!("Updated session {}", id);
    Ok(())
}

fn show_days(conn: &Connection, count: usize) -> Result<()> {
    let now = Local::now();
    let mut periods = Vec::new();

    for i in 0..count {
        let days_back = i as i64;
        let day_start = (now - Duration::days(days_back))
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let day_end = if i == 0 {
            now.naive_local()
        } else {
            day_start + Duration::days(1)
        };

        let label = if i == 0 {
            "Today".to_string()
        } else if i == 1 {
            "Yesterday".to_string()
        } else {
            day_start.format("%A, %d.%m.%Y").to_string()
        };

        let topics = queries::get_period_stats(conn, day_start, day_end)?;
        periods.push(queries::PeriodStats { label, topics });
    }

    display::print_period_stats(&periods);
    Ok(())
}

fn show_weeks(conn: &Connection, count: usize) -> Result<()> {
    let now = Local::now();
    let mut periods = Vec::new();

    for i in 0..count {
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
                            week_start.format("%V"),
                            week_start.format("%d.%m"),
                            week_end.format("%d.%m.%Y")
        );

        let topics = queries::get_period_stats(conn, week_start, week_end)?;
        periods.push(queries::PeriodStats { label, topics });
    }

    display::print_period_stats(&periods);
    Ok(())
}

fn show_months(conn: &Connection, count: usize) -> Result<()> {
    let now = Local::now();
    let mut periods = Vec::new();

    for i in 0..count {
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
            let next_month = if target_date.month() == 12 {
                NaiveDate::from_ymd_opt(target_date.year() + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(target_date.year(), target_date.month() + 1, 1).unwrap()
            };
            next_month.and_hms_opt(0, 0, 0).unwrap()
        };

        let label = target_date.format("%B %Y").to_string();
        let topics = queries::get_period_stats(conn, start, end)?;
        periods.push(queries::PeriodStats { label, topics });
    }

    display::print_period_stats(&periods);
    Ok(())
}

fn show_years(conn: &Connection, count: usize) -> Result<()> {
    let now = Local::now();
    let mut periods = Vec::new();

    for i in 0..count {
        let years_back = i as i32;
        let target_year = now.year() - years_back;

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

        let label = format!("{}", target_year);
        let topics = queries::get_period_stats(conn, start, end)?;
        periods.push(queries::PeriodStats { label, topics });
    }

    display::print_period_stats(&periods);
    Ok(())
}