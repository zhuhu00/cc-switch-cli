use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub mod commands;
pub mod editor;
pub mod i18n;
pub mod interactive;
pub mod terminal;
pub mod tui;
pub mod ui;

use crate::app_config::AppType;

#[derive(Parser)]
#[command(
    name = "cc-switch",
    version,
    about = "All-in-One Assistant for Claude Code, Codex, Gemini & OpenCode CLI",
    long_about = "Unified management for Claude Code, Codex, Gemini, and OpenCode CLI provider configurations, MCP servers, skills, environment checks, and system prompts.\n\nRun without arguments to enter interactive mode."
)]
pub struct Cli {
    /// Specify the application type
    #[arg(short, long, global = true, value_enum)]
    pub app: Option<AppType>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage providers (list, switch, speedtest, stream-check, fetch-models)
    #[command(subcommand)]
    Provider(commands::provider::ProviderCommand),

    /// Manage MCP servers (list, add, edit, delete, sync)
    #[command(subcommand)]
    Mcp(commands::mcp::McpCommand),

    /// Manage prompts (list, activate, edit)
    #[command(subcommand)]
    Prompts(commands::prompts::PromptsCommand),

    /// Manage skills and skill repositories
    #[command(subcommand)]
    Skills(commands::skills::SkillsCommand),

    /// Manage configuration, backups, common snippets, and WebDAV sync
    #[command(subcommand)]
    Config(commands::config::ConfigCommand),

    /// Manage local multi-app proxy
    #[command(subcommand)]
    Proxy(commands::proxy::ProxyCommand),

    /// Manage environment variables and local CLI tool checks
    #[command(subcommand)]
    Env(commands::env::EnvCommand),

    /// Update cc-switch binary to latest release
    Update(commands::update::UpdateCommand),

    /// Enter interactive mode
    #[command(alias = "ui")]
    Interactive,

    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Generate shell completions
pub fn generate_completions(shell: Shell) {
    use clap::CommandFactory;
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Commands};

    #[test]
    fn parses_proxy_serve_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "proxy", "serve", "--listen-port", "0"]);

        match cli.command {
            Some(Commands::Proxy(super::commands::proxy::ProxyCommand::Serve {
                listen_port,
                ..
            })) => {
                assert_eq!(listen_port, Some(0));
            }
            _ => panic!("expected proxy serve command"),
        }
    }

    #[test]
    fn parses_proxy_serve_takeover_flags() {
        let cli = Cli::parse_from([
            "cc-switch",
            "proxy",
            "serve",
            "--takeover",
            "claude",
            "--takeover",
            "codex",
        ]);

        match cli.command {
            Some(Commands::Proxy(super::commands::proxy::ProxyCommand::Serve {
                takeovers,
                ..
            })) => {
                assert_eq!(
                    takeovers,
                    vec![super::AppType::Claude, super::AppType::Codex]
                );
            }
            _ => panic!("expected proxy serve command with takeover flags"),
        }
    }

    #[test]
    fn parses_proxy_enable_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "proxy", "enable"]);

        match cli.command {
            Some(Commands::Proxy(super::commands::proxy::ProxyCommand::Enable)) => {}
            _ => panic!("expected proxy enable command"),
        }
    }

    #[test]
    fn parses_proxy_disable_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "proxy", "disable"]);

        match cli.command {
            Some(Commands::Proxy(super::commands::proxy::ProxyCommand::Disable)) => {}
            _ => panic!("expected proxy disable command"),
        }
    }

    #[test]
    fn parses_provider_stream_check_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "provider", "stream-check", "demo"]);

        match cli.command {
            Some(Commands::Provider(super::commands::provider::ProviderCommand::StreamCheck {
                id,
            })) => {
                assert_eq!(id, "demo");
            }
            _ => panic!("expected provider stream-check command"),
        }
    }

    #[test]
    fn parses_provider_fetch_models_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "provider", "fetch-models", "demo"]);

        match cli.command {
            Some(Commands::Provider(super::commands::provider::ProviderCommand::FetchModels {
                id,
            })) => {
                assert_eq!(id, "demo");
            }
            _ => panic!("expected provider fetch-models command"),
        }
    }

    #[test]
    fn parses_config_webdav_show_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "config", "webdav", "show"]);

        match cli.command {
            Some(Commands::Config(super::commands::config::ConfigCommand::WebDav(
                super::commands::config_webdav::WebDavCommand::Show,
            ))) => {}
            _ => panic!("expected config webdav show command"),
        }
    }

    #[test]
    fn parses_config_webdav_set_subcommand() {
        let cli = Cli::parse_from([
            "cc-switch",
            "config",
            "webdav",
            "set",
            "--base-url",
            "https://dav.example.com/root",
            "--username",
            "demo",
            "--password",
            "secret",
            "--enable",
        ]);

        match cli.command {
            Some(Commands::Config(super::commands::config::ConfigCommand::WebDav(
                super::commands::config_webdav::WebDavCommand::Set {
                    base_url,
                    username,
                    password,
                    enable,
                    ..
                },
            ))) => {
                assert_eq!(base_url.as_deref(), Some("https://dav.example.com/root"));
                assert_eq!(username.as_deref(), Some("demo"));
                assert_eq!(password.as_deref(), Some("secret"));
                assert!(enable);
            }
            _ => panic!("expected config webdav set command"),
        }
    }

    #[test]
    fn parses_config_webdav_check_connection_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "config", "webdav", "check-connection"]);

        match cli.command {
            Some(Commands::Config(super::commands::config::ConfigCommand::WebDav(
                super::commands::config_webdav::WebDavCommand::CheckConnection,
            ))) => {}
            _ => panic!("expected config webdav check-connection command"),
        }
    }

    #[test]
    fn parses_env_tools_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "env", "tools"]);

        match cli.command {
            Some(Commands::Env(super::commands::env::EnvCommand::Tools)) => {}
            _ => panic!("expected env tools command"),
        }
    }

    #[test]
    fn parses_skills_repo_enable_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "skills", "repos", "enable", "foo/bar"]);

        match cli.command {
            Some(Commands::Skills(super::commands::skills::SkillsCommand::Repos(
                super::commands::skills::SkillReposCommand::Enable { url },
            ))) => {
                assert_eq!(url, "foo/bar");
            }
            _ => panic!("expected skills repos enable command"),
        }
    }

    #[test]
    fn parses_skills_repo_disable_subcommand() {
        let cli = Cli::parse_from(["cc-switch", "skills", "repos", "disable", "foo/bar"]);

        match cli.command {
            Some(Commands::Skills(super::commands::skills::SkillsCommand::Repos(
                super::commands::skills::SkillReposCommand::Disable { url },
            ))) => {
                assert_eq!(url, "foo/bar");
            }
            _ => panic!("expected skills repos disable command"),
        }
    }
}
