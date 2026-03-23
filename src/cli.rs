use clap::{Parser, Subcommand};

/// CLI tool for analyzing AI coding session logs and extracting daily dev insights
#[derive(Parser)]
#[command(name = "rwd", version, about, after_help = "\
Examples:
  rwd today                         Analyze and print results
  rwd today -v                      Show detailed execution plan
  rwd today -b                      Run in background, notify when done
  rwd summary                       Summarize today's work and save to Obsidian
  rwd slack                         Generate Slack message and copy to clipboard
  rwd init                          Set up API key and output path
  rwd config                        Interactive settings menu
  rwd config output-path ~/vault    Set Obsidian vault path
  rwd config provider openai        Switch LLM provider
  rwd config api-key                Change API key
  rwd update                        Update to the latest version")]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Analyze today's AI coding sessions and save insights to Obsidian
    #[command(after_help = "\
Examples:
  rwd today       Analyze and print results
  rwd today -v    Show detailed execution plan
  rwd today -b    Run in background, notify when done")]
    Today {
        /// Show detailed execution plan per session
        #[arg(long, short)]
        verbose: bool,
        /// Override output language (en/ko)
        #[arg(long)]
        lang: Option<String>,
        /// Run in background with OS notification on completion
        #[arg(long, short)]
        background: bool,
        /// Internal: run as background worker (hidden from help)
        #[arg(long, hide = true)]
        worker: bool,
    },
    /// Summarize today's work and save to Obsidian (runs today first if needed)
    Summary {
        /// Override output language (en/ko)
        #[arg(long)]
        lang: Option<String>,
    },
    /// Generate a Slack-ready message and copy to clipboard
    Slack {
        /// Override output language (en/ko)
        #[arg(long)]
        lang: Option<String>,
    },
    /// Run initial setup (API key, output path)
    Init,
    /// Change a config value (interactive menu if no args)
    #[command(after_help = "\
Examples:
  rwd config                        Interactive settings menu
  rwd config output-path ~/vault    Set Obsidian vault path
  rwd config provider openai        Switch LLM provider")]
    Config {
        /// Config key (output-path, provider, api-key)
        key: Option<String>,
        /// Value to set
        value: Option<String>,
    },
    /// Update to the latest version via GitHub Releases
    Update,
}
