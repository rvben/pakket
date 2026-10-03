# pakket

Track shipments from the command line.

`pakket` looks up parcels across PostNL, DHL, and 17track (3200+ carriers) behind one interface, detects the carrier from the tracking number, and can keep a saved list of shipments it refreshes for you. Human-readable on a TTY, JSON with `--json`, and a `pakket schema` contract (clispec v0.2) for agents.

## Install

```sh
cargo install pakket
```

### Nix

With Nix’s `nix-command` and `flakes` features enabled, packages are available
for Linux x64/ARM64 and Apple Silicon macOS:

```sh
nix run github:rvben/pakket -- --help
nix build github:rvben/pakket
```

Both `Cargo.lock` and `flake.lock` are committed. The package runs the Rust test
suite and checks the installed command and Bash, Fish, and Zsh completions.
Intel Macs are not supported by the pinned nixpkgs; use the other installation
methods above.

For NixOS or Home Manager, add the input to your flake:

```nix
inputs.pakket = {
  url = "github:rvben/pakket";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Then add `inputs.pakket.packages.${pkgs.stdenv.hostPlatform.system}.default`
to `environment.systemPackages` (NixOS) or `home.packages` (Home Manager).
Pass `inputs` through `specialArgs` for `nixosSystem` or `extraSpecialArgs` for
`homeManagerConfiguration`. The overlay `inputs.pakket.overlays.default`
also provides `pkgs.pakket` using your package set and Rust toolchain overrides.
Following your own nixpkgs uses its toolchain; it must meet the Rust version
required by `Cargo.toml` and support your platform.

For development:

```sh
nix develop
nix flake check                  # build, Rust tests, installed-command checks
nix fmt -- --check flake.nix nix/*.nix
```

`direnv allow` is optional and requires nix-direnv. The development shell
includes the package's native build dependencies and Rust development tools.
Update Nix inputs deliberately with `nix flake update`, review the lockfile,
and run the checks before committing it.

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

## Releasing

Vership owns versioning, changelog generation, release commits, and tags. See
[the release runbook](docs/releases.md) for the verified workflow and recovery policy.
