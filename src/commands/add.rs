use chrono::Utc;

use crate::carriers::{TrackingResult, TrackingStatus};
use crate::shipments::Shipment;

pub fn create_shipment(name: &str, tracking_number: &str, result: &TrackingResult) -> Shipment {
    Shipment {
        name: name.to_string(),
        tracking_number: tracking_number.to_string(),
        carrier: result.carrier.clone(),
        added_at: Utc::now(),
        delivered_at: if result.status == TrackingStatus::Delivered {
            Some(Utc::now())
        } else {
            None
        },
        last_fetch: Some(Utc::now()),
        cached_status: result.status.clone(),
        cached_eta: result.eta.clone(),
        cached_location: result.location.clone(),
        cached_events: result.events.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_shipment_from_result() {
        let result = TrackingResult {
            carrier: "DHL".to_string(),
            status: TrackingStatus::InTransit,
            eta: None,
            location: Some("Amsterdam".to_string()),
            last_update: None,
            events: vec![],
        };
        let shipment = create_shipment("My package", "TEST123", &result);
        assert_eq!(shipment.name, "My package");
        assert_eq!(shipment.tracking_number, "TEST123");
        assert_eq!(shipment.carrier, "DHL");
        assert_eq!(shipment.cached_status, TrackingStatus::InTransit);
    }
}
