use reqwest::Client;
use serde_json::json;

use super::{Carrier, TrackingEvent, TrackingResult, TrackingStatus};
use crate::error::Error;

const BASE_URL: &str = "https://api.17track.net";

pub struct SeventeenTrack {
    api_key: String,
    client: Client,
    base_url: String,
}

impl SeventeenTrack {
    pub fn new(api_key: &str, base_url_override: Option<&str>) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: Client::new(),
            base_url: base_url_override.unwrap_or(BASE_URL).to_string(),
        }
    }

    pub async fn validate_key(&self) -> Result<(), Error> {
        let url = format!("{}/track/v2.2/gettrackinfo", self.base_url);
        let body = json!([{"number": "0"}]);
        let resp = self
            .client
            .post(&url)
            .header("17token", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(Error::Http)?;

        if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            return Err(Error::Config("invalid API key".to_string()));
        }
        if !resp.status().is_success() {
            return Err(Error::Api(format!("17track API error: {}", resp.status())));
        }
        Ok(())
    }

    async fn register(&self, number: &str) -> Result<(), Error> {
        let url = format!("{}/track/v2.2/register", self.base_url);
        let body = json!([{"number": number}]);
        let resp = self
            .client
            .post(&url)
            .header("17token", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(Error::Http)?;

        if !resp.status().is_success() {
            return Err(Error::Api(format!(
                "17track register failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn get_track_info(&self, number: &str) -> Result<TrackingResult, Error> {
        let url = format!("{}/track/v2.2/gettrackinfo", self.base_url);
        let body = json!([{"number": number}]);
        let resp = self
            .client
            .post(&url)
            .header("17token", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(Error::Http)?;

        if !resp.status().is_success() {
            return Err(Error::Api(format!("17track API error: {}", resp.status())));
        }

        let data: serde_json::Value = resp.json().await.map_err(Error::Http)?;

        let accepted = data.pointer("/data/accepted").and_then(|v| v.as_array());

        if let Some(items) = accepted
            && let Some(item) = items.first()
        {
            return parse_track_item(item);
        }

        Ok(TrackingResult {
            carrier: "Unknown".to_string(),
            status: TrackingStatus::Pending,
            eta: None,
            location: None,
            last_update: None,
            events: vec![],
        })
    }
}

impl Carrier for SeventeenTrack {
    async fn track(&self, tracking_number: &str) -> Result<TrackingResult, Error> {
        self.register(tracking_number).await?;
        self.get_track_info(tracking_number).await
    }
}

fn parse_track_item(item: &serde_json::Value) -> Result<TrackingResult, Error> {
    let track_info = item.get("track_info");

    // Carrier name from provider
    let carrier = track_info
        .and_then(|t| t.pointer("/tracking/providers/0/provider/name"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    // Status from latest_status.status
    let status_str = track_info
        .and_then(|t| t.pointer("/latest_status/status"))
        .and_then(|v| v.as_str())
        .unwrap_or("NotFound");

    let status = map_status(status_str);

    // ETA from time_metrics.estimated_delivery_date
    let eta = track_info
        .and_then(|t| t.pointer("/time_metrics/estimated_delivery_date/from"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Latest event for location and last_update
    let location = track_info
        .and_then(|t| t.pointer("/latest_event/location"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let last_update = track_info
        .and_then(|t| t.pointer("/latest_event/time_iso"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Events from tracking.providers[0].events
    let events: Vec<TrackingEvent> = track_info
        .and_then(|t| t.pointer("/tracking/providers/0/events"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(TrackingEvent {
                        date: e.get("time_iso")?.as_str()?.to_string(),
                        location: e
                            .get("location")
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
        carrier,
        status,
        eta,
        location,
        last_update,
        events,
    })
}

fn map_status(status: &str) -> TrackingStatus {
    match status {
        "Delivered" => TrackingStatus::Delivered,
        "InTransit" => TrackingStatus::InTransit,
        "OutForDelivery" => TrackingStatus::OutForDelivery,
        "DeliveryFailure" | "Exception" | "Expired" => TrackingStatus::Exception,
        "InfoReceived" | "NotFound" => TrackingStatus::Pending,
        "AvailableForPickup" => TrackingStatus::OutForDelivery,
        _ => TrackingStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_gettrackinfo_response() -> serde_json::Value {
        serde_json::json!({
            "code": 0,
            "data": {
                "accepted": [{
                    "number": "CE010269106DE",
                    "carrier": 7041,
                    "track_info": {
                        "shipping_info": {
                            "shipper_address": { "country": "DE" },
                            "recipient_address": { "country": "NL" }
                        },
                        "latest_status": {
                            "status": "Delivered",
                            "sub_status": "Delivered_Other"
                        },
                        "latest_event": {
                            "time_iso": "2026-03-26T20:34:00+01:00",
                            "description": "The shipment has been successfully delivered",
                            "location": "NL"
                        },
                        "time_metrics": {
                            "days_of_transit": 2,
                            "estimated_delivery_date": { "from": null, "to": null }
                        },
                        "tracking": {
                            "providers": [{
                                "provider": {
                                    "key": 7041,
                                    "name": "DHL Paket",
                                    "country": "DE"
                                },
                                "events": [
                                    {
                                        "time_iso": "2026-03-26T20:34:00+01:00",
                                        "description": "The shipment has been successfully delivered",
                                        "location": "NL",
                                        "stage": "Delivered"
                                    },
                                    {
                                        "time_iso": "2026-03-26T17:11:00+01:00",
                                        "description": "The shipment has been loaded onto the delivery vehicle",
                                        "location": "NL",
                                        "stage": "OutForDelivery"
                                    },
                                    {
                                        "time_iso": "2026-03-25T12:54:00+01:00",
                                        "description": "The international shipment has been processed",
                                        "location": "DE"
                                    }
                                ]
                            }]
                        }
                    }
                }],
                "rejected": []
            }
        })
    }

    #[tokio::test]
    async fn track_returns_result() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/track/v2.2/register"))
            .and(header("17token", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": 0,
                "data": { "accepted": [{"number": "CE010269106DE"}], "rejected": [] }
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/track/v2.2/gettrackinfo"))
            .and(header("17token", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_gettrackinfo_response()))
            .expect(1)
            .mount(&server)
            .await;

        let client = SeventeenTrack::new("test-api-key", Some(&server.uri()));
        let result = client.track("CE010269106DE").await.unwrap();

        assert_eq!(result.carrier, "DHL Paket");
        assert_eq!(result.status, TrackingStatus::Delivered);
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.location, Some("NL".to_string()));
        assert!(result.last_update.unwrap().contains("2026-03-26"));
    }

    #[tokio::test]
    async fn validate_key_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/track/v2.2/gettrackinfo"))
            .and(header("17token", "good-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": 0,
                "data": { "accepted": [], "rejected": [] }
            })))
            .mount(&server)
            .await;

        let client = SeventeenTrack::new("good-key", Some(&server.uri()));
        assert!(client.validate_key().await.is_ok());
    }

    #[tokio::test]
    async fn validate_key_invalid() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/track/v2.2/gettrackinfo"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = SeventeenTrack::new("bad-key", Some(&server.uri()));
        assert!(client.validate_key().await.is_err());
    }

    #[tokio::test]
    async fn track_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/track/v2.2/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": 0,
                "data": { "accepted": [{"number": "INVALID"}], "rejected": [] }
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/track/v2.2/gettrackinfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "code": 0,
                "data": { "accepted": [], "rejected": [{"number": "INVALID"}] }
            })))
            .mount(&server)
            .await;

        let client = SeventeenTrack::new("test-api-key", Some(&server.uri()));
        let result = client.track("INVALID").await.unwrap();
        assert_eq!(result.status, TrackingStatus::Pending);
    }

    #[test]
    fn map_17track_status_codes() {
        assert_eq!(map_status("Delivered"), TrackingStatus::Delivered);
        assert_eq!(map_status("InTransit"), TrackingStatus::InTransit);
        assert_eq!(map_status("OutForDelivery"), TrackingStatus::OutForDelivery);
        assert_eq!(map_status("Exception"), TrackingStatus::Exception);
        assert_eq!(map_status("DeliveryFailure"), TrackingStatus::Exception);
        assert_eq!(map_status("InfoReceived"), TrackingStatus::Pending);
        assert_eq!(map_status("NotFound"), TrackingStatus::Pending);
        assert_eq!(map_status("AvailableForPickup"), TrackingStatus::OutForDelivery);
    }
}
