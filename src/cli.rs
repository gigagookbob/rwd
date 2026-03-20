use clap::{Parser, Subcommand};

/// CLI tool for analyzing AI coding session logs and extracting daily dev insights
#[derive(Parser)]
#[command(name = "rwd", version, about)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Analyze today's session logs
    Today {
        /// Show detailed execution plan per session
        #[arg(long, short)]
        verbose: bool,
    },
    /// Run initial setup (API key, output path)
    Init,
    /// Change a config value (interactive menu if no args)
    Config {
        /// Config key (output-path, provider, api-key)
        key: Option<String>,
        /// Value to set
        value: Option<String>,
    },
    /// Update to the latest version
    Update,
    /// Generate a development progress summary
    Summary,
    /// Generate a Slack-ready share message
    Slack,
}
