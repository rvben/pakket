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
        "{:<12}  {:<16}  {:<12}  {:<20}  {}",
        "CARRIER", "STATUS", "ETA", "LOCATION", "LAST UPDATE"
    );
    if use_color() {
        eprintln!("{}", header.bold());
    } else {
        eprintln!("{header}");
    }
}

pub fn format_table_row(result: &TrackingResult) -> String {
    let carrier = &result.carrier;
    let eta = result.eta.as_deref().unwrap_or("-");
    let location = result.location.as_deref().unwrap_or("-");
    let last_update = format_timestamp(result.last_update.as_deref().unwrap_or("-"));

    // Pad the status label before colorizing to avoid ANSI escapes breaking alignment
    let status_label = format_status(&result.status);
    let status_padded = pad_colored(&status_label, 16);

    format!(
        "{:<12}  {}  {:<12}  {:<20}  {}",
        carrier, status_padded, eta, location, last_update
    )
}

pub fn format_history(events: &[TrackingEvent]) -> String {
    let mut out = String::new();
    let header = format!("{:<22}  {:<20}  {}", "DATE", "LOCATION", "EVENT");
    if use_color() {
        out.push_str(&format!("{}\n", header.bold()));
    } else {
        out.push_str(&format!("{header}\n"));
    }
    for event in events {
        out.push_str(&format!(
            "{:<22}  {:<20}  {}\n",
            format_timestamp(&event.date),
            event.location,
            event.description
        ));
    }
    out.trim_end().to_string()
}

/// Shorten ISO timestamps to a readable format (2026-03-26 18:32)
fn format_timestamp(ts: &str) -> String {
    if ts == "-" {
        return "-".to_string();
    }
    // "2026-03-26T18:32:01+01:00" → "2026-03-26 18:32"
    ts.replace('T', " ")
        .chars()
        .take(16)
        .collect()
}

/// Pad a string that may contain ANSI escape codes to a visible width
pub fn pad_colored(s: &str, width: usize) -> String {
    let visible_len = strip_ansi_len(s);
    if visible_len >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - visible_len))
    }
}

/// Count visible characters (strip ANSI escape sequences)
fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carriers::{TrackingEvent, TrackingResult, TrackingStatus};

    fn sample_result() -> TrackingResult {
        TrackingResult {
            carrier: "DHL".to_string(),
            status: TrackingStatus::InTransit,
            eta: Some("2026-03-28".to_string()),
            location: Some("Amsterdam, NL".to_string()),
            last_update: Some("2026-03-26T14:30:00+01:00".to_string()),
            events: vec![
                TrackingEvent {
                    date: "2026-03-26T14:30:00+01:00".to_string(),
                    location: "Amsterdam, NL".to_string(),
                    description: "Arrived at sorting center".to_string(),
                },
                TrackingEvent {
                    date: "2026-03-25T20:15:00+01:00".to_string(),
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
        assert!(row.contains("2026-03-28"));
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

    #[test]
    fn format_timestamp_shortens_iso() {
        assert_eq!(format_timestamp("2026-03-26T18:32:01+01:00"), "2026-03-26 18:32");
    }

    #[test]
    fn format_timestamp_dash() {
        assert_eq!(format_timestamp("-"), "-");
    }

    #[test]
    fn strip_ansi_len_plain() {
        assert_eq!(strip_ansi_len("Delivered"), 9);
    }

    #[test]
    fn strip_ansi_len_colored() {
        let colored = "Delivered".green().to_string();
        assert_eq!(strip_ansi_len(&colored), 9);
    }

    #[test]
    fn pad_colored_works() {
        let plain = "test";
        assert_eq!(pad_colored(plain, 10), "test      ");
    }
}
