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

        let resp = self.client.get(&url).send().await.map_err(Error::Http)?;

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
    // Status from statusPhase.index
    let status_phase = shipment
        .pointer("/statusPhase/index")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let status = map_postnl_status(status_phase);

    // ETA from eta.start (time window)
    let eta = shipment
        .pointer("/eta/start")
        .and_then(|v| v.as_str())
        .or_else(|| shipment.get("deliveryDate").and_then(|v| v.as_str()))
        .map(|s| {
            // Extract just the date part if it's a full timestamp
            if let Some(t_pos) = s.find('T') {
                s[..t_pos].to_string()
            } else {
                s.to_string()
            }
        });

    // Events from observations array
    let events: Vec<TrackingEvent> = shipment
        .get("observations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(TrackingEvent {
                        date: e.get("observationDate")?.as_str()?.to_string(),
                        location: "-".to_string(),
                        description: e.get("description")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Location from recipient address
    let location = shipment
        .pointer("/recipient/address/town")
        .and_then(|v| v.as_str())
        .map(String::from);

    let last_update = shipment
        .get("lastObservation")
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(TrackingResult {
        carrier: "PostNL".to_string(),
        status,
        eta,
        location,
        last_update,
        events,
    })
}

/// PostNL statusPhase.index:
/// 1 = Announced, 2 = In transit, 3 = Out for delivery, 4 = Delivered
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
                    "statusPhase": { "index": 4, "message": "Shipment delivered" },
                    "isDelivered": true,
                    "deliveryDate": "2026-03-26T18:32:01+01:00",
                    "eta": {
                        "type": "Specific",
                        "start": "2026-03-26T17:45:00+01:00",
                        "end": "2026-03-26T18:45:00+01:00"
                    },
                    "recipient": {
                        "names": { "personName": "Test User" },
                        "address": {
                            "street": "Teststraat",
                            "houseNumber": "1",
                            "postalCode": "1234AB",
                            "town": "AMSTERDAM",
                            "country": "NL"
                        }
                    },
                    "lastObservation": "2026-03-26T18:32:01+01:00",
                    "observations": [
                        {
                            "observationDate": "2026-03-26T17:13:59+01:00",
                            "observationCode": "J05",
                            "description": "Driver is en route"
                        },
                        {
                            "observationDate": "2026-03-26T09:06:30+01:00",
                            "observationCode": "J01",
                            "description": "Shipment has been sorted"
                        },
                        {
                            "observationDate": "2026-03-26T09:05:40+01:00",
                            "observationCode": "B01",
                            "description": "Shipment received by PostNL"
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
        assert_eq!(result.status, TrackingStatus::Delivered);
        assert_eq!(result.eta, Some("2026-03-26".to_string()));
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.location, Some("AMSTERDAM".to_string()));
        assert!(result.last_update.is_some());
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
