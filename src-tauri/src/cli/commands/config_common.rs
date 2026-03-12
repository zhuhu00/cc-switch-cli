use clap::Subcommand;
use std::fs;
use std::path::Path;

use crate::app_config::AppType;
use crate::cli::i18n::texts;
use crate::cli::ui::{highlight, info, success};
use crate::error::AppError;
use crate::store::AppState;

#[derive(Subcommand, Debug, Clone)]
pub enum CommonConfigCommand {
    /// Show current common config snippet
    Show,
    /// Set common config snippet (JSON object)
    Set {
        /// JSON object string (e.g. '{"env":{"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":1}}')
        #[arg(long, conflicts_with = "file")]
        json: Option<String>,

        /// Read JSON object from file
        #[arg(long, conflicts_with = "json")]
        file: Option<std::path::PathBuf>,

        /// Apply to current provider immediately
        #[arg(long)]
        apply: bool,
    },
    /// Clear common config snippet
    Clear {
        /// Apply to current provider immediately
        #[arg(long)]
        apply: bool,
    },
}

pub fn execute(cmd: CommonConfigCommand, app_type: AppType) -> Result<(), AppError> {
    match cmd {
        CommonConfigCommand::Show => show(app_type),
        CommonConfigCommand::Set { json, file, apply } => {
            set(app_type, json.as_deref(), file.as_deref(), apply)
        }
        CommonConfigCommand::Clear { apply } => clear(app_type, apply),
    }
}

fn get_state() -> Result<AppState, AppError> {
    AppState::try_new()
}

fn show(app_type: AppType) -> Result<(), AppError> {
    let state = get_state()?;
    let config = state.config.read()?;
    let snippet = config.common_config_snippets.get(&app_type).cloned();

    println!("{}", highlight(texts::config_common_snippet_title()));
    println!("{}", "=".repeat(50));
    println!("App: {}", app_type.as_str());
    println!();

    match snippet {
        Some(snippet) if !snippet.trim().is_empty() => println!("{}", snippet),
        _ => println!("{}", info(texts::config_common_snippet_none_set())),
    }

    Ok(())
}

fn set(
    app_type: AppType,
    json_text: Option<&str>,
    file: Option<&Path>,
    apply: bool,
) -> Result<(), AppError> {
    let raw = if let Some(text) = json_text {
        text.to_string()
    } else if let Some(path) = file {
        fs::read_to_string(path).map_err(|e| AppError::io(path, e))?
    } else {
        return Err(AppError::InvalidInput(
            texts::config_common_snippet_require_json_or_file().to_string(),
        ));
    };

    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| AppError::InvalidInput(texts::tui_toast_invalid_json(&e.to_string())))?;
    if !value.is_object() {
        return Err(AppError::InvalidInput(
            texts::common_config_snippet_not_object().to_string(),
        ));
    }

    let pretty = serde_json::to_string_pretty(&value)
        .map_err(|e| AppError::Message(texts::failed_to_serialize_json(&e.to_string())))?;

    let state = get_state()?;
    {
        let mut config = state.config.write()?;
        config.common_config_snippets.set(&app_type, Some(pretty));
    }
    state.save()?;

    println!(
        "{}",
        success(&texts::config_common_snippet_set_for_app(app_type.as_str()))
    );

    if apply {
        apply_to_current(&state, app_type)?;
    } else {
        println!(
            "{}",
            info("Tip: run `cc-switch provider switch <id>` to re-apply settings to the live config.")
        );
    }

    Ok(())
}

fn clear(app_type: AppType, apply: bool) -> Result<(), AppError> {
    let state = get_state()?;
    {
        let mut config = state.config.write()?;
        config.common_config_snippets.set(&app_type, None);
    }
    state.save()?;

    println!(
        "{}",
        success(&format!(
            "✓ Common config snippet cleared for app '{}'",
            app_type.as_str()
        ))
    );

    if apply {
        apply_to_current(&state, app_type)?;
    } else {
        println!(
            "{}",
            info("Tip: run `cc-switch provider switch <id>` to re-apply settings to the live config.")
        );
    }

    Ok(())
}

fn apply_to_current(state: &AppState, app_type: AppType) -> Result<(), AppError> {
    use crate::services::ProviderService;

    let current_id = ProviderService::current(state, app_type.clone())?;
    if current_id.trim().is_empty() {
        println!("{}", info("No current provider; nothing to apply."));
        return Ok(());
    }

    ProviderService::switch(state, app_type, &current_id)?;
    println!("{}", success("✓ Applied to live config."));
    Ok(())
}
