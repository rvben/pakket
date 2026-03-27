use crate::carriers::TrackingEvent;
use crate::carriers::TrackingResult;
use crate::output::{OutputConfig, format_status, use_color};
use owo_colors::OwoColorize;

pub fn print_result(output: &OutputConfig, result: &TrackingResult, show_history: bool) {
    if output.json {
        println!(
            "{}",
            serde_json::to_string_pretty(result).expect("serialize")
        );
        return;
    }

    print_table_header();
    println!("{}", format_table_row(result));

    if show_history && !result.events.is_empty() {
        println!();
        println!("{}", format_history(&result.events));
    }
}

fn print_table_header() {
    let header = format!(
        "{:<10}{:<14}{:<9}{:<22}{}",
        "CARRIER", "STATUS", "ETA", "LOCATION", "LAST UPDATE"
    );
    if use_color() {
        eprintln!("{}", header.bold());
    } else {
        eprintln!("{header}");
    }
}

pub fn format_table_row(result: &TrackingResult) -> String {
    format!(
        "{:<10}{:<14}{:<9}{:<22}{}",
        result.carrier,
        format_status(&result.status),
        result.eta.as_deref().unwrap_or("-"),
        result.location.as_deref().unwrap_or("-"),
        result.last_update.as_deref().unwrap_or("-"),
    )
}

pub fn format_history(events: &[TrackingEvent]) -> String {
    let mut out = String::new();
    let header = format!("{:<18}{:<22}{}", "DATE", "LOCATION", "EVENT");
    if use_color() {
        out.push_str(&format!("{}\n", header.bold()));
    } else {
        out.push_str(&format!("{header}\n"));
    }
    for event in events {
        out.push_str(&format!(
            "{:<18}{:<22}{}\n",
            event.date, event.location, event.description
        ));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carriers::{TrackingEvent, TrackingResult, TrackingStatus};

    fn sample_result() -> TrackingResult {
        TrackingResult {
            carrier: "DHL".to_string(),
            status: TrackingStatus::InTransit,
            eta: Some("Mar 28".to_string()),
            location: Some("Amsterdam, NL".to_string()),
            last_update: Some("2026-03-26 14:30".to_string()),
            events: vec![
                TrackingEvent {
                    date: "2026-03-26 14:30".to_string(),
                    location: "Amsterdam, NL".to_string(),
                    description: "Arrived at sorting center".to_string(),
                },
                TrackingEvent {
                    date: "2026-03-25 20:15".to_string(),
                    location: "Frankfurt, DE".to_string(),
                    description: "Departed facility".to_string(),
                },
            ],
        }
    }

    #[test]
    fn format_table_row_contains_fields() {
        let result = sample_result();
        let row = format_table_row(&result);
        assert!(row.contains("DHL"));
        assert!(row.contains("Amsterdam, NL"));
        assert!(row.contains("Mar 28"));
    }

    #[test]
    fn format_history_has_all_events() {
        let result = sample_result();
        let history = format_history(&result.events);
        assert!(history.contains("Arrived at sorting center"));
        assert!(history.contains("Departed facility"));
    }

    #[test]
    fn format_json_output() {
        let result = sample_result();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("InTransit"));
        assert!(json.contains("DHL"));
    }
}
