use serde_json::{Value, json};

pub fn generate() -> Value {
    let tracking_fields = json!([
        {"name": "carrier", "type": "string", "description": "Detected carrier (PostNL, DHL, 17track, ...)"},
        {"name": "status", "type": "string", "description": "Tracking status (Pending, InTransit, Delivered, ...)"},
        {"name": "eta", "type": "string", "description": "Estimated delivery time, if known"},
        {"name": "location", "type": "string", "description": "Last known location, if known"},
        {"name": "last_update", "type": "string", "description": "Timestamp of the most recent event"},
        {"name": "events", "type": "array", "description": "Event history; each has a timestamp, status, and description"}
    ]);

    json!({
        "clispec": "0.2",
        "name": "pakket",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Track shipments from PostNL, DHL, and 17track (3200+ carriers) from the command line",
        "global_args": [
            {"name": "--json", "type": "boolean", "required": false, "description": "Output as JSON"},
            {"name": "--profile", "type": "string", "required": false, "description": "Configuration profile (env: PAKKET_PROFILE)"}
        ],
        "commands": [
            {
                "name": "track",
                "description": "Track a shipment by tracking number (carrier auto-detected)",
                "mutating": false,
                "args": [
                    {"name": "number", "type": "string", "required": true, "description": "Tracking number"},
                    {"name": "--history", "type": "boolean", "required": false, "description": "Show full event history"},
                    {"name": "--carrier", "type": "string", "required": false, "description": "Override carrier detection"},
                    {"name": "--postcode", "type": "string", "required": false, "description": "Postal code (required for PostNL)"}
                ],
                "output_fields": tracking_fields.clone()
            },
            {
                "name": "add",
                "description": "Save a shipment for ongoing tracking",
                "mutating": true,
                "args": [
                    {"name": "name", "type": "string", "required": true, "description": "Name for this shipment"},
                    {"name": "number", "type": "string", "required": true, "description": "Tracking number"},
                    {"name": "--carrier", "type": "string", "required": false, "description": "Override carrier detection"},
                    {"name": "--postcode", "type": "string", "required": false, "description": "Postal code (required for PostNL)"}
                ],
                "output_fields": tracking_fields.clone()
            },
            {
                "name": "list",
                "description": "List all saved shipments, refreshing stale ones",
                "mutating": true,
                "args": [
                    {"name": "--history", "type": "boolean", "required": false, "description": "Show full event history"},
                    {"name": "--refresh", "type": "boolean", "required": false, "description": "Force refresh from the API"}
                ],
                "output_fields": [
                    {"name": "name", "type": "string", "description": "Saved shipment name"},
                    {"name": "tracking_number", "type": "string", "description": "Tracking number"},
                    {"name": "carrier", "type": "string", "description": "Carrier"},
                    {"name": "status", "type": "string", "description": "Latest tracking status"}
                ],
                "notes": "Refreshes statuses for stale shipments and removes delivered ones past the auto-cleanup window, persisting the result."
            },
            {
                "name": "remove",
                "description": "Remove a saved shipment (partial name match)",
                "mutating": true,
                "args": [
                    {"name": "name", "type": "string", "required": true, "description": "Shipment name (partial match supported)"}
                ],
                "output_fields": [
                    {"name": "removed", "type": "string", "description": "Name of the removed shipment"}
                ]
            },
            {
                "name": "config init",
                "description": "Initialize configuration interactively",
                "mutating": true,
                "args": [],
                "output_fields": []
            },
            {
                "name": "config show",
                "description": "Show configuration file path and contents (secrets masked)",
                "mutating": false,
                "args": [],
                "output_fields": []
            },
            {
                "name": "schema",
                "description": "Output this machine-readable clispec contract as JSON",
                "mutating": false,
                "args": [],
                "output_fields": []
            },
            {
                "name": "completions",
                "description": "Generate shell completions",
                "mutating": false,
                "args": [
                    {"name": "shell", "type": "string", "required": true, "description": "Shell to generate for (bash, zsh, fish, elvish, powershell)"}
                ],
                "output_fields": []
            }
        ],
        "outcomes": [],
        "errors": [
            {"kind": "general", "exit_code": 1, "retryable": false, "message": "General error (HTTP or other failure)", "hint": "Check connectivity and the tracking number"},
            {"kind": "config", "exit_code": 2, "retryable": false, "message": "Configuration error (missing API key or postcode)", "hint": "Run pakket config init"},
            {"kind": "api", "exit_code": 3, "retryable": true, "message": "Carrier or aggregator API error", "hint": "Often transient; retry later"},
            {"kind": "not_found", "exit_code": 4, "retryable": false, "message": "Shipment not found", "hint": "Run pakket list to see saved shipments"}
        ]
    })
}
