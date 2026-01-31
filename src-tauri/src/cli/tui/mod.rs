mod app;
mod data;
mod route;
mod terminal;
mod theme;
mod ui;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::{self, KeyEventKind};
use serde_json::Value;

use crate::app_config::{AppType, MultiAppConfig};
use crate::cli::i18n::{set_language, texts};
use crate::error::AppError;
use crate::provider::Provider;
use crate::services::{ConfigService, EndpointLatency, McpService, PromptService, ProviderService};

use app::{Action, App, EditorSubmit, Overlay, TextViewState, ToastKind};
use data::{load_state, UiData};
use terminal::{PanicRestoreHookGuard, TuiTerminal};

fn command_lookup_name(raw: &str) -> Option<&str> {
    raw.split_whitespace().next()
}

enum SpeedtestMsg {
    Finished {
        url: String,
        result: Result<Vec<EndpointLatency>, String>,
    },
}

struct SpeedtestSystem {
    req_tx: mpsc::Sender<String>,
    result_rx: mpsc::Receiver<SpeedtestMsg>,
    _handle: std::thread::JoinHandle<()>,
}

pub fn run(app_override: Option<AppType>) -> Result<(), AppError> {
    let _panic_hook = PanicRestoreHookGuard::install();
    let mut terminal = TuiTerminal::new()?;
    let mut app = App::new(app_override);
    let mut data = UiData::load(&app.app_type)?;

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    let speedtest = match start_speedtest_system() {
        Ok(system) => Some(system),
        Err(err) => {
            app.push_toast(
                texts::tui_toast_speedtest_unavailable(&err.to_string()),
                ToastKind::Warning,
            );
            None
        }
    };

    loop {
        app.last_size = terminal.size()?;
        terminal.draw(|f| ui::render(f, &app, &data))?;

        // Handle async speedtest results (non-blocking).
        if let Some(speedtest) = speedtest.as_ref() {
            while let Ok(msg) = speedtest.result_rx.try_recv() {
                handle_speedtest_msg(&mut app, msg);
            }
        }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout).map_err(|e| AppError::Message(e.to_string()))? {
            match event::read().map_err(|e| AppError::Message(e.to_string()))? {
                event::Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let action = app.on_key(key, &data);
                    if let Err(err) = handle_action(
                        &mut terminal,
                        &mut app,
                        &mut data,
                        speedtest.as_ref().map(|s| &s.req_tx),
                        action,
                    ) {
                        if matches!(
                            &err,
                            AppError::Localized { key, .. } if *key == "tui_terminal_error"
                        ) {
                            return Err(err);
                        }
                        app.push_toast(err.to_string(), ToastKind::Error);
                    }
                }
                event::Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_speedtest_msg(app: &mut App, msg: SpeedtestMsg) {
    match msg {
        SpeedtestMsg::Finished { url, result } => match result {
            Ok(rows) => {
                let mut lines = vec![texts::tui_speedtest_line_url(&url), String::new()];
                for row in rows {
                    let latency = row
                        .latency
                        .map(texts::tui_latency_ms)
                        .unwrap_or_else(|| texts::tui_na().to_string());
                    let status = row
                        .status
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| texts::tui_na().to_string());
                    let err = row.error.unwrap_or_default();

                    lines.push(texts::tui_speedtest_line_latency(&latency));
                    lines.push(texts::tui_speedtest_line_status(&status));
                    if !err.trim().is_empty() {
                        lines.push(texts::tui_speedtest_line_error(&err));
                    }
                }

                // Only force-open the result modal if the user hasn't closed it.
                match &app.overlay {
                    Overlay::SpeedtestRunning { url: running_url } if running_url == &url => {
                        app.overlay = Overlay::SpeedtestResult {
                            url,
                            lines,
                            scroll: 0,
                        };
                    }
                    _ => {
                        app.push_toast(texts::tui_toast_speedtest_finished(), ToastKind::Success);
                    }
                }
            }
            Err(err) => {
                app.push_toast(texts::tui_toast_speedtest_failed(&err), ToastKind::Error);
                if matches!(&app.overlay, Overlay::SpeedtestRunning { url: running_url } if running_url == &url)
                {
                    app.overlay = Overlay::None;
                }
            }
        },
    }
}

fn handle_action(
    _terminal: &mut TuiTerminal,
    app: &mut App,
    data: &mut UiData,
    speedtest_req_tx: Option<&mpsc::Sender<String>>,
    action: Action,
) -> Result<(), AppError> {
    match action {
        Action::None => Ok(()),
        Action::ReloadData => {
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::SetAppType(next) => {
            let next_data = UiData::load(&next)?;
            app.app_type = next;
            *data = next_data;
            Ok(())
        }
        Action::SwitchRoute(route) => {
            app.route = route;
            Ok(())
        }
        Action::Quit => {
            app.should_quit = true;
            Ok(())
        }
        Action::EditorDiscard => {
            app.editor = None;
            Ok(())
        }
        Action::EditorSubmit { submit, content } => match submit {
            EditorSubmit::PromptEdit { id } => {
                let state = load_state()?;
                let prompts = PromptService::get_prompts(&state, app.app_type.clone())?;
                let Some(mut prompt) = prompts.get(&id).cloned() else {
                    app.push_toast(texts::tui_toast_prompt_not_found(&id), ToastKind::Error);
                    return Ok(());
                };

                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                prompt.content = content;
                prompt.updated_at = Some(timestamp);

                if let Err(err) =
                    PromptService::upsert_prompt(&state, app.app_type.clone(), &id, prompt)
                {
                    app.push_toast(err.to_string(), ToastKind::Error);
                    return Ok(());
                }

                app.editor = None;
                app.push_toast(texts::tui_toast_prompt_edit_finished(), ToastKind::Success);
                *data = UiData::load(&app.app_type)?;
                Ok(())
            }
            EditorSubmit::ProviderAdd => {
                let provider: Provider = match serde_json::from_str(&content) {
                    Ok(p) => p,
                    Err(e) => {
                        app.push_toast(
                            texts::tui_toast_invalid_json(&e.to_string()),
                            ToastKind::Error,
                        );
                        return Ok(());
                    }
                };

                if provider.id.trim().is_empty() || provider.name.trim().is_empty() {
                    app.push_toast(
                        texts::tui_toast_provider_add_missing_fields(),
                        ToastKind::Warning,
                    );
                    return Ok(());
                }

                let state = load_state()?;
                match ProviderService::add(&state, app.app_type.clone(), provider) {
                    Ok(true) => {
                        app.editor = None;
                        app.push_toast(
                            texts::tui_toast_provider_add_finished(),
                            ToastKind::Success,
                        );
                        *data = UiData::load(&app.app_type)?;
                    }
                    Ok(false) => {
                        app.push_toast(texts::tui_toast_provider_add_failed(), ToastKind::Error);
                    }
                    Err(err) => {
                        app.push_toast(err.to_string(), ToastKind::Error);
                    }
                }

                Ok(())
            }
            EditorSubmit::ProviderEdit { id } => {
                let mut provider: Provider = match serde_json::from_str(&content) {
                    Ok(p) => p,
                    Err(e) => {
                        app.push_toast(
                            texts::tui_toast_invalid_json(&e.to_string()),
                            ToastKind::Error,
                        );
                        return Ok(());
                    }
                };
                provider.id = id.clone();

                if provider.name.trim().is_empty() {
                    app.push_toast(texts::tui_toast_provider_missing_name(), ToastKind::Warning);
                    return Ok(());
                }

                let state = load_state()?;
                if let Err(err) = ProviderService::update(&state, app.app_type.clone(), provider) {
                    app.push_toast(err.to_string(), ToastKind::Error);
                    return Ok(());
                }

                app.editor = None;
                app.push_toast(
                    texts::tui_toast_provider_edit_finished(),
                    ToastKind::Success,
                );
                *data = UiData::load(&app.app_type)?;
                Ok(())
            }
            EditorSubmit::McpAdd => {
                let server: crate::app_config::McpServer = match serde_json::from_str(&content) {
                    Ok(s) => s,
                    Err(e) => {
                        app.push_toast(
                            texts::tui_toast_invalid_json(&e.to_string()),
                            ToastKind::Error,
                        );
                        return Ok(());
                    }
                };

                if server.id.trim().is_empty() || server.name.trim().is_empty() {
                    app.push_toast(texts::tui_toast_mcp_missing_fields(), ToastKind::Warning);
                    return Ok(());
                }

                let state = load_state()?;
                if let Err(err) = McpService::upsert_server(&state, server) {
                    app.push_toast(err.to_string(), ToastKind::Error);
                    return Ok(());
                }

                app.editor = None;
                app.push_toast(texts::tui_toast_mcp_upserted(), ToastKind::Success);
                *data = UiData::load(&app.app_type)?;
                Ok(())
            }
            EditorSubmit::McpEdit { id } => {
                let mut server: crate::app_config::McpServer = match serde_json::from_str(&content)
                {
                    Ok(s) => s,
                    Err(e) => {
                        app.push_toast(
                            texts::tui_toast_invalid_json(&e.to_string()),
                            ToastKind::Error,
                        );
                        return Ok(());
                    }
                };
                server.id = id.clone();

                if server.name.trim().is_empty() {
                    app.push_toast(texts::tui_toast_mcp_missing_fields(), ToastKind::Warning);
                    return Ok(());
                }

                let state = load_state()?;
                if let Err(err) = McpService::upsert_server(&state, server) {
                    app.push_toast(err.to_string(), ToastKind::Error);
                    return Ok(());
                }

                app.editor = None;
                app.push_toast(texts::tui_toast_mcp_upserted(), ToastKind::Success);
                *data = UiData::load(&app.app_type)?;
                Ok(())
            }
            EditorSubmit::ConfigCommonSnippet => {
                let edited = content.trim().to_string();
                let (next_snippet, toast) = if edited.is_empty() {
                    (None, texts::common_config_snippet_cleared())
                } else {
                    let value: Value = match serde_json::from_str(&edited) {
                        Ok(v) => v,
                        Err(e) => {
                            app.push_toast(
                                texts::common_config_snippet_invalid_json(&e.to_string()),
                                ToastKind::Error,
                            );
                            return Ok(());
                        }
                    };

                    if !value.is_object() {
                        app.push_toast(texts::common_config_snippet_not_object(), ToastKind::Error);
                        return Ok(());
                    }

                    let pretty = match serde_json::to_string_pretty(&value) {
                        Ok(v) => v,
                        Err(e) => {
                            app.push_toast(
                                texts::failed_to_serialize_json(&e.to_string()),
                                ToastKind::Error,
                            );
                            return Ok(());
                        }
                    };

                    (Some(pretty), texts::common_config_snippet_saved())
                };

                let state = load_state()?;
                {
                    let mut cfg = match state.config.write().map_err(AppError::from) {
                        Ok(cfg) => cfg,
                        Err(err) => {
                            app.push_toast(err.to_string(), ToastKind::Error);
                            return Ok(());
                        }
                    };
                    cfg.common_config_snippets.set(&app.app_type, next_snippet);
                }
                if let Err(err) = state.save() {
                    app.push_toast(err.to_string(), ToastKind::Error);
                    return Ok(());
                }

                app.editor = None;
                app.push_toast(toast, ToastKind::Success);
                *data = UiData::load(&app.app_type)?;

                // Bring the user back to the snippet preview overlay.
                let snippet = if data.config.common_snippet.trim().is_empty() {
                    texts::tui_default_common_snippet().to_string()
                } else {
                    data.config.common_snippet.clone()
                };
                app.overlay = Overlay::CommonSnippetView(TextViewState {
                    title: texts::tui_common_snippet_title(app.app_type.as_str()),
                    lines: snippet.lines().map(|s| s.to_string()).collect(),
                    scroll: 0,
                });
                Ok(())
            }
        },

        Action::ProviderSwitch { id } => {
            let state = load_state()?;
            ProviderService::switch(&state, app.app_type.clone(), &id)?;
            if !crate::sync_policy::should_sync_live(&app.app_type) {
                let mut message =
                    texts::tui_toast_live_sync_skipped_uninitialized(app.app_type.as_str());
                message.push(' ');
                message.push_str(texts::restart_note());
                app.push_toast(message, ToastKind::Warning);
            } else {
                app.push_toast(texts::restart_note(), ToastKind::Success);
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::ProviderDelete { id } => {
            let state = load_state()?;
            ProviderService::delete(&state, app.app_type.clone(), &id)?;
            app.push_toast(texts::tui_toast_provider_deleted(), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        // Provider editing is handled via the in-app editor (EditorSubmit).
        Action::ProviderSpeedtest { url } => {
            let Some(tx) = speedtest_req_tx else {
                if matches!(&app.overlay, Overlay::SpeedtestRunning { url: running_url } if running_url == &url)
                {
                    app.overlay = Overlay::None;
                }
                app.push_toast(texts::tui_toast_speedtest_disabled(), ToastKind::Warning);
                return Ok(());
            };

            if let Err(err) = tx.send(url.clone()) {
                if matches!(&app.overlay, Overlay::SpeedtestRunning { url: running_url } if running_url == &url)
                {
                    app.overlay = Overlay::None;
                }
                app.push_toast(
                    texts::tui_toast_speedtest_request_failed(&err.to_string()),
                    ToastKind::Error,
                );
            }
            Ok(())
        }

        Action::McpToggle { id, enabled } => {
            let state = load_state()?;
            McpService::toggle_app(&state, &id, app.app_type.clone(), enabled)?;
            if !crate::sync_policy::should_sync_live(&app.app_type) {
                let mut message = texts::tui_toast_mcp_updated().to_string();
                message.push(' ');
                message.push_str(&texts::tui_toast_live_sync_skipped_uninitialized(
                    app.app_type.as_str(),
                ));
                app.push_toast(message, ToastKind::Warning);
            } else {
                app.push_toast(texts::tui_toast_mcp_updated(), ToastKind::Success);
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::McpSetApps { id, apps } => {
            let Some(before) = data
                .mcp
                .rows
                .iter()
                .find(|row| row.id == id)
                .map(|row| row.server.apps.clone())
            else {
                app.push_toast(texts::tui_toast_mcp_server_not_found(), ToastKind::Warning);
                return Ok(());
            };

            let state = load_state()?;
            let mut skipped: Vec<&str> = Vec::new();
            let mut changed = false;

            for app_type in [AppType::Claude, AppType::Codex, AppType::Gemini] {
                let next_enabled = apps.is_enabled_for(&app_type);
                if before.is_enabled_for(&app_type) == next_enabled {
                    continue;
                }

                changed = true;
                McpService::toggle_app(&state, &id, app_type.clone(), next_enabled)?;
                if !crate::sync_policy::should_sync_live(&app_type) {
                    skipped.push(app_type.as_str());
                }
            }

            if !changed {
                // Shouldn't happen because the picker avoids emitting an action when unchanged.
                app.push_toast(texts::tui_toast_mcp_updated(), ToastKind::Success);
            } else if skipped.is_empty() {
                app.push_toast(texts::tui_toast_mcp_updated(), ToastKind::Success);
            } else {
                app.push_toast(
                    texts::tui_toast_mcp_updated_live_sync_skipped(&skipped),
                    ToastKind::Warning,
                );
            }

            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::McpDelete { id } => {
            let state = load_state()?;
            let deleted = McpService::delete_server(&state, &id)?;
            if deleted {
                app.push_toast(texts::tui_toast_mcp_server_deleted(), ToastKind::Success);
            } else {
                app.push_toast(texts::tui_toast_mcp_server_not_found(), ToastKind::Warning);
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::McpImport => {
            let state = load_state()?;
            let count = match app.app_type {
                AppType::Claude => McpService::import_from_claude(&state)?,
                AppType::Codex => McpService::import_from_codex(&state)?,
                AppType::Gemini => McpService::import_from_gemini(&state)?,
            };
            app.push_toast(texts::tui_toast_mcp_imported(count), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::McpValidate { command } => {
            let Some(bin) = command_lookup_name(&command) else {
                app.push_toast(texts::tui_toast_command_empty(), ToastKind::Warning);
                return Ok(());
            };

            if which::which(bin).is_ok() {
                app.push_toast(
                    texts::tui_toast_command_available_in_path(bin),
                    ToastKind::Success,
                );
            } else {
                app.push_toast(
                    texts::tui_toast_command_not_found_in_path(bin),
                    ToastKind::Warning,
                );
            }
            Ok(())
        }

        Action::PromptActivate { id } => {
            let state = load_state()?;
            PromptService::enable_prompt(&state, app.app_type.clone(), &id)?;
            app.push_toast(texts::tui_toast_prompt_activated(), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::PromptDeactivate { id } => {
            let state = load_state()?;
            PromptService::disable_prompt(&state, app.app_type.clone(), &id)?;
            app.push_toast(texts::tui_toast_prompt_deactivated(), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::PromptDelete { id } => {
            let state = load_state()?;
            PromptService::delete_prompt(&state, app.app_type.clone(), &id)?;
            app.push_toast(texts::tui_toast_prompt_deleted(), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }

        Action::ConfigExport { path } => {
            let target = PathBuf::from(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
            }
            ConfigService::export_config_to_path(&target)?;
            app.push_toast(
                texts::tui_toast_exported_to(&target.display().to_string()),
                ToastKind::Success,
            );
            Ok(())
        }
        Action::ConfigShowFull => {
            let title = data
                .config
                .config_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| texts::tui_default_config_filename().to_string());
            let content = std::fs::read_to_string(&data.config.config_path)
                .unwrap_or_else(|e| texts::tui_error_failed_to_read_config(&e.to_string()));
            app.overlay = Overlay::TextView(TextViewState {
                title,
                lines: content.lines().map(|s| s.to_string()).collect(),
                scroll: 0,
            });
            Ok(())
        }
        Action::ConfigImport { path } => {
            let source = PathBuf::from(path);
            if !source.exists() {
                return Err(AppError::Message(texts::tui_error_import_file_not_found(
                    &source.display().to_string(),
                )));
            }
            let state = load_state()?;
            let backup_id = ConfigService::import_config_from_path(&source, &state)?;
            if backup_id.is_empty() {
                app.push_toast(texts::tui_toast_imported_config(), ToastKind::Success);
            } else {
                app.push_toast(
                    texts::tui_toast_imported_with_backup(&backup_id),
                    ToastKind::Success,
                );
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::ConfigBackup { name } => {
            let config_path = crate::config::get_app_config_path();
            let id = ConfigService::create_backup(&config_path, name)?;
            if id.is_empty() {
                app.push_toast(texts::tui_toast_no_config_file_to_backup(), ToastKind::Info);
            } else {
                app.push_toast(texts::tui_toast_backup_created(&id), ToastKind::Success);
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::ConfigRestoreBackup { id } => {
            let state = load_state()?;
            let pre_backup = ConfigService::restore_from_backup_id(&id, &state)?;
            if pre_backup.is_empty() {
                app.push_toast(texts::tui_toast_restored_from_backup(), ToastKind::Success);
            } else {
                app.push_toast(
                    texts::tui_toast_restored_with_pre_backup(&pre_backup),
                    ToastKind::Success,
                );
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::ConfigValidate => {
            let config_path = crate::config::get_app_config_path();
            if !config_path.exists() {
                app.push_toast(
                    texts::tui_toast_config_file_does_not_exist(),
                    ToastKind::Warning,
                );
                return Ok(());
            }

            match MultiAppConfig::load() {
                Ok(cfg) => {
                    let claude_count = cfg
                        .get_manager(&AppType::Claude)
                        .map(|m| m.providers.len())
                        .unwrap_or(0);
                    let codex_count = cfg
                        .get_manager(&AppType::Codex)
                        .map(|m| m.providers.len())
                        .unwrap_or(0);
                    let gemini_count = cfg
                        .get_manager(&AppType::Gemini)
                        .map(|m| m.providers.len())
                        .unwrap_or(0);
                    let mcp_count = cfg.mcp.servers.as_ref().map(|s| s.len()).unwrap_or(0);

                    let lines = vec![
                        texts::tui_config_validation_ok().to_string(),
                        String::new(),
                        texts::tui_config_validation_provider_count(
                            AppType::Claude.as_str(),
                            claude_count,
                        ),
                        texts::tui_config_validation_provider_count(
                            AppType::Codex.as_str(),
                            codex_count,
                        ),
                        texts::tui_config_validation_provider_count(
                            AppType::Gemini.as_str(),
                            gemini_count,
                        ),
                        texts::tui_config_validation_mcp_servers(mcp_count),
                    ];
                    app.overlay = Overlay::TextView(TextViewState {
                        title: texts::tui_config_validation_title().to_string(),
                        lines,
                        scroll: 0,
                    });
                    app.push_toast(texts::tui_toast_validation_passed(), ToastKind::Success);
                    Ok(())
                }
                Err(err) => {
                    app.overlay = Overlay::TextView(TextViewState {
                        title: texts::tui_config_validation_failed_title().to_string(),
                        lines: vec![err.to_string()],
                        scroll: 0,
                    });
                    Err(err)
                }
            }
        }
        Action::ConfigCommonSnippetClear => {
            let state = load_state()?;
            {
                let mut cfg = state.config.write().map_err(AppError::from)?;
                cfg.common_config_snippets.set(&app.app_type, None);
            }
            state.save()?;

            app.push_toast(texts::common_config_snippet_cleared(), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            refresh_common_snippet_overlay(app, data);
            Ok(())
        }
        Action::ConfigCommonSnippetApply => {
            let state = load_state()?;
            let current_id = ProviderService::current(&state, app.app_type.clone())?;
            if current_id.trim().is_empty() {
                app.push_toast(
                    texts::common_config_snippet_no_current_provider(),
                    ToastKind::Info,
                );
                return Ok(());
            }
            ProviderService::switch(&state, app.app_type.clone(), &current_id)?;
            app.push_toast(texts::common_config_snippet_applied(), ToastKind::Success);
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }
        Action::ConfigReset => {
            let config_path = crate::config::get_app_config_path();
            let backup_id = ConfigService::create_backup(&config_path, None)?;
            if config_path.exists() {
                std::fs::remove_file(&config_path).map_err(|e| AppError::io(&config_path, e))?;
            }
            let _ = MultiAppConfig::load()?;
            if backup_id.is_empty() {
                app.push_toast(
                    texts::tui_toast_config_reset_to_defaults(),
                    ToastKind::Success,
                );
            } else {
                app.push_toast(
                    texts::tui_toast_config_reset_with_backup(&backup_id),
                    ToastKind::Success,
                );
            }
            *data = UiData::load(&app.app_type)?;
            Ok(())
        }

        Action::SetLanguage(lang) => {
            set_language(lang)?;
            app.push_toast(texts::language_changed(), ToastKind::Success);
            Ok(())
        }
    }
}

fn refresh_common_snippet_overlay(app: &mut App, data: &UiData) {
    let Overlay::CommonSnippetView(view) = &mut app.overlay else {
        return;
    };

    let snippet = if data.config.common_snippet.trim().is_empty() {
        texts::tui_default_common_snippet().to_string()
    } else {
        data.config.common_snippet.clone()
    };

    view.title = texts::tui_common_snippet_title(app.app_type.as_str());
    view.lines = snippet.lines().map(|s| s.to_string()).collect();
    view.scroll = 0;
}

fn start_speedtest_system() -> Result<SpeedtestSystem, AppError> {
    let (result_tx, result_rx) = mpsc::channel::<SpeedtestMsg>();
    let (req_tx, req_rx) = mpsc::channel::<String>();

    let handle = std::thread::Builder::new()
        .name("cc-switch-speedtest".to_string())
        .spawn(move || speedtest_worker_loop(req_rx, result_tx))
        .map_err(|e| AppError::IoContext {
            context: "failed to spawn speedtest worker thread".to_string(),
            source: e,
        })?;

    Ok(SpeedtestSystem {
        req_tx,
        result_rx,
        _handle: handle,
    })
}

fn speedtest_worker_loop(rx: mpsc::Receiver<String>, tx: mpsc::Sender<SpeedtestMsg>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let err = e.to_string();
            while let Ok(url) = rx.recv() {
                let _ = tx.send(SpeedtestMsg::Finished {
                    url,
                    result: Err(err.clone()),
                });
            }
            return;
        }
    };

    while let Ok(mut url) = rx.recv() {
        for next in rx.try_iter() {
            url = next;
        }

        let result = rt
            .block_on(async {
                crate::services::SpeedtestService::test_endpoints(vec![url.clone()], None).await
            })
            .map_err(|e| e.to_string());

        let _ = tx.send(SpeedtestMsg::Finished { url, result });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_lookup_name_extracts_first_token() {
        assert_eq!(super::command_lookup_name("node --version"), Some("node"));
        assert_eq!(super::command_lookup_name("  rg   -n foo "), Some("rg"));
    }

    #[test]
    fn command_lookup_name_rejects_empty_or_whitespace() {
        assert_eq!(super::command_lookup_name(""), None);
        assert_eq!(super::command_lookup_name("   "), None);
    }
}
