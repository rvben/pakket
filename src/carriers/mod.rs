pub mod dhl;
pub mod postnl;
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

/// Detected carrier based on tracking number pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedCarrier {
    PostNL,
    DHL,
    Unknown,
}

/// Auto-detect carrier from tracking number format.
///
/// PostNL barcodes start with 3S, LS, or RS.
/// DHL barcodes start with JD, JVGL, or GM, or are long numeric strings (10+ digits).
pub fn detect_carrier(tracking_number: &str) -> DetectedCarrier {
    let upper = tracking_number.to_uppercase();

    if upper.starts_with("3S") || upper.starts_with("LS") || upper.starts_with("RS") {
        return DetectedCarrier::PostNL;
    }

    if upper.starts_with("JD") || upper.starts_with("JVGL") || upper.starts_with("GM") {
        return DetectedCarrier::DHL;
    }

    if tracking_number.len() >= 10 && tracking_number.chars().all(|c| c.is_ascii_digit()) {
        return DetectedCarrier::DHL;
    }

    DetectedCarrier::Unknown
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
    fn detect_postnl_3s() {
        assert_eq!(detect_carrier("3SDEVC123456789"), DetectedCarrier::PostNL);
    }

    #[test]
    fn detect_postnl_ls() {
        assert_eq!(detect_carrier("LS123456789NL"), DetectedCarrier::PostNL);
    }

    #[test]
    fn detect_dhl_numeric() {
        assert_eq!(
            detect_carrier("00340434161094015063"),
            DetectedCarrier::DHL
        );
    }

    #[test]
    fn detect_dhl_jvgl() {
        assert_eq!(detect_carrier("JVGL1234567890"), DetectedCarrier::DHL);
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_carrier("ABCXYZ"), DetectedCarrier::Unknown);
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
