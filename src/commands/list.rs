use crate::commands::track::pad_colored;
use crate::output::{format_status, use_color, OutputConfig};
use crate::shipments::Shipment;
use owo_colors::OwoColorize;

pub fn print_list(output: &OutputConfig, shipments: &[Shipment], show_history: bool) {
    if output.json {
        println!(
            "{}",
            serde_json::to_string_pretty(shipments).expect("serialize")
        );
        return;
    }

    if shipments.is_empty() {
        eprintln!("No shipments saved. Use 'pakket add' to track a shipment.");
        return;
    }

    print_list_header();
    for s in shipments {
        println!("{}", format_list_row(s));
        if show_history && !s.cached_events.is_empty() {
            println!();
            println!(
                "{}",
                crate::commands::track::format_history(&s.cached_events)
            );
            println!();
        }
    }
}

fn print_list_header() {
    let header = format!(
        "{:<18}  {:<12}  {:<16}  {:<12}  {:<20}  {}",
        "NAME", "CARRIER", "STATUS", "ETA", "LOCATION", "LAST UPDATE"
    );
    if use_color() {
        eprintln!("{}", header.bold());
    } else {
        eprintln!("{header}");
    }
}

pub fn format_list_row(s: &Shipment) -> String {
    let status_padded = pad_colored(&format_status(&s.cached_status), 16);

    format!(
        "{:<18}  {:<12}  {}  {:<12}  {:<20}  {}",
        s.name,
        s.carrier,
        status_padded,
        s.cached_eta.as_deref().unwrap_or("-"),
        s.cached_location.as_deref().unwrap_or("-"),
        s.last_fetch
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "-".to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carriers::TrackingStatus;

    fn sample_shipment(name: &str, status: TrackingStatus) -> Shipment {
        Shipment {
            name: name.to_string(),
            tracking_number: "TEST123".to_string(),
            postcode: None,
            carrier: "DHL".to_string(),
            added_at: chrono::Utc::now(),
            delivered_at: None,
            last_fetch: Some(chrono::Utc::now()),
            cached_status: status,
            cached_eta: Some("2026-03-28".to_string()),
            cached_location: Some("Amsterdam".to_string()),
            cached_events: vec![],
        }
    }

    #[test]
    fn format_list_row_contains_name() {
        let s = sample_shipment("Monitor", TrackingStatus::InTransit);
        let row = format_list_row(&s);
        assert!(row.contains("Monitor"));
        assert!(row.contains("DHL"));
        assert!(row.contains("Amsterdam"));
    }

    #[test]
    fn format_list_row_no_location() {
        let mut s = sample_shipment("Keyboard", TrackingStatus::Pending);
        s.cached_location = None;
        let row = format_list_row(&s);
        assert!(row.contains("Keyboard"));
        assert!(row.contains("-"));
    }
}
