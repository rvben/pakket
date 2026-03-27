use std::process;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

use pakket::carriers::{Carrier, DetectedCarrier};
use pakket::error::Error;

#[derive(Parser)]
#[command(name = "pakket", version, about = "CLI for tracking shipments")]
struct Cli {
    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Configuration profile
    #[arg(long, env = "PAKKET_PROFILE", global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Track a shipment
    Track {
        /// Tracking number
        number: String,
        /// Show full event history
        #[arg(long)]
        history: bool,
        /// Override carrier detection
        #[arg(long)]
        carrier: Option<String>,
        /// Postal code (required for PostNL)
        #[arg(long)]
        postcode: Option<String>,
    },
    /// Save a shipment for ongoing tracking
    Add {
        /// Name for this shipment
        name: String,
        /// Tracking number
        number: String,
        /// Override carrier detection
        #[arg(long)]
        carrier: Option<String>,
        /// Postal code (required for PostNL)
        #[arg(long)]
        postcode: Option<String>,
    },
    /// List all saved shipments
    List {
        /// Show full event history
        #[arg(long)]
        history: bool,
        /// Force refresh from API
        #[arg(long)]
        refresh: bool,
    },
    /// Remove a saved shipment
    Remove {
        /// Shipment name (partial match supported)
        name: String,
    },
    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Print JSON schema for agent integration
    Schema,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Initialize configuration interactively
    Init,
    /// Show configuration file path and contents
    Show,
}

fn load_config_optional(profile: Option<&str>) -> Option<pakket::config::Config> {
    pakket::config::Config::load(profile).ok()
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output = pakket::output::OutputConfig::new(cli.json);

    if let Err(e) = run(cli, output).await {
        eprintln!("Error: {e}");
        process::exit(e.exit_code());
    }
}

/// Track a number using the best available backend.
/// Priority: 17track (if configured) > carrier-specific > error.
async fn track_number(
    number: &str,
    postcode: Option<&str>,
    config: &Option<pakket::config::Config>,
) -> Result<pakket::carriers::TrackingResult, Error> {
    // 17track first — universal backend, handles all carriers
    if let Some(key) = config.as_ref().and_then(|c| c.seventeen_track_api_key.as_ref()) {
        let client = pakket::carriers::seventeen::SeventeenTrack::new(key, None);
        return client.track(number).await;
    }

    // Fall back to carrier-specific backends
    let detected = pakket::carriers::detect_carrier(number);
    match detected {
        DetectedCarrier::PostNL => {
            let pc = postcode
                .or_else(|| config.as_ref().and_then(|c| c.postcode.as_deref()))
                .ok_or_else(|| {
                    Error::Config("PostNL requires --postcode or postcode in config".to_string())
                })?;
            let client = pakket::carriers::postnl::PostNL::new(None);
            client.track_with_postcode(number, pc).await
        }
        DetectedCarrier::DHL => {
            let api_key = config
                .as_ref()
                .and_then(|c| c.dhl_api_key.as_ref())
                .ok_or_else(|| {
                    Error::Config("DHL requires dhl_api_key in config (free at developer.dhl.com)".to_string())
                })?;
            let client = pakket::carriers::dhl::Dhl::new(api_key, None);
            client.track(number).await
        }
        DetectedCarrier::Unknown => Err(Error::Config(
            "Unknown carrier. Configure seventeen_track_api_key for universal tracking, or dhl_api_key / postcode for direct backends".to_string(),
        )),
    }
}

/// Try tracking a number, returning None on failure (for list refresh).
async fn try_track_number(
    number: &str,
    postcode: Option<&str>,
    config: &Option<pakket::config::Config>,
) -> Option<pakket::carriers::TrackingResult> {
    track_number(number, postcode, config).await.ok()
}

async fn run(cli: Cli, output: pakket::output::OutputConfig) -> Result<(), Error> {
    match cli.command {
        Command::Schema => {
            use clap::CommandFactory;
            let cmd = Cli::command();
            let schema = pakket::schema::generate(&cmd);
            println!(
                "{}",
                serde_json::to_string_pretty(&schema).expect("serialize")
            );
            Ok(())
        }
        Command::Completions { shell } => {
            use clap::CommandFactory;
            use clap_complete::generate;
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "pakket", &mut std::io::stdout());
            Ok(())
        }
        Command::Config(ConfigCommand::Init) => {
            use dialoguer::Input;

            let config_path = pakket::config::config_path();
            let profile = cli.profile.as_deref().unwrap_or("default");

            eprintln!("pakket supports three tracking backends:\n");
            eprintln!("  PostNL   No account needed, just your postal code");
            eprintln!("  DHL      Free API key (personal signup, no company)");
            eprintln!("  17track  3200+ carriers (requires account signup)\n");

            // 1. PostNL — just a postcode
            let postcode: String = Input::new()
                .with_prompt("Your postal code (enables PostNL tracking)")
                .allow_empty(true)
                .interact_text()
                .map_err(|e| Error::Other(format!("input error: {e}")))?;

            // 2. DHL — free API key
            let dhl_key: String = Input::new()
                .with_prompt("DHL API key (free: developer.dhl.com, empty to skip)")
                .allow_empty(true)
                .interact_text()
                .map_err(|e| Error::Other(format!("input error: {e}")))?;
            let dhl_key = if dhl_key.is_empty() {
                None
            } else {
                Some(dhl_key)
            };

            // 3. 17track — universal
            let seventeen_key: String = Input::new()
                .with_prompt("17track API key (3200+ carriers: 17track.net/en/api, empty to skip)")
                .allow_empty(true)
                .interact_text()
                .map_err(|e| Error::Other(format!("input error: {e}")))?;
            let seventeen_key = if seventeen_key.is_empty() {
                None
            } else {
                Some(seventeen_key)
            };

            // Save
            pakket::config::Config::save_config(
                &config_path,
                profile,
                if postcode.is_empty() {
                    None
                } else {
                    Some(&postcode)
                },
                dhl_key.as_deref(),
                seventeen_key.as_deref(),
            )?;

            // Summary
            eprintln!("\nConfig saved to {}\n", config_path.display());
            eprintln!("Configured backends:");
            if !postcode.is_empty() {
                eprintln!("  PostNL    ready (postcode: {})", postcode);
            }
            if dhl_key.is_some() {
                eprintln!("  DHL       ready");
            }
            if seventeen_key.is_some() {
                eprintln!("  17track   ready (universal)");
            }
            if postcode.is_empty() && dhl_key.is_none() && seventeen_key.is_none() {
                eprintln!("  (none)    run 'pakket config init' again to set up backends");
            }

            Ok(())
        }
        Command::Config(ConfigCommand::Show) => {
            let path = pakket::config::config_path();
            println!("Config file: {}", path.display());
            println!();
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    for line in contents.lines() {
                        let trimmed = line.trim_start();
                        if trimmed.starts_with("api_key")
                            || trimmed.starts_with("dhl_api_key")
                            || trimmed.starts_with("seventeen_track_api_key")
                        {
                            if let Some((key, _)) = line.split_once('=') {
                                println!("{key}= \"****\"");
                            } else {
                                println!("{line}");
                            }
                        } else {
                            println!("{line}");
                        }
                    }
                }
                Err(_) => {
                    println!("No config file found.");
                    println!("Run 'pakket config init' to create one.");
                }
            }
            Ok(())
        }
        Command::Track {
            number,
            history,
            carrier: _carrier,
            postcode,
        } => {
            let config = load_config_optional(cli.profile.as_deref());
            let result = track_number(&number, postcode.as_deref(), &config).await?;
            pakket::commands::track::print_result(&output, &result, history);
            Ok(())
        }
        Command::Add {
            name,
            number,
            carrier: _carrier,
            postcode,
        } => {
            let config = load_config_optional(cli.profile.as_deref());
            let postcode = postcode.or_else(|| config.as_ref().and_then(|c| c.postcode.clone()));
            let result = track_number(&number, postcode.as_deref(), &config).await?;

            let shipment = pakket::commands::add::create_shipment(
                &name,
                &number,
                postcode.as_deref(),
                &result,
            );
            let shipments_path = pakket::config::shipments_path();
            let mut shipments = pakket::shipments::load(&shipments_path)?;
            shipments.push(shipment);
            pakket::shipments::save(&shipments_path, &shipments)?;

            if output.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).expect("serialize")
                );
            } else {
                output.print_message(&format!("Added '{}' ({})", name, result.carrier));
            }
            Ok(())
        }
        Command::List { history, refresh } => {
            let config = load_config_optional(cli.profile.as_deref());
            let auto_cleanup_days = config.as_ref().map(|c| c.auto_cleanup_days).unwrap_or(7);
            let cache_minutes = config.as_ref().map(|c| c.cache_minutes).unwrap_or(30);

            let shipments_path = pakket::config::shipments_path();
            let mut shipments = pakket::shipments::load(&shipments_path)?;

            let (cleaned, removed_count) =
                pakket::shipments::cleanup(&shipments, auto_cleanup_days);
            if removed_count > 0 {
                output.print_message(&format!(
                    "Cleaned up {} delivered shipment(s)",
                    removed_count
                ));
            }
            shipments = cleaned;

            for s in &mut shipments {
                if refresh || s.needs_refresh(cache_minutes) {
                    let pc = s
                        .postcode
                        .as_deref()
                        .or_else(|| config.as_ref().and_then(|c| c.postcode.as_deref()));
                    if let Some(result) =
                        try_track_number(&s.tracking_number, pc, &config).await
                    {
                        s.update_from_result(&result);
                    }
                }
            }

            pakket::shipments::save(&shipments_path, &shipments)?;
            pakket::commands::list::print_list(&output, &shipments, history);
            Ok(())
        }
        Command::Remove { name } => {
            let shipments_path = pakket::config::shipments_path();
            let shipments = pakket::shipments::load(&shipments_path)?;

            let matches = pakket::shipments::find_matching_names(&shipments, &name);
            if matches.is_empty() {
                Err(Error::NotFound(format!("shipment '{name}'")))
            } else if matches.len() > 1 {
                eprintln!("Ambiguous name '{}'. Matches:", name);
                for m in &matches {
                    eprintln!("  - {m}");
                }
                Err(Error::Other("ambiguous name".to_string()))
            } else {
                let (remaining, _) = pakket::shipments::remove_by_name(&shipments, &name);
                pakket::shipments::save(&shipments_path, &remaining)?;

                if output.json {
                    println!("{}", serde_json::json!({"removed": matches[0]}));
                } else {
                    output.print_message(&format!("Removed '{}'", matches[0]));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_track_basic() {
        let cli = Cli::try_parse_from(["pakket", "track", "TEST123"]).unwrap();
        match cli.command {
            Command::Track {
                number,
                history,
                carrier,
                postcode,
            } => {
                assert_eq!(number, "TEST123");
                assert!(!history);
                assert!(carrier.is_none());
                assert!(postcode.is_none());
            }
            _ => panic!("expected Track"),
        }
    }

    #[test]
    fn cli_track_with_history() {
        let cli = Cli::try_parse_from(["pakket", "track", "TEST123", "--history"]).unwrap();
        match cli.command {
            Command::Track { history, .. } => assert!(history),
            _ => panic!("expected Track"),
        }
    }

    #[test]
    fn cli_track_with_carrier() {
        let cli = Cli::try_parse_from(["pakket", "track", "TEST123", "--carrier", "dhl"]).unwrap();
        match cli.command {
            Command::Track { carrier, .. } => assert_eq!(carrier.as_deref(), Some("dhl")),
            _ => panic!("expected Track"),
        }
    }

    #[test]
    fn cli_track_with_postcode() {
        let cli =
            Cli::try_parse_from(["pakket", "track", "3STEST123", "--postcode", "1234AB"]).unwrap();
        match cli.command {
            Command::Track { postcode, .. } => assert_eq!(postcode.as_deref(), Some("1234AB")),
            _ => panic!("expected Track"),
        }
    }

    #[test]
    fn cli_add() {
        let cli = Cli::try_parse_from(["pakket", "add", "Monitor", "TEST123"]).unwrap();
        match cli.command {
            Command::Add {
                name,
                number,
                carrier,
                postcode,
            } => {
                assert_eq!(name, "Monitor");
                assert_eq!(number, "TEST123");
                assert!(carrier.is_none());
                assert!(postcode.is_none());
            }
            _ => panic!("expected Add"),
        }
    }

    #[test]
    fn cli_add_with_postcode() {
        let cli = Cli::try_parse_from([
            "pakket",
            "add",
            "Monitor",
            "3STEST123",
            "--postcode",
            "9999ZZ",
        ])
        .unwrap();
        match cli.command {
            Command::Add { postcode, .. } => assert_eq!(postcode.as_deref(), Some("9999ZZ")),
            _ => panic!("expected Add"),
        }
    }

    #[test]
    fn cli_list() {
        let cli = Cli::try_parse_from(["pakket", "list"]).unwrap();
        assert!(matches!(cli.command, Command::List { .. }));
    }

    #[test]
    fn cli_list_with_refresh() {
        let cli = Cli::try_parse_from(["pakket", "list", "--refresh"]).unwrap();
        match cli.command {
            Command::List { refresh, .. } => assert!(refresh),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn cli_remove() {
        let cli = Cli::try_parse_from(["pakket", "remove", "Monitor"]).unwrap();
        match cli.command {
            Command::Remove { name } => assert_eq!(name, "Monitor"),
            _ => panic!("expected Remove"),
        }
    }

    #[test]
    fn cli_schema() {
        let cli = Cli::try_parse_from(["pakket", "schema"]).unwrap();
        assert!(matches!(cli.command, Command::Schema));
    }

    #[test]
    fn cli_json_flag() {
        let cli = Cli::try_parse_from(["pakket", "--json", "schema"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn cli_profile_flag() {
        let cli = Cli::try_parse_from(["pakket", "--profile", "work", "schema"]).unwrap();
        assert_eq!(cli.profile.as_deref(), Some("work"));
    }

    #[test]
    fn cli_config_init() {
        let cli = Cli::try_parse_from(["pakket", "config", "init"]).unwrap();
        assert!(matches!(cli.command, Command::Config(ConfigCommand::Init)));
    }

    #[test]
    fn cli_missing_subcommand_fails() {
        let result = Cli::try_parse_from(["pakket"]);
        assert!(result.is_err());
    }
}
