use reqwest::Client;

use super::{TrackingEvent, TrackingResult, TrackingStatus};
use crate::error::Error;

const BASE_URL: &str = "https://jouw.postnl.nl";

pub struct PostNL {
    client: Client,
    base_url: String,
}

impl PostNL {
    pub fn new(base_url_override: Option<&str>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url_override.unwrap_or(BASE_URL).to_string(),
        }
    }

    pub async fn track_with_postcode(
        &self,
        barcode: &str,
        postcode: &str,
    ) -> Result<TrackingResult, Error> {
        let url = format!(
            "{}/track-and-trace/api/trackAndTrace/{}-NL-{}?language=en",
            self.base_url, barcode, postcode
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(Error::Http)?;

        if !resp.status().is_success() {
            return Err(Error::Api(format!("PostNL API error: {}", resp.status())));
        }

        let data: serde_json::Value = resp.json().await.map_err(Error::Http)?;

        let colli = data.get("colli").and_then(|c| c.as_object());
        if let Some(colli_map) = colli
            && let Some((_key, shipment)) = colli_map.iter().next()
        {
            return parse_postnl_shipment(shipment);
        }

        Ok(TrackingResult {
            carrier: "PostNL".to_string(),
            status: TrackingStatus::NotFound,
            eta: None,
            location: None,
            last_update: None,
            events: vec![],
        })
    }
}

fn parse_postnl_shipment(shipment: &serde_json::Value) -> Result<TrackingResult, Error> {
    let status_phase = shipment
        .pointer("/status/phase")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let status = map_postnl_status(status_phase);

    let eta = shipment
        .pointer("/delivery/deliveryDate")
        .and_then(|v| v.as_str())
        .map(String::from);

    let events: Vec<TrackingEvent> = shipment
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(TrackingEvent {
                        date: e.get("dateTime")?.as_str()?.to_string(),
                        location: e
                            .get("location")
                            .and_then(|l| l.as_str())
                            .unwrap_or("-")
                            .to_string(),
                        description: e.pointer("/status/description")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let location = events.first().map(|e| e.location.clone());
    let last_update = events.first().map(|e| e.date.clone());

    Ok(TrackingResult {
        carrier: "PostNL".to_string(),
        status,
        eta,
        location,
        last_update,
        events,
    })
}

/// PostNL status phases:
/// 1 = Announced, 2 = In transit, 3 = Out for delivery, 4 = Delivered, 99 = Exception
fn map_postnl_status(phase: i64) -> TrackingStatus {
    match phase {
        1 => TrackingStatus::Pending,
        2 => TrackingStatus::InTransit,
        3 => TrackingStatus::OutForDelivery,
        4 => TrackingStatus::Delivered,
        _ => TrackingStatus::Exception,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_postnl_response() -> serde_json::Value {
        serde_json::json!({
            "colli": {
                "3STEST123456789": {
                    "barcode": "3STEST123456789",
                    "status": { "phase": 2, "description": "In transit" },
                    "delivery": { "deliveryDate": "2026-03-28" },
                    "events": [
                        {
                            "status": { "description": "Arrived at sorting center" },
                            "dateTime": "2026-03-27T14:30:00",
                            "location": "Amsterdam"
                        },
                        {
                            "status": { "description": "Shipment picked up" },
                            "dateTime": "2026-03-26T10:00:00",
                            "location": "Rotterdam"
                        }
                    ]
                }
            },
            "processed": "2026-03-27T15:00:00+01:00"
        })
    }

    #[tokio::test]
    async fn track_postnl_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(r"/track-and-trace/api/trackAndTrace/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_postnl_response()))
            .mount(&server)
            .await;

        let client = PostNL::new(Some(&server.uri()));
        let result = client
            .track_with_postcode("3STEST123456789", "1234AB")
            .await
            .unwrap();

        assert_eq!(result.carrier, "PostNL");
        assert_eq!(result.status, TrackingStatus::InTransit);
        assert_eq!(result.eta, Some("2026-03-28".to_string()));
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.location, Some("Amsterdam".to_string()));
    }

    #[tokio::test]
    async fn track_postnl_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(r"/track-and-trace/api/trackAndTrace/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "colli": {},
                "processed": "2026-03-27T15:00:00+01:00"
            })))
            .mount(&server)
            .await;

        let client = PostNL::new(Some(&server.uri()));
        let result = client
            .track_with_postcode("INVALID", "1234AB")
            .await
            .unwrap();
        assert_eq!(result.status, TrackingStatus::NotFound);
    }

    #[test]
    fn map_postnl_phases() {
        assert_eq!(map_postnl_status(1), TrackingStatus::Pending);
        assert_eq!(map_postnl_status(2), TrackingStatus::InTransit);
        assert_eq!(map_postnl_status(3), TrackingStatus::OutForDelivery);
        assert_eq!(map_postnl_status(4), TrackingStatus::Delivered);
        assert_eq!(map_postnl_status(99), TrackingStatus::Exception);
    }
}
