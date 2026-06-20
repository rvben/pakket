# pakket

Track shipments from the command line.

`pakket` looks up parcels across PostNL, DHL, and 17track (3200+ carriers) behind one interface, detects the carrier from the tracking number, and can keep a saved list of shipments it refreshes for you. Human-readable on a TTY, JSON with `--json`, and a `pakket schema` contract (clispec v0.2) for agents.

## Install

```sh
cargo install pakket
```

## Backends

`pakket` supports three tracking backends; configure whichever you need:

| Backend | What it needs | Coverage |
|---------|---------------|----------|
| PostNL  | Your postal code (no account) | PostNL parcels |
| DHL     | Free API key (`developer.dhl.com`) | DHL parcels |
| 17track | Account + API key (`17track.net/en/api`) | 3200+ carriers, universal |

When a 17track key is configured it is used first as a universal backend, falling back to the carrier-specific API for immediate data when 17track is still pending.

```sh
pakket config init     # interactive setup, writes the config file
pakket config show     # show config path and contents (secrets masked)
```

## Commands

Track a one-off number (carrier auto-detected):

```sh
pakket track 3STEST1234567890 --postcode 1234AB
pakket track JD0002340001234567 --history     # full event history
```

Save shipments and refresh them as a group:

```sh
pakket add "New monitor" 3STEST1234567890 --postcode 1234AB
pakket list                  # all saved shipments, refreshed if stale
pakket list --refresh        # force refresh from the API
pakket remove "monitor"      # partial name match
```

Delivered shipments are cleaned up automatically after a configurable number of days.

## Flags

- `--json`: machine-readable output (global).
- `--profile <name>` (or `PAKKET_PROFILE`): select a configuration profile (global).
- `--history`: include the full event history (`track`, `list`).
- `--carrier <name>`: override carrier detection (`track`, `add`).
- `--postcode <code>`: postal code, required for PostNL (`track`, `add`).

## Agent integration

`pakket schema` prints the full machine-readable contract (commands, arguments, output fields, exit codes) following clispec v0.2.
