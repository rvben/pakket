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
    let track = item.get("track");

    let status_code = track
        .and_then(|t| t.get("e"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let carrier = track
        .and_then(|t| t.get("b"))
        .and_then(|v| v.as_i64())
        .map(|c| format!("Carrier {c}"))
        .unwrap_or_else(|| "Unknown".to_string());

    let latest = track.and_then(|t| t.get("z0"));
    let location = latest
        .and_then(|z| z.get("c"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let last_update = latest
        .and_then(|z| z.get("a"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let events: Vec<TrackingEvent> = track
        .and_then(|t| t.get("z1"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(TrackingEvent {
                        date: e.get("a")?.as_str()?.to_string(),
                        location: e.get("c")?.as_str()?.to_string(),
                        description: e.get("z")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(TrackingResult {
        carrier,
        status: map_status(status_code),
        eta: None,
        location,
        last_update,
        events,
    })
}

pub fn map_status(code: i64) -> TrackingStatus {
    match code {
        0 => TrackingStatus::Pending,
        30 | 35 | 40 | 45 => TrackingStatus::InTransit,
        50 => TrackingStatus::OutForDelivery,
        60 => TrackingStatus::Delivered,
        70 | 80 => TrackingStatus::Exception,
        _ => TrackingStatus::NotFound,
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
                    "number": "TEST123",
                    "track": {
                        "b": 1,
                        "e": 40,
                        "z0": {
                            "a": "2026-03-26 14:30",
                            "c": "Amsterdam, NL",
                            "z": "Arrived at sorting center"
                        },
                        "z1": [
                            {
                                "a": "2026-03-26 14:30",
                                "c": "Amsterdam, NL",
                                "z": "Arrived at sorting center"
                            },
                            {
                                "a": "2026-03-25 20:15",
                                "c": "Frankfurt, DE",
                                "z": "Departed facility"
                            }
                        ]
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
                "data": { "accepted": [{"number": "TEST123"}], "rejected": [] }
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
        let result = client.track("TEST123").await.unwrap();

        assert_eq!(result.status, TrackingStatus::InTransit);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.location, Some("Amsterdam, NL".to_string()));
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
    fn map_17track_status() {
        assert_eq!(map_status(0), TrackingStatus::Pending);
        assert_eq!(map_status(30), TrackingStatus::InTransit);
        assert_eq!(map_status(35), TrackingStatus::InTransit);
        assert_eq!(map_status(40), TrackingStatus::InTransit);
        assert_eq!(map_status(50), TrackingStatus::OutForDelivery);
        assert_eq!(map_status(60), TrackingStatus::Delivered);
        assert_eq!(map_status(70), TrackingStatus::Exception);
    }
}
