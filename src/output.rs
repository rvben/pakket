use owo_colors::OwoColorize;
use std::io::IsTerminal;

use crate::carriers::TrackingStatus;

#[derive(Clone, Copy)]
pub struct OutputConfig {
    pub json: bool,
}

impl OutputConfig {
    pub fn new(json_flag: bool) -> Self {
        let json = json_flag || !std::io::stdout().is_terminal();
        Self { json }
    }

    pub fn print_data(&self, data: &str) {
        println!("{data}");
    }

    pub fn print_message(&self, msg: &str) {
        eprintln!("{msg}");
    }
}

pub fn use_color() -> bool {
    std::io::stdout().is_terminal()
}

pub fn format_status(status: &TrackingStatus) -> String {
    let label = match status {
        TrackingStatus::Delivered => "Delivered",
        TrackingStatus::InTransit => "In Transit",
        TrackingStatus::OutForDelivery => "Out for dlv",
        TrackingStatus::Exception => "Exception",
        TrackingStatus::Pending => "Pending",
        TrackingStatus::NotFound => "Not Found",
    };

    if !use_color() {
        return label.to_string();
    }

    match status {
        TrackingStatus::Delivered => label.green().to_string(),
        TrackingStatus::InTransit => label.yellow().to_string(),
        TrackingStatus::OutForDelivery => label.cyan().to_string(),
        TrackingStatus::Exception => label.red().to_string(),
        TrackingStatus::Pending | TrackingStatus::NotFound => label.dimmed().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_flag_forces_json() {
        let output = OutputConfig::new(true);
        assert!(output.json);
    }

    #[test]
    fn status_color_delivered_is_green() {
        let s = TrackingStatus::Delivered;
        let colored = format_status(&s);
        assert!(colored.contains("Delivered"));
    }

    #[test]
    fn status_color_in_transit_is_yellow() {
        let s = TrackingStatus::InTransit;
        let colored = format_status(&s);
        assert!(colored.contains("In Transit"));
    }
}
