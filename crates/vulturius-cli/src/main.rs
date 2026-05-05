use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use vulturius_core::config::AppConfig;

mod commands;
mod output;

#[derive(Parser)]
#[command(name = "vulturius")]
#[command(author, version, about = "Intelligent disk analysis and cleanup tool", long_about = None)]
struct Cli {
    /// Increase logging verbosity (repeat for more detail)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long)]
    quiet: bool,

    /// Output format
    #[arg(short, long, default_value = "table")]
    output_format: String,

    /// Custom config file path
    #[arg(short, long)]
    config: Option<String>,

    /// Simulate without making changes
    #[arg(long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan disk/filessystem for analyzable data
    Scan {
        #[arg(long)]
        quick: bool,
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long)]
        analyze: bool,
        #[arg(long, default_value = "balanced")]
        risk_tolerance: String,
    },
    /// Analyze scan results and generate recommendations
    Analyze,
    /// Execute cleanup operations
    Clean {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        quarantine: bool,
        #[arg(long, default_value = "balanced")]
        risk_tolerance: String,
        #[arg(long)]
        quick: bool,
        #[arg(long)]
        confirm: bool,
    },
    /// Application analysis and management
    App {
        #[command(subcommand)]
        command: AppCommand,
    },
    /// Windows registry analysis and cleanup
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    /// Manage quarantined files
    Quarantine {
        #[command(subcommand)]
        command: QuarantineCommand,
    },
    /// Generate and view reports
    Report {
        #[arg(long, default_value = "text")]
        format: String,
        #[arg(long)]
        output: Option<String>,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// View cleanup history
    History,
    /// Reverse previous cleanup operations
    Undo {
        #[arg(long, default_value_t = 10)]
        count: usize,
    },
}

#[derive(Subcommand)]
enum AppCommand {
    /// List installed applications
    List,
    /// Uninstall an application
    Uninstall {
        name: String,
        #[arg(long)]
        cleanup_leftovers: bool,
    },
}

#[derive(Subcommand)]
enum RegistryCommand {
    /// Scan for orphaned/unused registry entries
    Scan,
    /// Clean orphaned registry entries
    Clean,
}

#[derive(Subcommand)]
enum QuarantineCommand {
    /// List quarantined files
    List,
    /// Restore a quarantined file
    Restore { id: String },
    /// Purge all quarantined files
    Purge,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set { key: String, value: String },
    /// Reset to defaults
    Reset,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => if cli.quiet { "error" } else { "info" },
        1 => "debug",
        2 => "trace",
        _ => "trace",
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(log_level))
        .with_target(false)
        .init();

    let config = AppConfig::default();

    match cli.command {
        Command::Scan {
            quick,
            depth,
            analyze,
            risk_tolerance,
        } => commands::scan::handle(&config, quick, depth, analyze, risk_tolerance).await,
        Command::Analyze => commands::analyze::handle(&config).await,
        Command::Clean {
            dry_run,
            quarantine,
            risk_tolerance,
            quick,
            confirm,
        } => commands::clean::handle(&config, dry_run, quarantine, risk_tolerance, quick, confirm).await,
        Command::App { command } => match command {
            AppCommand::List => commands::app::handle(&config).await,
            AppCommand::Uninstall { .. } => {
                println!("App uninstall — full implementation in Phase 4");
                Ok(())
            }
        },
        Command::Registry { command } => match command {
            RegistryCommand::Scan => {
                commands::registry::handle(&config, commands::registry::RegistrySubCommand::Scan).await
            }
            RegistryCommand::Clean => {
                commands::registry::handle(&config, commands::registry::RegistrySubCommand::Clean { dry_run: cli.dry_run }).await
            }
        },
        Command::Quarantine { command } => {
            let qc = match command {
                QuarantineCommand::List => commands::quarantine::QuarantineCommand::List,
                QuarantineCommand::Restore { id } => commands::quarantine::QuarantineCommand::Restore { id },
                QuarantineCommand::Purge => commands::quarantine::QuarantineCommand::Purge,
            };
            commands::quarantine::handle(&config, qc).await
        }
        Command::Report { format, output } => commands::report::handle(&config, format, output).await,
        Command::Config { .. } => commands::config::handle(&config).await,
        Command::History => commands::history::handle(&config).await,
        Command::Undo { count } => commands::undo::handle(&config, count).await,
    }?;

    Ok(())
}
