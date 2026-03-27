use reqwest::Client;

use super::{TrackingEvent, TrackingResult, TrackingStatus};
use crate::error::Error;

const BASE_URL: &str = "https://api-eu.dhl.com";

pub struct Dhl {
    api_key: String,
    client: Client,
    base_url: String,
}

impl Dhl {
    pub fn new(api_key: &str, base_url_override: Option<&str>) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: Client::new(),
            base_url: base_url_override.unwrap_or(BASE_URL).to_string(),
        }
    }

    pub async fn track(&self, tracking_number: &str) -> Result<TrackingResult, Error> {
        let url = format!(
            "{}/track/shipments?trackingNumber={}",
            self.base_url, tracking_number
        );

        let resp = self
            .client
            .get(&url)
            .header("DHL-API-Key", &self.api_key)
            .send()
            .await
            .map_err(Error::Http)?;

        if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            return Err(Error::Config("invalid DHL API key".to_string()));
        }

        if resp.status().as_u16() == 404 {
            return Ok(TrackingResult {
                carrier: "DHL".to_string(),
                status: TrackingStatus::NotFound,
                eta: None,
                location: None,
                last_update: None,
                events: vec![],
            });
        }

        if !resp.status().is_success() {
            return Err(Error::Api(format!("DHL API error: {}", resp.status())));
        }

        let data: serde_json::Value = resp.json().await.map_err(Error::Http)?;

        let shipments = data.get("shipments").and_then(|s| s.as_array());
        if let Some(items) = shipments
            && let Some(shipment) = items.first()
        {
            return parse_dhl_shipment(shipment);
        }

        Ok(TrackingResult {
            carrier: "DHL".to_string(),
            status: TrackingStatus::NotFound,
            eta: None,
            location: None,
            last_update: None,
            events: vec![],
        })
    }
}

fn parse_dhl_shipment(shipment: &serde_json::Value) -> Result<TrackingResult, Error> {
    let status_code = shipment
        .pointer("/status/statusCode")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let status = map_dhl_status(status_code);

    let location = shipment
        .pointer("/status/location/address/addressLocality")
        .and_then(|v| v.as_str())
        .map(String::from);

    let last_update = shipment
        .pointer("/status/timestamp")
        .and_then(|v| v.as_str())
        .map(String::from);

    let eta = shipment
        .pointer("/estimatedTimeOfDelivery")
        .and_then(|v| v.as_str())
        .map(String::from);

    let events: Vec<TrackingEvent> = shipment
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(TrackingEvent {
                        date: e.get("timestamp")?.as_str()?.to_string(),
                        location: e
                            .pointer("/location/address/addressLocality")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        description: e.get("description")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(TrackingResult {
        carrier: "DHL".to_string(),
        status,
        eta,
        location,
        last_update,
        events,
    })
}

fn map_dhl_status(code: &str) -> TrackingStatus {
    match code {
        "delivered" => TrackingStatus::Delivered,
        "transit" => TrackingStatus::InTransit,
        "out-for-delivery" => TrackingStatus::OutForDelivery,
        "failure" | "exception" => TrackingStatus::Exception,
        "pre-transit" => TrackingStatus::Pending,
        _ => TrackingStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_dhl_response() -> serde_json::Value {
        serde_json::json!({
            "shipments": [{
                "id": "00340434161094015063",
                "service": "express",
                "status": {
                    "timestamp": "2026-03-27T10:37:00",
                    "location": { "address": { "addressLocality": "Amsterdam, NL" } },
                    "statusCode": "transit",
                    "status": "IN TRANSIT",
                    "description": "Shipment in transit"
                },
                "events": [
                    {
                        "timestamp": "2026-03-27T10:37:00",
                        "location": { "address": { "addressLocality": "Amsterdam, NL" } },
                        "statusCode": "transit",
                        "description": "Arrived at sorting facility"
                    },
                    {
                        "timestamp": "2026-03-26T14:00:00",
                        "location": { "address": { "addressLocality": "Frankfurt, DE" } },
                        "statusCode": "transit",
                        "description": "Shipment picked up"
                    }
                ]
            }]
        })
    }

    #[tokio::test]
    async fn track_dhl_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(query_param("trackingNumber", "00340434161094015063"))
            .and(header("DHL-API-Key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_dhl_response()))
            .mount(&server)
            .await;

        let client = Dhl::new("test-key", Some(&server.uri()));
        let result = client.track("00340434161094015063").await.unwrap();

        assert_eq!(result.carrier, "DHL");
        assert_eq!(result.status, TrackingStatus::InTransit);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.location, Some("Amsterdam, NL".to_string()));
    }

    #[tokio::test]
    async fn track_dhl_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = Dhl::new("test-key", Some(&server.uri()));
        let result = client.track("INVALID").await.unwrap();
        assert_eq!(result.status, TrackingStatus::NotFound);
    }

    #[tokio::test]
    async fn track_dhl_invalid_key() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = Dhl::new("bad-key", Some(&server.uri()));
        let result = client.track("TEST").await;
        assert!(result.is_err());
    }

    #[test]
    fn map_dhl_statuses() {
        assert_eq!(map_dhl_status("delivered"), TrackingStatus::Delivered);
        assert_eq!(map_dhl_status("transit"), TrackingStatus::InTransit);
        assert_eq!(
            map_dhl_status("out-for-delivery"),
            TrackingStatus::OutForDelivery
        );
        assert_eq!(map_dhl_status("failure"), TrackingStatus::Exception);
        assert_eq!(map_dhl_status("pre-transit"), TrackingStatus::Pending);
        assert_eq!(map_dhl_status("unknown"), TrackingStatus::Pending);
    }
}
