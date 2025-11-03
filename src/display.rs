use crate::queries::{Session, PeriodStats};
use chrono::Local;

pub fn print_active_session(session: &Session) {
    let now = Local::now();
    let duration = now.signed_duration_since(session.start);
    let hours = duration.num_seconds() as f64 / 3600.0;
    println!("\nActive: {} ({:.2}h)\n", session.topic, hours);
}

pub fn print_sessions(sessions: &[Session], show_id: bool) {
    if show_id {
        println!("\n{:<5} {:<20} {:<20} {:<20} {:>10}", "ID", "Topic", "Start", "End", "Hours");
        println!("{}", "─".repeat(80));
    } else {
        println!("\n{:<20} {:<20} {:<20} {:>10}", "Topic", "Start", "End", "Hours");
        println!("{}", "─".repeat(75));
    }

    for session in sessions {
        if let Some(end) = session.end {
            let duration = end.signed_duration_since(session.start);
            let hours = duration.num_seconds() as f64 / 3600.0;

            if show_id {
                println!(
                    "{:<5} {:<20} {:<20} {:<20} {:>9.2}h",
                    session.id, session.topic,
                    session.start.format("%d.%m.%Y %H:%M"),
                    end.format("%d.%m.%Y %H:%M"),
                    hours
                );
            } else {
                println!(
                    "{:<20} {:<20} {:<20} {:>9.2}h",
                    session.topic,
                    session.start.format("%d.%m.%Y %H:%M"),
                    end.format("%d.%m.%Y %H:%M"),
                    hours
                );
            }
        } else if show_id {
            println!(
                "{:<5} {:<20} {:<20} {:<20} {:>10}",
                session.id, session.topic,
                session.start.format("%d.%m.%Y %H:%M"),
                "ACTIVE",
                "-"
            );
        }
    }

    println!();
}

pub fn print_sessions_with_hours(sessions_with_hours: &[(Session, f64)], show_id: bool) {
    if show_id {
        println!("\n{:<5} {:<20} {:<20} {:<20} {:>10}", "ID", "Topic", "Start", "End", "Hours");
        println!("{}", "─".repeat(80));
    } else {
        println!("\n{:<20} {:<20} {:<20} {:>10}", "Topic", "Start", "End", "Hours");
        println!("{}", "─".repeat(75));
    }

    for (session, hours) in sessions_with_hours {
        if let Some(end) = session.end {
            if show_id {
                println!(
                    "{:<5} {:<20} {:<20} {:<20} {:>9.2}h",
                    session.id, session.topic,
                    session.start.format("%d.%m.%Y %H:%M"),
                    end.format("%d.%m.%Y %H:%M"),
                    hours
                );
            } else {
                println!(
                    "{:<20} {:<20} {:<20} {:>9.2}h",
                    session.topic,
                    session.start.format("%d.%m.%Y %H:%M"),
                    end.format("%d.%m.%Y %H:%M"),
                    hours
                );
            }
        } else if show_id {
            println!(
                "{:<5} {:<20} {:<20} {:<20} {:>10}",
                session.id, session.topic,
                session.start.format("%d.%m.%Y %H:%M"),
                "ACTIVE",
                "-"
            );
        }
    }

    println!();
}

pub fn print_period_stats(stats: &[PeriodStats]) {
    println!();

    let mut grand_total: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

    for (i, period) in stats.iter().enumerate() {
        println!("{}", period.label);

        let mut total = 0.0;
        for (topic, hours) in &period.topics {
            total += hours;
            *grand_total.entry(topic.clone()).or_insert(0.0) += hours;
            println!("  {:<20} {:>8.2}h", topic, hours);
        }

        println!("  {}", "─".repeat(30));
        println!("  {:<20} {:>8.2}h", "Total", total);

        if i < stats.len() - 1 {
            println!();
        }
    }

    if stats.len() > 1 {
        println!("\n{}", "═".repeat(33));
        println!("Grand Total:");

        let mut sorted: Vec<_> = grand_total.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());

        let total: f64 = sorted.iter().map(|(_, h)| *h).sum();

        for (topic, hours) in sorted {
            println!("  {:<20} {:>8.2}h", topic, hours);
        }
        println!("  {}", "─".repeat(30));
        println!("  {:<20} {:>8.2}h", "Total", total);
    }

    println!();
}