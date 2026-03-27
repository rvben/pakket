use std::process;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

use pakket::carriers::Carrier;
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output = pakket::output::OutputConfig::new(cli.json);

    if let Err(e) = run(cli, output).await {
        eprintln!("Error: {e}");
        process::exit(e.exit_code());
    }
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
            use dialoguer::Password;

            let config_path = pakket::config::config_path();
            let api_key: String = Password::new()
                .with_prompt("17track API key")
                .interact()
                .map_err(|e| Error::Other(format!("input error: {e}")))?;

            eprintln!("Validating API key...");
            let client = pakket::carriers::seventeen::SeventeenTrack::new(&api_key, None);
            client.validate_key().await?;

            let profile = cli.profile.as_deref().unwrap_or("default");
            pakket::config::Config::save_api_key(&config_path, profile, &api_key)?;

            eprintln!("Config saved to {}", config_path.display());
            Ok(())
        }
        Command::Config(ConfigCommand::Show) => {
            let path = pakket::config::config_path();
            println!("Config file: {}", path.display());
            println!();
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    for line in contents.lines() {
                        if line.trim_start().starts_with("api_key") {
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
        } => {
            let config = pakket::config::Config::load(cli.profile.as_deref()).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                process::exit(e.exit_code());
            });
            let client = pakket::carriers::seventeen::SeventeenTrack::new(&config.api_key, None);
            match client.track(&number).await {
                Ok(result) => {
                    pakket::commands::track::print_result(&output, &result, history);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Command::Add {
            name,
            number,
            carrier: _carrier,
        } => {
            let config = pakket::config::Config::load(cli.profile.as_deref()).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                process::exit(e.exit_code());
            });
            let client = pakket::carriers::seventeen::SeventeenTrack::new(&config.api_key, None);
            let result = client.track(&number).await?;

            let shipment = pakket::commands::add::create_shipment(&name, &number, &result);
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
            let config = pakket::config::Config::load(cli.profile.as_deref()).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                process::exit(e.exit_code());
            });
            let client = pakket::carriers::seventeen::SeventeenTrack::new(&config.api_key, None);
            let shipments_path = pakket::config::shipments_path();
            let mut shipments = pakket::shipments::load(&shipments_path)?;

            let (cleaned, removed_count) =
                pakket::shipments::cleanup(&shipments, config.auto_cleanup_days);
            if removed_count > 0 {
                output.print_message(&format!(
                    "Cleaned up {} delivered shipment(s)",
                    removed_count
                ));
            }
            shipments = cleaned;

            for s in &mut shipments {
                if (refresh || s.needs_refresh(config.cache_minutes))
                    && let Ok(result) = client.track(&s.tracking_number).await
                {
                    s.update_from_result(&result);
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
            } => {
                assert_eq!(number, "TEST123");
                assert!(!history);
                assert!(carrier.is_none());
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
    fn cli_add() {
        let cli = Cli::try_parse_from(["pakket", "add", "Monitor", "TEST123"]).unwrap();
        match cli.command {
            Command::Add {
                name,
                number,
                carrier,
            } => {
                assert_eq!(name, "Monitor");
                assert_eq!(number, "TEST123");
                assert!(carrier.is_none());
            }
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
