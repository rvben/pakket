pub mod seventeen;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackingStatus {
    Delivered,
    InTransit,
    OutForDelivery,
    Exception,
    Pending,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingEvent {
    pub date: String,
    pub location: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingResult {
    pub carrier: String,
    pub status: TrackingStatus,
    pub eta: Option<String>,
    pub location: Option<String>,
    pub last_update: Option<String>,
    pub events: Vec<TrackingEvent>,
}

pub trait Carrier: Send + Sync {
    fn track(
        &self,
        tracking_number: &str,
    ) -> impl std::future::Future<Output = Result<TrackingResult, Error>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_status_serialize() {
        let status = TrackingStatus::InTransit;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"InTransit\"");
    }

    #[test]
    fn tracking_status_deserialize() {
        let status: TrackingStatus = serde_json::from_str("\"Delivered\"").unwrap();
        assert_eq!(status, TrackingStatus::Delivered);
    }

    #[test]
    fn tracking_result_has_all_fields() {
        let result = TrackingResult {
            carrier: "DHL".to_string(),
            status: TrackingStatus::InTransit,
            eta: Some("2026-03-28".to_string()),
            location: Some("Amsterdam, NL".to_string()),
            last_update: Some("2026-03-26T14:30:00Z".to_string()),
            events: vec![TrackingEvent {
                date: "2026-03-26T14:30:00Z".to_string(),
                location: "Amsterdam, NL".to_string(),
                description: "Arrived at sorting center".to_string(),
            }],
        };
        assert_eq!(result.carrier, "DHL");
        assert_eq!(result.events.len(), 1);
    }
}
