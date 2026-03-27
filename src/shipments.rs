use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::carriers::{TrackingEvent, TrackingStatus};
use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shipment {
    pub name: String,
    pub tracking_number: String,
    pub postcode: Option<String>,
    pub carrier: String,
    pub added_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub last_fetch: Option<DateTime<Utc>>,
    pub cached_status: TrackingStatus,
    pub cached_eta: Option<String>,
    pub cached_location: Option<String>,
    pub cached_events: Vec<TrackingEvent>,
}

pub fn load(path: &Path) -> Result<Vec<Shipment>, Error> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::Other(format!("cannot read shipments: {e}")))?;
    if content.trim().is_empty() {
        return Ok(vec![]);
    }
    serde_json::from_str(&content).map_err(|e| Error::Other(format!("invalid shipments file: {e}")))
}

pub fn save(path: &Path, shipments: &[Shipment]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Other(format!("cannot create directory: {e}")))?;
    }
    let content = serde_json::to_string_pretty(shipments)
        .map_err(|e| Error::Other(format!("cannot serialize shipments: {e}")))?;
    std::fs::write(path, content).map_err(|e| Error::Other(format!("cannot write shipments: {e}")))
}

pub fn find_matching_names<'a>(shipments: &'a [Shipment], query: &str) -> Vec<&'a str> {
    let lower = query.to_lowercase();
    shipments
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&lower))
        .map(|s| s.name.as_str())
        .collect()
}

pub fn remove_by_name(shipments: &[Shipment], query: &str) -> (Vec<Shipment>, bool) {
    let matches = find_matching_names(shipments, query);
    if matches.len() != 1 {
        return (shipments.to_vec(), false);
    }
    let matched_name = matches[0];
    let remaining: Vec<Shipment> = shipments
        .iter()
        .filter(|s| s.name != matched_name)
        .cloned()
        .collect();
    (remaining, true)
}

pub fn cleanup(shipments: &[Shipment], max_days: u32) -> (Vec<Shipment>, usize) {
    let cutoff = Utc::now() - chrono::Duration::days(max_days as i64);
    let mut removed = 0;
    let remaining: Vec<Shipment> = shipments
        .iter()
        .filter(|s| {
            if let Some(delivered_at) = s.delivered_at
                && delivered_at < cutoff
            {
                removed += 1;
                return false;
            }
            true
        })
        .cloned()
        .collect();
    (remaining, removed)
}

impl Shipment {
    pub fn needs_refresh(&self, cache_minutes: u32) -> bool {
        match self.last_fetch {
            None => true,
            Some(last) => {
                let age = Utc::now() - last;
                age > chrono::Duration::minutes(cache_minutes as i64)
            }
        }
    }

    pub fn update_from_result(&mut self, result: &crate::carriers::TrackingResult) {
        self.carrier = result.carrier.clone();
        self.cached_status = result.status.clone();
        self.cached_eta = result.eta.clone();
        self.cached_location = result.location.clone();
        self.cached_events = result.events.clone();
        self.last_fetch = Some(Utc::now());

        if result.status == TrackingStatus::Delivered && self.delivered_at.is_none() {
            self.delivered_at = Some(Utc::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_shipment(name: &str) -> Shipment {
        Shipment {
            name: name.to_string(),
            tracking_number: "TEST123".to_string(),
            postcode: None,
            carrier: "DHL".to_string(),
            added_at: chrono::Utc::now(),
            delivered_at: None,
            last_fetch: None,
            cached_status: TrackingStatus::Pending,
            cached_eta: None,
            cached_location: None,
            cached_events: vec![],
        }
    }

    #[test]
    fn add_and_load_shipment() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shipments.json");
        let s = sample_shipment("Test package");
        save(&path, &[s.clone()]).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Test package");
    }

    #[test]
    fn load_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shipments.json");
        let loaded = load(&path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn remove_by_exact_name() {
        let shipments = vec![sample_shipment("Monitor"), sample_shipment("Keyboard")];
        let (remaining, removed) = remove_by_name(&shipments, "Monitor");
        assert_eq!(remaining.len(), 1);
        assert!(removed);
    }

    #[test]
    fn remove_by_partial_name() {
        let shipments = vec![sample_shipment("New monitor"), sample_shipment("Keyboard")];
        let (remaining, removed) = remove_by_name(&shipments, "moni");
        assert_eq!(remaining.len(), 1);
        assert!(removed);
    }

    #[test]
    fn remove_ambiguous_returns_false() {
        let shipments = vec![sample_shipment("Monitor 1"), sample_shipment("Monitor 2")];
        let (remaining, removed) = remove_by_name(&shipments, "Monitor");
        assert_eq!(remaining.len(), 2);
        assert!(!removed);
    }

    #[test]
    fn cleanup_removes_old_delivered() {
        let mut s = sample_shipment("Old delivery");
        s.delivered_at = Some(chrono::Utc::now() - chrono::Duration::days(10));
        let (remaining, count) = cleanup(&[s], 7);
        assert!(remaining.is_empty());
        assert_eq!(count, 1);
    }

    #[test]
    fn cleanup_keeps_recent_delivered() {
        let mut s = sample_shipment("Recent delivery");
        s.delivered_at = Some(chrono::Utc::now() - chrono::Duration::days(3));
        let (remaining, count) = cleanup(&[s], 7);
        assert_eq!(remaining.len(), 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn cleanup_keeps_undelivered() {
        let s = sample_shipment("In transit");
        let (remaining, count) = cleanup(&[s], 7);
        assert_eq!(remaining.len(), 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn find_by_name_exact() {
        let shipments = vec![sample_shipment("Monitor")];
        let names = find_matching_names(&shipments, "Monitor");
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn find_by_name_case_insensitive() {
        let shipments = vec![sample_shipment("Monitor")];
        let names = find_matching_names(&shipments, "monitor");
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn needs_refresh_true_when_no_fetch() {
        let s = sample_shipment("Test");
        assert!(s.needs_refresh(30));
    }

    #[test]
    fn needs_refresh_false_when_recent() {
        let mut s = sample_shipment("Test");
        s.last_fetch = Some(chrono::Utc::now());
        assert!(!s.needs_refresh(30));
    }

    #[test]
    fn needs_refresh_true_when_stale() {
        let mut s = sample_shipment("Test");
        s.last_fetch = Some(chrono::Utc::now() - chrono::Duration::minutes(60));
        assert!(s.needs_refresh(30));
    }

    #[test]
    fn update_from_result_sets_delivered_at() {
        let mut s = sample_shipment("Test");
        assert!(s.delivered_at.is_none());
        let result = crate::carriers::TrackingResult {
            carrier: "PostNL".to_string(),
            status: TrackingStatus::Delivered,
            eta: None,
            location: Some("Home".to_string()),
            last_update: None,
            events: vec![],
        };
        s.update_from_result(&result);
        assert!(s.delivered_at.is_some());
        assert_eq!(s.cached_status, TrackingStatus::Delivered);
        assert_eq!(s.carrier, "PostNL");
    }

    #[test]
    fn update_from_result_keeps_delivered_at_once_set() {
        let mut s = sample_shipment("Test");
        let original_time = chrono::Utc::now() - chrono::Duration::days(1);
        s.delivered_at = Some(original_time);
        let result = crate::carriers::TrackingResult {
            carrier: "DHL".to_string(),
            status: TrackingStatus::Delivered,
            eta: None,
            location: None,
            last_update: None,
            events: vec![],
        };
        s.update_from_result(&result);
        assert_eq!(s.delivered_at, Some(original_time));
    }
}
