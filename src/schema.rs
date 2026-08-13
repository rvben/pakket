use serde_json::{Value, json};

pub fn generate() -> Value {
    let tracking_fields = json!([
        {"name": "carrier", "type": "string", "description": "Detected carrier (PostNL, DHL, 17track, ...)"},
        {"name": "status", "type": "string", "description": "Tracking status (Pending, InTransit, Delivered, ...)"},
        {"name": "eta", "type": "string", "description": "Estimated delivery time, if known"},
        {"name": "location", "type": "string", "description": "Last known location, if known"},
        {"name": "last_update", "type": "string", "description": "Timestamp of the most recent event"},
        {"name": "events", "type": "array", "items": {"type": "object"}, "description": "Event history; each has a timestamp, status, and description"}
    ]);

    json!({
        "clispec": "0.3",
        "name": "pakket",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Track shipments from PostNL, DHL, and 17track (3200+ carriers) from the command line",
        "output": {"tty": "text", "piped": "json"},
        "global_args": [
            {"name": "--json", "type": "boolean", "required": false, "description": "Output as JSON"},
            {"name": "--output", "short": "-o", "type": "string", "enum": ["auto", "json", "text"], "default": "auto", "description": "Explicit output format; auto selects JSON when piped"},
            {"name": "--profile", "type": "string", "required": false, "description": "Configuration profile (env: PAKKET_PROFILE)"},
            {"name": "--limit", "type": "integer", "required": false, "description": "Maximum saved shipments returned by list"},
            {"name": "--offset", "type": "integer", "default": 0, "description": "Saved shipments to skip"},
            {"name": "--fields", "type": "string[]", "required": false, "description": "Comma-separated fields to include in JSON list output"}
        ],
        "commands": [
            {
                "name": "track",
                "description": "Track a shipment by tracking number (carrier auto-detected)",
                "effects": "read_only",
                "mutating": false,
                "cardinality": "single",
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
                "effects": "non_idempotent",
                "mutating": true,
                "cardinality": "single",
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
                "effects": "non_idempotent",
                "mutating": true,
                "cardinality": "unbounded",
                "pagination": {"style": "offset", "limit_arg": "--limit", "offset_arg": "--offset"},
                "fields_arg": "--fields",
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
                "effects": "non_idempotent",
                "mutating": true,
                "cardinality": "single",
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
                "effects": "non_idempotent",
                "mutating": true,
                "cardinality": "single",
                "args": [],
                "stdout_schema": {}
            },
            {
                "name": "config show",
                "description": "Show configuration file path and contents (secrets masked)",
                "effects": "read_only",
                "mutating": false,
                "cardinality": "single",
                "args": [],
                "output_fields": [
                    {"name": "path", "type": "string"},
                    {"name": "exists", "type": "boolean"},
                    {"name": "contents", "type": "string"}
                ],
                "example": {"args": ["config", "show"]}
            },
            {
                "name": "schema",
                "description": "Output this machine-readable clispec contract as JSON",
                "effects": "read_only",
                "mutating": false,
                "cardinality": "single",
                "args": [],
                "stdout_schema": {"$ref": "https://clispec.dev/schema/v0.3.json"}
            },
            {
                "name": "completions",
                "description": "Generate shell completions",
                "effects": "read_only",
                "mutating": false,
                "output_kind": "opaque",
                "media_type": "text/plain",
                "args": [
                    {"name": "shell", "type": "string", "required": true, "description": "Shell to generate for (bash, zsh, fish, elvish, powershell)"}
                ]
            }
        ],
        "outcomes": [],
        "errors": [
            {"kind": "general", "exit_code": 1, "retryable": false, "message": "General error (HTTP or other failure)", "hint": "Check connectivity and the tracking number"},
            {"kind": "usage", "exit_code": 2, "retryable": false, "message": "Invalid command-line arguments", "hint": "Run pakket --help"},
            {"kind": "config", "exit_code": 2, "retryable": false, "message": "Configuration error (missing API key or postcode)", "hint": "Run pakket config init"},
            {"kind": "api", "exit_code": 3, "retryable": true, "message": "Carrier or aggregator API error", "hint": "Often transient; retry later"},
            {"kind": "http", "exit_code": 1, "retryable": true, "message": "HTTP transport error", "hint": "Check connectivity and retry"},
            {"kind": "not_found", "exit_code": 4, "retryable": false, "message": "Shipment not found", "hint": "Run pakket list to see saved shipments"}
        ]
    })
}
