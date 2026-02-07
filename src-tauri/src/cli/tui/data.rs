use std::path::PathBuf;

use indexmap::IndexMap;
use serde_json::Value;

use crate::app_config::{AppType, CommonConfigSnippets, McpServer};
use crate::error::AppError;
use crate::prompt::Prompt;
use crate::provider::Provider;
use crate::services::config::BackupInfo;
use crate::services::{ConfigService, McpService, PromptService, ProviderService, SkillService};
use crate::store::AppState;

#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub id: String,
    pub provider: Provider,
    pub api_url: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProvidersSnapshot {
    pub current_id: String,
    pub rows: Vec<ProviderRow>,
}

#[derive(Debug, Clone)]
pub struct McpRow {
    pub id: String,
    pub server: McpServer,
}

#[derive(Debug, Clone, Default)]
pub struct McpSnapshot {
    pub rows: Vec<McpRow>,
}

#[derive(Debug, Clone)]
pub struct PromptRow {
    pub id: String,
    pub prompt: Prompt,
}

#[derive(Debug, Clone, Default)]
pub struct PromptsSnapshot {
    pub rows: Vec<PromptRow>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigSnapshot {
    pub config_path: PathBuf,
    pub config_dir: PathBuf,
    pub backups: Vec<BackupInfo>,
    pub common_snippet: String,
    pub common_snippets: CommonConfigSnippets,
    pub webdav_sync: Option<crate::settings::WebDavSyncSettings>,
}

#[derive(Debug, Clone, Default)]
pub struct SkillsSnapshot {
    pub installed: Vec<crate::services::skill::InstalledSkill>,
    pub repos: Vec<crate::services::skill::SkillRepo>,
    pub sync_method: crate::services::skill::SyncMethod,
}

#[derive(Debug, Clone, Default)]
pub struct UiData {
    pub providers: ProvidersSnapshot,
    pub mcp: McpSnapshot,
    pub prompts: PromptsSnapshot,
    pub config: ConfigSnapshot,
    pub skills: SkillsSnapshot,
}

pub(crate) fn load_state() -> Result<AppState, AppError> {
    AppState::try_new()
}

impl UiData {
    pub fn load(app_type: &AppType) -> Result<Self, AppError> {
        let state = load_state()?;

        let providers = load_providers(&state, app_type)?;
        let mcp = load_mcp(&state)?;
        let prompts = load_prompts(&state, app_type)?;
        let config = load_config_snapshot(&state, app_type)?;
        let skills = load_skills_snapshot()?;

        Ok(Self {
            providers,
            mcp,
            prompts,
            config,
            skills,
        })
    }
}

fn load_providers(state: &AppState, app_type: &AppType) -> Result<ProvidersSnapshot, AppError> {
    let current_id = ProviderService::current(state, app_type.clone())?;
    let providers = ProviderService::list(state, app_type.clone())?;
    let sorted = sort_providers(&providers);

    let rows = sorted
        .into_iter()
        .map(|(id, provider)| ProviderRow {
            api_url: extract_api_url(&provider.settings_config, app_type),
            is_current: id == current_id,
            id: id.clone(),
            provider,
        })
        .collect::<Vec<_>>();

    Ok(ProvidersSnapshot { current_id, rows })
}

fn sort_providers(providers: &IndexMap<String, Provider>) -> Vec<(String, Provider)> {
    let mut items = providers
        .iter()
        .map(|(id, p)| (id.clone(), p.clone()))
        .collect::<Vec<_>>();

    items.sort_by(|(_, a), (_, b)| match (a.sort_index, b.sort_index) {
        (Some(idx_a), Some(idx_b)) => idx_a.cmp(&idx_b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.created_at.cmp(&b.created_at),
    });

    items
}

fn extract_api_url(settings_config: &Value, app_type: &AppType) -> Option<String> {
    match app_type {
        AppType::Claude => settings_config
            .get("env")?
            .get("ANTHROPIC_BASE_URL")?
            .as_str()
            .map(|s| s.to_string()),
        AppType::Codex => {
            if let Some(config_str) = settings_config.get("config")?.as_str() {
                for line in config_str.lines() {
                    let line = line.trim();
                    if line.starts_with("base_url") {
                        if let Some(url_part) = line.split('=').nth(1) {
                            let url = url_part.trim().trim_matches('"').trim_matches('\'');
                            if !url.is_empty() {
                                return Some(url.to_string());
                            }
                        }
                    }
                }
            }
            None
        }
        AppType::Gemini => settings_config
            .get("env")
            .and_then(|env| {
                env.get("GOOGLE_GEMINI_BASE_URL")
                    .or_else(|| env.get("GEMINI_BASE_URL"))
                    .or_else(|| env.get("BASE_URL"))
            })?
            .as_str()
            .map(|s| s.to_string()),
    }
}

fn load_mcp(state: &AppState) -> Result<McpSnapshot, AppError> {
    let servers = McpService::get_all_servers(state)?;
    let mut rows = servers
        .into_iter()
        .map(|(id, server)| McpRow { id, server })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(McpSnapshot { rows })
}

fn load_prompts(state: &AppState, app_type: &AppType) -> Result<PromptsSnapshot, AppError> {
    let prompts = PromptService::get_prompts(state, app_type.clone())?;
    let mut rows = prompts
        .into_iter()
        .map(|(id, prompt)| PromptRow { id, prompt })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| {
        b.prompt
            .updated_at
            .unwrap_or(0)
            .cmp(&a.prompt.updated_at.unwrap_or(0))
    });

    Ok(PromptsSnapshot { rows })
}

fn load_config_snapshot(state: &AppState, app_type: &AppType) -> Result<ConfigSnapshot, AppError> {
    let config_dir = crate::config::get_app_config_dir();
    let config_path = config_dir.join("cc-switch.db");
    let backups = ConfigService::list_backups(&config_path)?;
    let (common_snippet, common_snippets) = {
        let guard = state.config.read().map_err(AppError::from)?;
        let common_snippets = guard.common_config_snippets.clone();
        let common_snippet = common_snippets.get(app_type).cloned().unwrap_or_default();
        (common_snippet, common_snippets)
    };

    Ok(ConfigSnapshot {
        config_path,
        config_dir,
        backups,
        common_snippet,
        common_snippets,
        webdav_sync: crate::settings::get_webdav_sync_settings(),
    })
}

fn load_skills_snapshot() -> Result<SkillsSnapshot, AppError> {
    Ok(SkillsSnapshot {
        installed: SkillService::list_installed()?,
        repos: SkillService::list_repos()?,
        sync_method: SkillService::get_sync_method()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_api_url_gemini_prefers_google_env_key() {
        let settings = json!({
            "env": {
                "GOOGLE_GEMINI_BASE_URL": "https://google.example",
                "GEMINI_BASE_URL": "https://legacy.example",
                "BASE_URL": "https://fallback.example"
            }
        });

        assert_eq!(
            extract_api_url(&settings, &AppType::Gemini),
            Some("https://google.example".to_string())
        );
    }

    #[test]
    fn extract_api_url_gemini_falls_back_to_legacy_keys() {
        let settings = json!({
            "env": {
                "GEMINI_BASE_URL": "https://legacy.example",
                "BASE_URL": "https://fallback.example"
            }
        });

        assert_eq!(
            extract_api_url(&settings, &AppType::Gemini),
            Some("https://legacy.example".to_string())
        );
    }
}
