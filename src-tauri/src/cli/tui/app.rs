use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::Size;
use serde_json::Value;

use crate::app_config::AppType;
use crate::cli::i18n::texts;
use crate::cli::i18n::Language;

use super::data::UiData;
use super::route::{NavItem, Route};

#[derive(Debug, Clone)]
pub struct FilterState {
    pub active: bool,
    pub buffer: String,
}

impl FilterState {
    pub fn new() -> Self {
        Self {
            active: false,
            buffer: String::new(),
        }
    }

    pub fn query_lower(&self) -> Option<String> {
        let trimmed = self.buffer.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Nav,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub remaining_ticks: u16,
}

impl Toast {
    pub fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            message: message.into(),
            kind,
            remaining_ticks: 12,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Quit,
    ProviderDelete { id: String },
    McpDelete { id: String },
    PromptDelete { id: String },
    ConfigImport { path: String },
    ConfigRestoreBackup { id: String },
    ConfigReset,
    EditorDiscard,
}

#[derive(Debug, Clone)]
pub struct ConfirmOverlay {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSubmit {
    ConfigExport,
    ConfigImport,
    ConfigBackupName,
    McpValidateCommand,
}

#[derive(Debug, Clone)]
pub struct TextInputState {
    pub title: String,
    pub prompt: String,
    pub buffer: String,
    pub submit: TextSubmit,
}

#[derive(Debug, Clone)]
pub struct TextViewState {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: usize,
}

#[derive(Debug, Clone)]
pub enum Overlay {
    None,
    Help,
    Confirm(ConfirmOverlay),
    TextInput(TextInputState),
    BackupPicker {
        selected: usize,
    },
    TextView(TextViewState),
    CommonSnippetView(TextViewState),
    McpAppsPicker {
        id: String,
        name: String,
        selected: usize,
        apps: crate::app_config::McpApps,
    },
    SpeedtestRunning {
        url: String,
    },
    SpeedtestResult {
        url: String,
        lines: Vec<String>,
        scroll: usize,
    },
}

impl Overlay {
    pub fn is_active(&self) -> bool {
        !matches!(self, Overlay::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    Plain,
    Json,
}

#[derive(Debug, Clone)]
pub enum EditorSubmit {
    PromptEdit { id: String },
    ProviderAdd,
    ProviderEdit { id: String },
    McpAdd,
    McpEdit { id: String },
    ConfigCommonSnippet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    View,
    Edit,
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub title: String,
    pub kind: EditorKind,
    pub submit: EditorSubmit,
    pub mode: EditorMode,
    pub lines: Vec<String>,
    pub scroll: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub initial_text: String,
}

impl EditorState {
    pub fn new(
        title: impl Into<String>,
        kind: EditorKind,
        submit: EditorSubmit,
        initial: impl Into<String>,
    ) -> Self {
        let initial_text = initial.into();
        let mut lines = initial_text
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push(String::new());
        }

        Self {
            title: title.into(),
            kind,
            submit,
            mode: EditorMode::View,
            lines,
            scroll: 0,
            cursor_row: 0,
            cursor_col: 0,
            initial_text,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.text().trim_end() != self.initial_text.trim_end()
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    fn line_len_chars(&self, row: usize) -> usize {
        self.lines.get(row).map(|s| s.chars().count()).unwrap_or(0)
    }

    fn ensure_cursor_visible(&mut self, viewport_rows: usize) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);
        self.cursor_col = self.cursor_col.min(self.line_len_chars(self.cursor_row));

        if self.cursor_row < self.scroll {
            self.scroll = self.cursor_row;
        } else if viewport_rows > 0 && self.cursor_row >= self.scroll + viewport_rows {
            self.scroll = self
                .cursor_row
                .saturating_sub(viewport_rows.saturating_sub(1));
        }

        if !self.lines.is_empty() {
            self.scroll = self.scroll.min(self.lines.len() - 1);
        } else {
            self.scroll = 0;
        }
    }

    fn byte_index(line: &str, col: usize) -> usize {
        line.char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len())
    }

    fn insert_char(&mut self, c: char) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);
        let line = &mut self.lines[self.cursor_row];
        let idx = Self::byte_index(line, self.cursor_col);
        line.insert(idx, c);
        self.cursor_col += 1;
    }

    fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    fn newline(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);
        let line = &mut self.lines[self.cursor_row];
        let idx = Self::byte_index(line, self.cursor_col);
        let rest = line.split_off(idx);
        let next_row = self.cursor_row + 1;
        self.lines.insert(next_row, rest);
        self.cursor_row = next_row;
        self.cursor_col = 0;
    }

    fn backspace(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);

        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let start = Self::byte_index(line, self.cursor_col.saturating_sub(1));
            let end = Self::byte_index(line, self.cursor_col);
            if start < end && end <= line.len() {
                line.replace_range(start..end, "");
                self.cursor_col -= 1;
            }
            return;
        }

        if self.cursor_row == 0 {
            return;
        }

        let current = self.lines.remove(self.cursor_row);
        self.cursor_row -= 1;
        let prev = &mut self.lines[self.cursor_row];
        self.cursor_col = prev.chars().count();
        prev.push_str(&current);
    }

    fn delete(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.cursor_row.min(self.lines.len() - 1);

        let line_len = self.line_len_chars(self.cursor_row);
        if self.cursor_col < line_len {
            let line = &mut self.lines[self.cursor_row];
            let start = Self::byte_index(line, self.cursor_col);
            let end = Self::byte_index(line, self.cursor_col + 1);
            if start < end && end <= line.len() {
                line.replace_range(start..end, "");
            }
            return;
        }

        if self.cursor_row + 1 >= self.lines.len() {
            return;
        }

        let next = self.lines.remove(self.cursor_row + 1);
        self.lines[self.cursor_row].push_str(&next);
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    None,
    ReloadData,
    SwitchRoute(Route),
    Quit,
    SetAppType(AppType),

    ProviderSwitch {
        id: String,
    },
    ProviderDelete {
        id: String,
    },
    ProviderSpeedtest {
        url: String,
    },

    McpToggle {
        id: String,
        enabled: bool,
    },
    McpSetApps {
        id: String,
        apps: crate::app_config::McpApps,
    },
    McpDelete {
        id: String,
    },
    McpImport,
    McpValidate {
        command: String,
    },

    PromptActivate {
        id: String,
    },
    PromptDeactivate {
        id: String,
    },
    PromptDelete {
        id: String,
    },

    ConfigExport {
        path: String,
    },
    ConfigImport {
        path: String,
    },
    ConfigBackup {
        name: Option<String>,
    },
    ConfigRestoreBackup {
        id: String,
    },
    ConfigShowFull,
    ConfigValidate,
    ConfigCommonSnippetClear,
    ConfigCommonSnippetApply,
    ConfigReset,

    EditorSubmit {
        submit: EditorSubmit,
        content: String,
    },
    EditorDiscard,

    SetLanguage(Language),
}

#[derive(Debug, Clone)]
pub enum ConfigItem {
    Path,
    ShowFull,
    Export,
    Import,
    Backup,
    Restore,
    Validate,
    CommonSnippet,
    Reset,
}

impl ConfigItem {
    pub const ALL: [ConfigItem; 9] = [
        ConfigItem::Path,
        ConfigItem::ShowFull,
        ConfigItem::Export,
        ConfigItem::Import,
        ConfigItem::Backup,
        ConfigItem::Restore,
        ConfigItem::Validate,
        ConfigItem::CommonSnippet,
        ConfigItem::Reset,
    ];
}

#[derive(Debug, Clone)]
pub struct App {
    pub app_type: AppType,
    pub route: Route,
    pub route_stack: Vec<Route>,
    pub focus: Focus,
    pub nav_idx: usize,

    pub filter: FilterState,
    pub editor: Option<EditorState>,
    pub overlay: Overlay,
    pub toast: Option<Toast>,
    pub should_quit: bool,
    pub last_size: Size,

    pub provider_idx: usize,
    pub mcp_idx: usize,
    pub prompt_idx: usize,
    pub config_idx: usize,
    pub language_idx: usize,
}

impl App {
    pub fn new(app_override: Option<AppType>) -> Self {
        let app_type = app_override.unwrap_or(AppType::Claude);
        Self {
            app_type,
            route: Route::Main,
            route_stack: Vec::new(),
            focus: Focus::Nav,
            nav_idx: 0,
            filter: FilterState::new(),
            editor: None,
            overlay: Overlay::None,
            toast: None,
            should_quit: false,
            last_size: Size::new(0, 0),
            provider_idx: 0,
            mcp_idx: 0,
            prompt_idx: 0,
            config_idx: 0,
            language_idx: 0,
        }
    }

    pub fn nav_item(&self) -> NavItem {
        NavItem::ALL
            .get(self.nav_idx)
            .copied()
            .unwrap_or(NavItem::Main)
    }

    fn nav_item_for_route(route: &Route) -> NavItem {
        match route {
            Route::Main => NavItem::Main,
            Route::Providers | Route::ProviderDetail { .. } => NavItem::Providers,
            Route::Mcp => NavItem::Mcp,
            Route::Prompts => NavItem::Prompts,
            Route::Config => NavItem::Config,
            Route::Settings => NavItem::Settings,
        }
    }

    fn set_route_no_history(&mut self, route: Route) -> Action {
        if route == self.route {
            return Action::None;
        }

        self.route = route.clone();
        self.focus = route_default_focus(&route);

        let nav_item = Self::nav_item_for_route(&route);
        if let Some(idx) = NavItem::ALL.iter().position(|item| *item == nav_item) {
            self.nav_idx = idx;
        }

        if matches!(route, Route::Main) {
            self.route_stack.clear();
            self.focus = Focus::Nav;
        }

        Action::SwitchRoute(route)
    }

    fn push_route_and_switch(&mut self, route: Route) -> Action {
        if route == self.route {
            return Action::None;
        }
        self.route_stack.push(self.route.clone());
        self.set_route_no_history(route)
    }

    fn pop_route_and_switch(&mut self) -> Action {
        if let Some(prev) = self.route_stack.pop() {
            self.set_route_no_history(prev)
        } else {
            self.set_route_no_history(Route::Main)
        }
    }

    pub fn on_tick(&mut self) {
        if let Some(toast) = &mut self.toast {
            if toast.remaining_ticks > 0 {
                toast.remaining_ticks -= 1;
            }
            if toast.remaining_ticks == 0 {
                self.toast = None;
            }
        }
    }

    pub fn push_toast(&mut self, message: impl Into<String>, kind: ToastKind) {
        self.toast = Some(Toast::new(message, kind));
    }

    pub fn open_help(&mut self) {
        self.overlay = Overlay::Help;
    }

    pub fn close_overlay(&mut self) {
        self.overlay = Overlay::None;
    }

    pub fn on_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        self.clamp_selections(data);

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return Action::Quit;
        }

        if self.overlay.is_active() {
            return self.on_overlay_key(key, data);
        }

        if self.editor.is_some() {
            return self.on_editor_key(key);
        }

        if self.filter.active {
            return self.on_filter_key(key);
        }

        // Global actions.
        match key.code {
            KeyCode::Char('?') => {
                self.open_help();
                return Action::None;
            }
            KeyCode::Char('/') => {
                self.filter.active = true;
                return Action::None;
            }
            KeyCode::Char('[') => return Action::SetAppType(cycle_app_type(&self.app_type, -1)),
            KeyCode::Char(']') => return Action::SetAppType(cycle_app_type(&self.app_type, 1)),
            KeyCode::Left => {
                self.focus = Focus::Nav;
                return Action::None;
            }
            KeyCode::Right => {
                if route_has_content_list(&self.route) {
                    self.focus = Focus::Content;
                } else {
                    self.focus = Focus::Nav;
                }
                return Action::None;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                return self.on_back_key();
            }
            _ => {}
        }

        // Navigation + route-specific actions.
        match self.focus {
            Focus::Nav => self.on_nav_key(key),
            Focus::Content => self.on_content_key(key, data),
        }
    }

    fn on_back_key(&mut self) -> Action {
        match self.route {
            Route::Main => {
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: crate::cli::i18n::texts::tui_confirm_exit_title().to_string(),
                    message: crate::cli::i18n::texts::tui_confirm_exit_message().to_string(),
                    action: ConfirmAction::Quit,
                });
                Action::None
            }
            _ => self.pop_route_and_switch(),
        }
    }

    fn on_filter_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.filter.active = false;
                self.filter.buffer.clear();
            }
            KeyCode::Enter => {
                self.filter.active = false;
            }
            KeyCode::Backspace => {
                self.filter.buffer.pop();
            }
            KeyCode::Char(c) => {
                if !c.is_control() {
                    self.filter.buffer.push(c);
                }
            }
            _ => {}
        }
        Action::None
    }

    fn on_nav_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Up => {
                self.nav_idx = self.nav_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.nav_idx = (self.nav_idx + 1).min(NavItem::ALL.len() - 1);
                Action::None
            }
            KeyCode::Enter => {
                if let Some(route) = self.nav_item().to_route() {
                    self.push_route_and_switch(route)
                } else {
                    self.overlay = Overlay::Confirm(ConfirmOverlay {
                        title: crate::cli::i18n::texts::tui_confirm_exit_title().to_string(),
                        message: crate::cli::i18n::texts::tui_confirm_exit_message().to_string(),
                        action: ConfirmAction::Quit,
                    });
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    fn on_content_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        match self.route.clone() {
            Route::Providers => self.on_providers_key(key, data),
            Route::ProviderDetail { id } => self.on_provider_detail_key(key, data, &id),
            Route::Mcp => self.on_mcp_key(key, data),
            Route::Prompts => self.on_prompts_key(key, data),
            Route::Config => self.on_config_key(key, data),
            Route::Settings => self.on_settings_key(key),
            Route::Main => Action::None,
        }
    }

    fn on_providers_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        let visible = visible_providers(&self.filter, data);
        match key.code {
            KeyCode::Up => {
                self.provider_idx = self.provider_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                if !visible.is_empty() {
                    self.provider_idx = (self.provider_idx + 1).min(visible.len() - 1);
                }
                Action::None
            }
            KeyCode::Enter => {
                let Some(row) = visible.get(self.provider_idx) else {
                    return Action::None;
                };
                self.push_route_and_switch(Route::ProviderDetail { id: row.id.clone() })
            }
            KeyCode::Char('a') => {
                let template = serde_json::json!({
                    "id": "",
                    "name": "",
                    "settingsConfig": {}
                });
                let json =
                    serde_json::to_string_pretty(&template).unwrap_or_else(|_| "{}".to_string());
                self.open_editor(
                    texts::tui_provider_add_title(),
                    EditorKind::Json,
                    json,
                    EditorSubmit::ProviderAdd,
                );
                Action::None
            }
            KeyCode::Char('e') => {
                let Some(row) = visible.get(self.provider_idx) else {
                    return Action::None;
                };
                self.open_provider_editor(row);
                Action::None
            }
            KeyCode::Char('s') => {
                let Some(row) = visible.get(self.provider_idx) else {
                    return Action::None;
                };
                if row.is_current {
                    self.push_toast(texts::tui_toast_provider_already_in_use(), ToastKind::Info);
                    return Action::None;
                }
                Action::ProviderSwitch { id: row.id.clone() }
            }
            KeyCode::Char('d') => {
                let Some(row) = visible.get(self.provider_idx) else {
                    return Action::None;
                };
                if row.is_current {
                    self.push_toast(
                        texts::tui_toast_provider_cannot_delete_current(),
                        ToastKind::Warning,
                    );
                    return Action::None;
                }
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: texts::tui_confirm_delete_provider_title().to_string(),
                    message: texts::tui_confirm_delete_provider_message(
                        &row.provider.name,
                        &row.id,
                    ),
                    action: ConfirmAction::ProviderDelete { id: row.id.clone() },
                });
                Action::None
            }
            KeyCode::Char('t') => {
                let Some(row) = visible.get(self.provider_idx) else {
                    return Action::None;
                };
                let Some(url) = row.api_url.clone() else {
                    self.push_toast(texts::tui_toast_provider_no_api_url(), ToastKind::Warning);
                    return Action::None;
                };
                self.overlay = Overlay::SpeedtestRunning { url: url.clone() };
                Action::ProviderSpeedtest { url }
            }
            _ => Action::None,
        }
    }

    fn on_provider_detail_key(&mut self, key: KeyEvent, data: &UiData, id: &str) -> Action {
        let Some(row) = data.providers.rows.iter().find(|p| p.id == id) else {
            return Action::None;
        };

        match key.code {
            KeyCode::Char('e') => {
                self.open_provider_editor(row);
                Action::None
            }
            KeyCode::Enter => Action::None,
            KeyCode::Char('s') => {
                if row.is_current {
                    self.push_toast(texts::tui_toast_provider_already_in_use(), ToastKind::Info);
                    return Action::None;
                }
                Action::ProviderSwitch { id: row.id.clone() }
            }
            KeyCode::Char('t') => {
                let Some(url) = row.api_url.clone() else {
                    self.push_toast(texts::tui_toast_provider_no_api_url(), ToastKind::Warning);
                    return Action::None;
                };
                self.overlay = Overlay::SpeedtestRunning { url: url.clone() };
                Action::ProviderSpeedtest { url }
            }
            _ => Action::None,
        }
    }

    fn open_provider_editor(&mut self, row: &super::data::ProviderRow) {
        let value =
            serde_json::to_value(&row.provider).unwrap_or(Value::Object(Default::default()));
        let display = strip_provider_internal_fields(&value);
        let json = serde_json::to_string_pretty(&display).unwrap_or_else(|_| "{}".to_string());
        self.open_editor(
            format!(
                "{}: {}",
                texts::tui_provider_detail_title(),
                row.provider.name
            ),
            EditorKind::Json,
            json,
            EditorSubmit::ProviderEdit { id: row.id.clone() },
        );
    }

    fn on_mcp_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        let visible = visible_mcp(&self.filter, data);
        match key.code {
            KeyCode::Up => {
                self.mcp_idx = self.mcp_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                if !visible.is_empty() {
                    self.mcp_idx = (self.mcp_idx + 1).min(visible.len() - 1);
                }
                Action::None
            }
            KeyCode::Char('a') => {
                let mut apps = crate::app_config::McpApps::default();
                apps.set_enabled_for(&self.app_type, true);
                let template = crate::app_config::McpServer {
                    id: String::new(),
                    name: String::new(),
                    server: serde_json::json!({
                        "command": "",
                        "args": [],
                    }),
                    apps,
                    description: None,
                    homepage: None,
                    docs: None,
                    tags: vec![],
                };
                let json =
                    serde_json::to_string_pretty(&template).unwrap_or_else(|_| "{}".to_string());
                self.open_editor(
                    texts::tui_mcp_add_title(),
                    EditorKind::Json,
                    json,
                    EditorSubmit::McpAdd,
                );
                Action::None
            }
            KeyCode::Char('e') => {
                let Some(row) = visible.get(self.mcp_idx) else {
                    return Action::None;
                };
                let json =
                    serde_json::to_string_pretty(&row.server).unwrap_or_else(|_| "{}".to_string());
                self.open_editor(
                    texts::tui_mcp_edit_title(&row.server.name),
                    EditorKind::Json,
                    json,
                    EditorSubmit::McpEdit { id: row.id.clone() },
                );
                Action::None
            }
            KeyCode::Char('x') => {
                let Some(row) = visible.get(self.mcp_idx) else {
                    return Action::None;
                };
                let enabled = row.server.apps.is_enabled_for(&self.app_type);
                Action::McpToggle {
                    id: row.id.clone(),
                    enabled: !enabled,
                }
            }
            KeyCode::Char('m') => {
                let Some(row) = visible.get(self.mcp_idx) else {
                    return Action::None;
                };
                self.overlay = Overlay::McpAppsPicker {
                    id: row.id.clone(),
                    name: row.server.name.clone(),
                    selected: app_type_picker_index(&self.app_type),
                    apps: row.server.apps.clone(),
                };
                Action::None
            }
            KeyCode::Char('i') => Action::McpImport,
            KeyCode::Char('v') => {
                self.overlay = Overlay::TextInput(TextInputState {
                    title: texts::tui_input_validate_command_title().to_string(),
                    prompt: texts::tui_input_validate_command_prompt().to_string(),
                    buffer: String::new(),
                    submit: TextSubmit::McpValidateCommand,
                });
                Action::None
            }
            KeyCode::Char('d') => {
                let Some(row) = visible.get(self.mcp_idx) else {
                    return Action::None;
                };
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: texts::tui_confirm_delete_mcp_title().to_string(),
                    message: texts::tui_confirm_delete_mcp_message(&row.server.name, &row.id),
                    action: ConfirmAction::McpDelete { id: row.id.clone() },
                });
                Action::None
            }
            _ => Action::None,
        }
    }

    fn on_prompts_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        let visible = visible_prompts(&self.filter, data);
        match key.code {
            KeyCode::Up => {
                self.prompt_idx = self.prompt_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                if !visible.is_empty() {
                    self.prompt_idx = (self.prompt_idx + 1).min(visible.len() - 1);
                }
                Action::None
            }
            KeyCode::Enter => {
                let Some(row) = visible.get(self.prompt_idx) else {
                    return Action::None;
                };
                self.overlay = Overlay::TextView(TextViewState {
                    title: texts::tui_prompt_title(&row.prompt.name),
                    lines: row.prompt.content.lines().map(|s| s.to_string()).collect(),
                    scroll: 0,
                });
                Action::None
            }
            KeyCode::Char('a') => {
                let Some(row) = visible.get(self.prompt_idx) else {
                    return Action::None;
                };
                Action::PromptActivate { id: row.id.clone() }
            }
            KeyCode::Char('x') => {
                let active = data.prompts.rows.iter().find(|p| p.prompt.enabled);
                let Some(active) = active else {
                    self.push_toast(
                        texts::tui_toast_prompt_no_active_to_deactivate(),
                        ToastKind::Info,
                    );
                    return Action::None;
                };
                Action::PromptDeactivate {
                    id: active.id.clone(),
                }
            }
            KeyCode::Char('d') => {
                let Some(row) = visible.get(self.prompt_idx) else {
                    return Action::None;
                };
                if row.prompt.enabled {
                    self.push_toast(
                        texts::tui_toast_prompt_cannot_delete_active(),
                        ToastKind::Warning,
                    );
                    return Action::None;
                }
                self.overlay = Overlay::Confirm(ConfirmOverlay {
                    title: texts::tui_confirm_delete_prompt_title().to_string(),
                    message: texts::tui_confirm_delete_prompt_message(&row.prompt.name, &row.id),
                    action: ConfirmAction::PromptDelete { id: row.id.clone() },
                });
                Action::None
            }
            KeyCode::Char('e') => {
                let Some(row) = visible.get(self.prompt_idx) else {
                    return Action::None;
                };
                self.open_editor(
                    texts::tui_prompt_title(&row.prompt.name),
                    EditorKind::Plain,
                    row.prompt.content.clone(),
                    EditorSubmit::PromptEdit { id: row.id.clone() },
                );
                Action::None
            }
            _ => Action::None,
        }
    }

    fn on_config_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        let items = visible_config_items(&self.filter);
        match key.code {
            KeyCode::Up => {
                self.config_idx = self.config_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                if !items.is_empty() {
                    self.config_idx = (self.config_idx + 1).min(items.len() - 1);
                }
                Action::None
            }
            KeyCode::Char('e') => {
                let Some(item) = items.get(self.config_idx) else {
                    return Action::None;
                };
                if matches!(item, ConfigItem::CommonSnippet) {
                    let snippet = if data.config.common_snippet.trim().is_empty() {
                        texts::tui_default_common_snippet().to_string()
                    } else {
                        data.config.common_snippet.clone()
                    };
                    self.open_editor(
                        texts::tui_common_snippet_title(self.app_type.as_str()),
                        EditorKind::Json,
                        snippet,
                        EditorSubmit::ConfigCommonSnippet,
                    );
                }
                Action::None
            }
            KeyCode::Enter => {
                let Some(item) = items.get(self.config_idx) else {
                    return Action::None;
                };
                match item {
                    ConfigItem::Path => {
                        self.overlay = Overlay::TextView(TextViewState {
                            title: texts::tui_config_paths_title().to_string(),
                            lines: vec![
                                texts::tui_config_paths_config_file(
                                    &data.config.config_path.display().to_string(),
                                ),
                                texts::tui_config_paths_config_dir(
                                    &data.config.config_dir.display().to_string(),
                                ),
                            ],
                            scroll: 0,
                        });
                        Action::None
                    }
                    ConfigItem::ShowFull => Action::ConfigShowFull,
                    ConfigItem::Export => {
                        self.overlay = Overlay::TextInput(TextInputState {
                            title: texts::tui_config_export_title().to_string(),
                            prompt: texts::tui_config_export_prompt().to_string(),
                            buffer: texts::tui_default_config_export_path().to_string(),
                            submit: TextSubmit::ConfigExport,
                        });
                        Action::None
                    }
                    ConfigItem::Import => {
                        self.overlay = Overlay::TextInput(TextInputState {
                            title: texts::tui_config_import_title().to_string(),
                            prompt: texts::tui_config_import_prompt().to_string(),
                            buffer: texts::tui_default_config_export_path().to_string(),
                            submit: TextSubmit::ConfigImport,
                        });
                        Action::None
                    }
                    ConfigItem::Backup => {
                        self.overlay = Overlay::TextInput(TextInputState {
                            title: texts::tui_config_backup_title().to_string(),
                            prompt: texts::tui_config_backup_prompt().to_string(),
                            buffer: String::new(),
                            submit: TextSubmit::ConfigBackupName,
                        });
                        Action::None
                    }
                    ConfigItem::Restore => {
                        if data.config.backups.is_empty() {
                            self.push_toast(texts::tui_toast_no_backups_found(), ToastKind::Info);
                            return Action::None;
                        }
                        self.overlay = Overlay::BackupPicker { selected: 0 };
                        Action::None
                    }
                    ConfigItem::Validate => Action::ConfigValidate,
                    ConfigItem::CommonSnippet => {
                        let snippet = if data.config.common_snippet.trim().is_empty() {
                            texts::tui_default_common_snippet().to_string()
                        } else {
                            data.config.common_snippet.clone()
                        };
                        self.overlay = Overlay::CommonSnippetView(TextViewState {
                            title: texts::tui_common_snippet_title(self.app_type.as_str()),
                            lines: snippet.lines().map(|s| s.to_string()).collect(),
                            scroll: 0,
                        });
                        Action::None
                    }
                    ConfigItem::Reset => {
                        self.overlay = Overlay::Confirm(ConfirmOverlay {
                            title: texts::tui_config_reset_title().to_string(),
                            message: texts::tui_config_reset_message().to_string(),
                            action: ConfirmAction::ConfigReset,
                        });
                        Action::None
                    }
                }
            }
            _ => Action::None,
        }
    }

    fn on_settings_key(&mut self, key: KeyEvent) -> Action {
        let languages = [Language::English, Language::Chinese];
        match key.code {
            KeyCode::Up => {
                self.language_idx = self.language_idx.saturating_sub(1);
                Action::None
            }
            KeyCode::Down => {
                self.language_idx = (self.language_idx + 1).min(languages.len() - 1);
                Action::None
            }
            KeyCode::Enter => Action::SetLanguage(languages[self.language_idx]),
            _ => Action::None,
        }
    }

    fn on_overlay_key(&mut self, key: KeyEvent, data: &UiData) -> Action {
        if matches!(key.code, KeyCode::Char('e')) {
            if let Overlay::CommonSnippetView(view) = &self.overlay {
                let initial = view.lines.join("\n");
                self.open_editor(
                    texts::tui_common_snippet_title(self.app_type.as_str()),
                    EditorKind::Json,
                    initial,
                    EditorSubmit::ConfigCommonSnippet,
                );
                return Action::None;
            }
        }

        match &mut self.overlay {
            Overlay::None => Action::None,
            Overlay::Help => match key.code {
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                _ => Action::None,
            },
            Overlay::Confirm(confirm) => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let action = match &confirm.action {
                        ConfirmAction::Quit => Action::Quit,
                        ConfirmAction::ProviderDelete { id } => {
                            Action::ProviderDelete { id: id.clone() }
                        }
                        ConfirmAction::McpDelete { id } => Action::McpDelete { id: id.clone() },
                        ConfirmAction::PromptDelete { id } => {
                            Action::PromptDelete { id: id.clone() }
                        }
                        ConfirmAction::ConfigImport { path } => {
                            Action::ConfigImport { path: path.clone() }
                        }
                        ConfirmAction::ConfigRestoreBackup { id } => {
                            Action::ConfigRestoreBackup { id: id.clone() }
                        }
                        ConfirmAction::ConfigReset => Action::ConfigReset,
                        ConfirmAction::EditorDiscard => Action::EditorDiscard,
                    };
                    self.overlay = Overlay::None;
                    action
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                _ => Action::None,
            },
            Overlay::TextInput(input) => match key.code {
                KeyCode::Esc => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                KeyCode::Enter => {
                    let raw = input.buffer.trim().to_string();
                    let submit = input.submit;
                    self.overlay = Overlay::None;
                    match submit {
                        TextSubmit::ConfigExport => {
                            if raw.is_empty() {
                                self.push_toast(
                                    texts::tui_toast_export_path_empty(),
                                    ToastKind::Warning,
                                );
                                return Action::None;
                            }
                            Action::ConfigExport { path: raw }
                        }
                        TextSubmit::ConfigImport => {
                            if raw.is_empty() {
                                self.push_toast(
                                    texts::tui_toast_import_path_empty(),
                                    ToastKind::Warning,
                                );
                                return Action::None;
                            }
                            self.overlay = Overlay::Confirm(ConfirmOverlay {
                                title: texts::tui_config_import_title().to_string(),
                                message: texts::tui_confirm_import_message(&raw),
                                action: ConfirmAction::ConfigImport { path: raw },
                            });
                            Action::None
                        }
                        TextSubmit::ConfigBackupName => {
                            let name = if raw.is_empty() { None } else { Some(raw) };
                            Action::ConfigBackup { name }
                        }
                        TextSubmit::McpValidateCommand => {
                            if raw.is_empty() {
                                self.push_toast(
                                    texts::tui_toast_command_empty(),
                                    ToastKind::Warning,
                                );
                                return Action::None;
                            }
                            Action::McpValidate { command: raw }
                        }
                    }
                }
                KeyCode::Backspace => {
                    input.buffer.pop();
                    Action::None
                }
                KeyCode::Char(c) => {
                    if !c.is_control() {
                        input.buffer.push(c);
                    }
                    Action::None
                }
                _ => Action::None,
            },
            Overlay::BackupPicker { selected } => {
                let backups = &data.config.backups;
                match key.code {
                    KeyCode::Esc => {
                        self.overlay = Overlay::None;
                        Action::None
                    }
                    KeyCode::Up => {
                        *selected = selected.saturating_sub(1);
                        Action::None
                    }
                    KeyCode::Down => {
                        if !backups.is_empty() {
                            *selected = (*selected + 1).min(backups.len() - 1);
                        }
                        Action::None
                    }
                    KeyCode::Enter => {
                        let Some(backup) = backups.get(*selected) else {
                            return Action::None;
                        };
                        let id = backup.id.clone();
                        self.overlay = Overlay::Confirm(ConfirmOverlay {
                            title: texts::tui_confirm_restore_backup_title().to_string(),
                            message: texts::tui_confirm_restore_backup_message(
                                &backup.display_name,
                            ),
                            action: ConfirmAction::ConfigRestoreBackup { id },
                        });
                        Action::None
                    }
                    _ => Action::None,
                }
            }
            Overlay::TextView(view) => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                KeyCode::Up => {
                    view.scroll = view.scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    if !view.lines.is_empty() {
                        view.scroll = (view.scroll + 1).min(view.lines.len() - 1);
                    }
                    Action::None
                }
                _ => Action::None,
            },
            Overlay::CommonSnippetView(view) => match key.code {
                KeyCode::Char('a') => Action::ConfigCommonSnippetApply,
                KeyCode::Char('c') => Action::ConfigCommonSnippetClear,
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                KeyCode::Up => {
                    view.scroll = view.scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    if !view.lines.is_empty() {
                        view.scroll = (view.scroll + 1).min(view.lines.len() - 1);
                    }
                    Action::None
                }
                _ => Action::None,
            },
            Overlay::McpAppsPicker {
                id, selected, apps, ..
            } => match key.code {
                KeyCode::Esc => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                KeyCode::Up => {
                    *selected = selected.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    *selected = (*selected + 1).min(2);
                    Action::None
                }
                KeyCode::Char('x') | KeyCode::Char(' ') => {
                    let app_type = app_type_for_picker_index(*selected);
                    let enabled = apps.is_enabled_for(&app_type);
                    apps.set_enabled_for(&app_type, !enabled);
                    Action::None
                }
                KeyCode::Enter => {
                    let id = id.clone();
                    let next = apps.clone();
                    let unchanged = data
                        .mcp
                        .rows
                        .iter()
                        .find(|row| row.id == id)
                        .map(|row| row.server.apps == next)
                        .unwrap_or(false);

                    self.overlay = Overlay::None;
                    if unchanged {
                        Action::None
                    } else {
                        Action::McpSetApps { id, apps: next }
                    }
                }
                _ => Action::None,
            },
            Overlay::SpeedtestRunning { .. } => match key.code {
                KeyCode::Esc => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                _ => Action::None,
            },
            Overlay::SpeedtestResult { scroll, lines, .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.overlay = Overlay::None;
                    Action::None
                }
                KeyCode::Up => {
                    *scroll = scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    if !lines.is_empty() {
                        *scroll = (*scroll + 1).min(lines.len() - 1);
                    }
                    Action::None
                }
                _ => Action::None,
            },
        }
    }

    pub fn open_editor(
        &mut self,
        title: impl Into<String>,
        kind: EditorKind,
        initial: impl Into<String>,
        submit: EditorSubmit,
    ) {
        self.filter.active = false;
        self.overlay = Overlay::None;
        self.focus = Focus::Content;
        self.editor = Some(EditorState::new(title, kind, submit, initial));
    }

    fn on_editor_key(&mut self, key: KeyEvent) -> Action {
        let viewport = self.editor_viewport_rows();

        let Some(editor) = &mut self.editor else {
            return Action::None;
        };

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('s')) {
            return Action::EditorSubmit {
                submit: editor.submit.clone(),
                content: editor.text(),
            };
        }

        match editor.mode {
            EditorMode::View => match key.code {
                KeyCode::Enter => {
                    editor.mode = EditorMode::Edit;
                    Action::None
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    if editor.is_dirty() {
                        self.overlay = Overlay::Confirm(ConfirmOverlay {
                            title: texts::tui_editor_discard_title().to_string(),
                            message: texts::tui_editor_discard_message().to_string(),
                            action: ConfirmAction::EditorDiscard,
                        });
                        Action::None
                    } else {
                        self.editor = None;
                        Action::None
                    }
                }
                KeyCode::Up => {
                    editor.scroll = editor.scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down => {
                    if !editor.lines.is_empty() {
                        editor.scroll = (editor.scroll + 1).min(editor.lines.len() - 1);
                    }
                    Action::None
                }
                KeyCode::PageUp => {
                    editor.scroll = editor.scroll.saturating_sub(viewport);
                    Action::None
                }
                KeyCode::PageDown => {
                    if !editor.lines.is_empty() {
                        editor.scroll = (editor.scroll + viewport).min(editor.lines.len() - 1);
                    }
                    Action::None
                }
                _ => Action::None,
            },
            EditorMode::Edit => match key.code {
                KeyCode::Esc => {
                    editor.mode = EditorMode::View;
                    Action::None
                }
                KeyCode::Up => {
                    editor.cursor_row = editor.cursor_row.saturating_sub(1);
                    editor.cursor_col = editor
                        .cursor_col
                        .min(editor.line_len_chars(editor.cursor_row));
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Down => {
                    if !editor.lines.is_empty() {
                        editor.cursor_row = (editor.cursor_row + 1).min(editor.lines.len() - 1);
                    }
                    editor.cursor_col = editor
                        .cursor_col
                        .min(editor.line_len_chars(editor.cursor_row));
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Left => {
                    if editor.cursor_col > 0 {
                        editor.cursor_col -= 1;
                    } else if editor.cursor_row > 0 {
                        editor.cursor_row -= 1;
                        editor.cursor_col = editor.line_len_chars(editor.cursor_row);
                    }
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Right => {
                    let line_len = editor.line_len_chars(editor.cursor_row);
                    if editor.cursor_col < line_len {
                        editor.cursor_col += 1;
                    } else if editor.cursor_row + 1 < editor.lines.len() {
                        editor.cursor_row += 1;
                        editor.cursor_col = 0;
                    }
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Home => {
                    editor.cursor_col = 0;
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::End => {
                    editor.cursor_col = editor.line_len_chars(editor.cursor_row);
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::PageUp => {
                    editor.scroll = editor.scroll.saturating_sub(viewport);
                    editor.cursor_row = editor.cursor_row.saturating_sub(viewport);
                    editor.cursor_col = editor
                        .cursor_col
                        .min(editor.line_len_chars(editor.cursor_row));
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::PageDown => {
                    if !editor.lines.is_empty() {
                        editor.scroll = (editor.scroll + viewport).min(editor.lines.len() - 1);
                        editor.cursor_row =
                            (editor.cursor_row + viewport).min(editor.lines.len() - 1);
                        editor.cursor_col = editor
                            .cursor_col
                            .min(editor.line_len_chars(editor.cursor_row));
                    }
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Backspace => {
                    editor.backspace();
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Delete => {
                    editor.delete();
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Enter => {
                    editor.newline();
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Tab => {
                    editor.insert_str("  ");
                    editor.ensure_cursor_visible(viewport);
                    Action::None
                }
                KeyCode::Char(c) => {
                    if !c.is_control() {
                        editor.insert_char(c);
                        editor.ensure_cursor_visible(viewport);
                    }
                    Action::None
                }
                _ => Action::None,
            },
        }
    }

    fn editor_viewport_rows(&self) -> usize {
        // Approximate rows available for the editor's inner text area. The layout is stable:
        // header(3) + footer(1) + content block borders(2) + editor outer borders(2)
        // + editor hint(1) + textarea borders(2) = 11.
        let h = self.last_size.height as usize;
        h.saturating_sub(11).max(1)
    }

    fn clamp_selections(&mut self, data: &UiData) {
        let providers_len = visible_providers(&self.filter, data).len();
        if providers_len == 0 {
            self.provider_idx = 0;
        } else {
            self.provider_idx = self.provider_idx.min(providers_len - 1);
        }

        let mcp_len = visible_mcp(&self.filter, data).len();
        if mcp_len == 0 {
            self.mcp_idx = 0;
        } else {
            self.mcp_idx = self.mcp_idx.min(mcp_len - 1);
        }

        let prompt_len = visible_prompts(&self.filter, data).len();
        if prompt_len == 0 {
            self.prompt_idx = 0;
        } else {
            self.prompt_idx = self.prompt_idx.min(prompt_len - 1);
        }

        let config_len = visible_config_items(&self.filter).len();
        if config_len == 0 {
            self.config_idx = 0;
        } else {
            self.config_idx = self.config_idx.min(config_len - 1);
        }
    }
}

fn route_has_content_list(route: &Route) -> bool {
    matches!(
        route,
        Route::Providers
            | Route::ProviderDetail { .. }
            | Route::Mcp
            | Route::Prompts
            | Route::Config
            | Route::Settings
    )
}

fn route_default_focus(route: &Route) -> Focus {
    match route {
        Route::Main => Focus::Nav,
        _ => Focus::Content,
    }
}

fn visible_providers<'a>(
    filter: &FilterState,
    data: &'a UiData,
) -> Vec<&'a super::data::ProviderRow> {
    let query = filter.query_lower();
    data.providers
        .rows
        .iter()
        .filter(|row| match &query {
            None => true,
            Some(q) => {
                row.provider.name.to_lowercase().contains(q) || row.id.to_lowercase().contains(q)
            }
        })
        .collect()
}

fn visible_mcp<'a>(filter: &FilterState, data: &'a UiData) -> Vec<&'a super::data::McpRow> {
    let query = filter.query_lower();
    data.mcp
        .rows
        .iter()
        .filter(|row| match &query {
            None => true,
            Some(q) => {
                row.server.name.to_lowercase().contains(q) || row.id.to_lowercase().contains(q)
            }
        })
        .collect()
}

fn visible_prompts<'a>(filter: &FilterState, data: &'a UiData) -> Vec<&'a super::data::PromptRow> {
    let query = filter.query_lower();
    data.prompts
        .rows
        .iter()
        .filter(|row| match &query {
            None => true,
            Some(q) => {
                row.prompt.name.to_lowercase().contains(q) || row.id.to_lowercase().contains(q)
            }
        })
        .collect()
}

fn visible_config_items(filter: &FilterState) -> Vec<ConfigItem> {
    let all = ConfigItem::ALL.to_vec();
    let Some(q) = filter.query_lower() else {
        return all;
    };

    all.into_iter()
        .filter(|item| config_item_label(item).to_lowercase().contains(&q))
        .collect()
}

fn config_item_label(item: &ConfigItem) -> &'static str {
    match item {
        ConfigItem::Path => crate::cli::i18n::texts::tui_config_item_show_path(),
        ConfigItem::ShowFull => crate::cli::i18n::texts::tui_config_item_show_full(),
        ConfigItem::Export => crate::cli::i18n::texts::tui_config_item_export(),
        ConfigItem::Import => crate::cli::i18n::texts::tui_config_item_import(),
        ConfigItem::Backup => crate::cli::i18n::texts::tui_config_item_backup(),
        ConfigItem::Restore => crate::cli::i18n::texts::tui_config_item_restore(),
        ConfigItem::Validate => crate::cli::i18n::texts::tui_config_item_validate(),
        ConfigItem::CommonSnippet => crate::cli::i18n::texts::tui_config_item_common_snippet(),
        ConfigItem::Reset => crate::cli::i18n::texts::tui_config_item_reset(),
    }
}

fn cycle_app_type(current: &AppType, dir: i8) -> AppType {
    match (current, dir) {
        (AppType::Claude, 1) => AppType::Codex,
        (AppType::Codex, 1) => AppType::Gemini,
        (AppType::Gemini, 1) => AppType::Claude,
        (AppType::Claude, -1) => AppType::Gemini,
        (AppType::Codex, -1) => AppType::Claude,
        (AppType::Gemini, -1) => AppType::Codex,
        (other, _) => other.clone(),
    }
}

fn app_type_picker_index(app_type: &AppType) -> usize {
    match app_type {
        AppType::Claude => 0,
        AppType::Codex => 1,
        AppType::Gemini => 2,
    }
}

fn app_type_for_picker_index(index: usize) -> AppType {
    match index {
        1 => AppType::Codex,
        2 => AppType::Gemini,
        _ => AppType::Claude,
    }
}

fn should_hide_provider_field(key: &str) -> bool {
    matches!(key, "createdAt" | "updatedAt" | "inFailoverQueue")
}

fn strip_provider_internal_fields(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if should_hide_provider_field(k) {
                    continue;
                }
                out.insert(k.clone(), strip_provider_internal_fields(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(strip_provider_internal_fields).collect())
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use serde_json::json;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn data() -> UiData {
        UiData::default()
    }

    #[test]
    fn config_e_key_opens_common_snippet_editor_when_selected() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Config;
        app.focus = Focus::Content;
        app.config_idx = 7; // ConfigItem::CommonSnippet in ConfigItem::ALL

        let action = app.on_key(key(KeyCode::Char('e')), &data());
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| &e.submit),
            Some(EditorSubmit::ConfigCommonSnippet)
        ));
    }

    #[test]
    fn app_cycles_left_right() {
        let mut app = App::new(Some(AppType::Claude));
        assert!(matches!(
            app.on_key(key(KeyCode::Char(']')), &data()),
            Action::SetAppType(AppType::Codex)
        ));
        assert!(matches!(
            app.on_key(key(KeyCode::Char('[')), &data()),
            Action::SetAppType(AppType::Gemini)
        ));
    }

    #[test]
    fn q_from_main_opens_exit_confirm_overlay() {
        let mut app = App::new(Some(AppType::Claude));
        assert_eq!(app.route, Route::Main);
        app.on_key(key(KeyCode::Char('q')), &data());
        assert!(matches!(app.overlay, Overlay::Confirm(_)));
    }

    #[test]
    fn filter_mode_updates_buffer_and_exits() {
        let mut app = App::new(Some(AppType::Claude));
        assert_eq!(app.filter.active, false);
        app.on_key(key(KeyCode::Char('/')), &data());
        assert_eq!(app.filter.active, true);
        app.on_key(key(KeyCode::Char('a')), &data());
        app.on_key(key(KeyCode::Char('b')), &data());
        assert_eq!(app.filter.buffer, "ab");
        app.on_key(key(KeyCode::Backspace), &data());
        assert_eq!(app.filter.buffer, "a");
        app.on_key(key(KeyCode::Enter), &data());
        assert_eq!(app.filter.active, false);
    }

    #[test]
    fn tab_key_is_noop() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Nav;

        let data = UiData::default();
        let action = app.on_key(key(KeyCode::Tab), &data);
        assert!(matches!(action, Action::None));
        assert_eq!(app.focus, Focus::Nav);
    }

    #[test]
    fn provider_json_editor_hides_internal_fields() {
        let original = json!({
            "id": "p1",
            "name": "demo",
            "settingsConfig": {
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "secret-token",
                    "FOO": "bar"
                }
            },
            "createdAt": 123
        });

        let display = strip_provider_internal_fields(&original);
        assert!(display.get("createdAt").is_none());
        assert_eq!(
            display["settingsConfig"]["env"]["ANTHROPIC_AUTH_TOKEN"],
            "secret-token"
        );
    }

    #[test]
    fn providers_enter_key_opens_detail() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.providers.rows.push(super::super::data::ProviderRow {
            id: "p1".to_string(),
            provider: crate::provider::Provider::with_id(
                "p1".to_string(),
                "Provider One".to_string(),
                json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com"}}),
                None,
            ),
            api_url: Some("https://example.com".to_string()),
            is_current: false,
        });

        let action = app.on_key(key(KeyCode::Enter), &data);
        assert!(matches!(
            action,
            Action::SwitchRoute(Route::ProviderDetail { id }) if id == "p1"
        ));
    }

    #[test]
    fn providers_s_key_triggers_switch_action() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.providers.rows.push(super::super::data::ProviderRow {
            id: "p1".to_string(),
            provider: crate::provider::Provider::with_id(
                "p1".to_string(),
                "Provider One".to_string(),
                json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com"}}),
                None,
            ),
            api_url: Some("https://example.com".to_string()),
            is_current: false,
        });

        let action = app.on_key(key(KeyCode::Char('s')), &data);
        assert!(matches!(action, Action::ProviderSwitch { id } if id == "p1"));
    }

    #[test]
    fn provider_detail_s_key_triggers_switch_action_and_enter_is_noop() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::ProviderDetail {
            id: "p1".to_string(),
        };
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.providers.rows.push(super::super::data::ProviderRow {
            id: "p1".to_string(),
            provider: crate::provider::Provider::with_id(
                "p1".to_string(),
                "Provider One".to_string(),
                json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com"}}),
                None,
            ),
            api_url: Some("https://example.com".to_string()),
            is_current: false,
        });

        let enter_action = app.on_key(key(KeyCode::Enter), &data);
        assert!(matches!(enter_action, Action::None));

        let action = app.on_key(key(KeyCode::Char('s')), &data);
        assert!(matches!(action, Action::ProviderSwitch { id } if id == "p1"));
    }

    #[test]
    fn mcp_x_key_toggles_current_app() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Mcp;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.mcp.rows.push(super::super::data::McpRow {
            id: "m1".to_string(),
            server: crate::app_config::McpServer {
                id: "m1".to_string(),
                name: "Server".to_string(),
                server: json!({}),
                apps: crate::app_config::McpApps::default(),
                description: None,
                homepage: None,
                docs: None,
                tags: vec![],
            },
        });

        let action = app.on_key(key(KeyCode::Char('x')), &data);
        assert!(matches!(
            action,
            Action::McpToggle {
                id,
                enabled: true
            } if id == "m1"
        ));
    }

    #[test]
    fn mcp_a_opens_add_editor() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Mcp;
        app.focus = Focus::Content;

        let data = UiData::default();
        let action = app.on_key(key(KeyCode::Char('a')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| &e.submit),
            Some(EditorSubmit::McpAdd)
        ));
    }

    #[test]
    fn mcp_m_opens_apps_picker_overlay() {
        let mut app = App::new(Some(AppType::Codex));
        app.route = Route::Mcp;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.mcp.rows.push(super::super::data::McpRow {
            id: "m1".to_string(),
            server: crate::app_config::McpServer {
                id: "m1".to_string(),
                name: "Server".to_string(),
                server: json!({}),
                apps: crate::app_config::McpApps::default(),
                description: None,
                homepage: None,
                docs: None,
                tags: vec![],
            },
        });

        let action = app.on_key(key(KeyCode::Char('m')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            &app.overlay,
            Overlay::McpAppsPicker {
                id,
                name,
                selected: 1,
                ..
            } if id == "m1" && name == "Server"
        ));
    }

    #[test]
    fn mcp_apps_picker_x_toggles_selected_app_and_enter_emits_action() {
        let mut app = App::new(Some(AppType::Codex));
        app.route = Route::Mcp;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.mcp.rows.push(super::super::data::McpRow {
            id: "m1".to_string(),
            server: crate::app_config::McpServer {
                id: "m1".to_string(),
                name: "Server".to_string(),
                server: json!({}),
                apps: crate::app_config::McpApps::default(),
                description: None,
                homepage: None,
                docs: None,
                tags: vec![],
            },
        });

        app.on_key(key(KeyCode::Char('m')), &data);

        let action = app.on_key(key(KeyCode::Char('x')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            &app.overlay,
            Overlay::McpAppsPicker { apps, .. } if apps.codex
        ));

        let action = app.on_key(key(KeyCode::Enter), &data);
        assert!(matches!(
            action,
            Action::McpSetApps { id, apps } if id == "m1" && apps.codex && !apps.claude && !apps.gemini
        ));
    }

    #[test]
    fn mcp_e_opens_edit_editor() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Mcp;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.mcp.rows.push(super::super::data::McpRow {
            id: "m1".to_string(),
            server: crate::app_config::McpServer {
                id: "m1".to_string(),
                name: "Server".to_string(),
                server: json!({"command":"foo","args":[]}),
                apps: crate::app_config::McpApps::default(),
                description: None,
                homepage: None,
                docs: None,
                tags: vec![],
            },
        });

        let action = app.on_key(key(KeyCode::Char('e')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| &e.submit),
            Some(EditorSubmit::McpEdit { id }) if id == "m1"
        ));
    }

    #[test]
    fn prompts_a_key_triggers_activate_action() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Prompts;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.prompts.rows.push(super::super::data::PromptRow {
            id: "pr1".to_string(),
            prompt: crate::prompt::Prompt {
                id: "pr1".to_string(),
                name: "My Prompt".to_string(),
                content: "Hello".to_string(),
                description: None,
                enabled: false,
                created_at: None,
                updated_at: None,
            },
        });

        let action = app.on_key(key(KeyCode::Char('a')), &data);
        assert!(matches!(action, Action::PromptActivate { id } if id == "pr1"));
    }

    #[test]
    fn back_from_provider_detail_returns_to_providers() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.providers.rows.push(super::super::data::ProviderRow {
            id: "p1".to_string(),
            provider: crate::provider::Provider::with_id(
                "p1".to_string(),
                "Provider One".to_string(),
                json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com"}}),
                None,
            ),
            api_url: Some("https://example.com".to_string()),
            is_current: false,
        });

        assert!(matches!(
            app.on_key(key(KeyCode::Enter), &data),
            Action::SwitchRoute(Route::ProviderDetail { .. })
        ));
        assert!(matches!(app.route, Route::ProviderDetail { .. }));

        assert!(matches!(
            app.on_key(key(KeyCode::Esc), &data),
            Action::SwitchRoute(Route::Providers)
        ));
        assert_eq!(app.route, Route::Providers);
    }

    #[test]
    fn config_common_snippet_overlay_supports_edit_clear_apply_actions() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Config;
        app.focus = Focus::Content;
        app.config_idx = ConfigItem::ALL
            .iter()
            .position(|item| matches!(item, ConfigItem::CommonSnippet))
            .expect("CommonSnippet missing from ConfigItem::ALL");

        let data = UiData::default();
        app.on_key(key(KeyCode::Enter), &data);
        assert!(matches!(app.overlay, Overlay::CommonSnippetView(_)));

        assert!(matches!(
            app.on_key(key(KeyCode::Char('a')), &data),
            Action::ConfigCommonSnippetApply
        ));
        assert!(matches!(
            app.on_key(key(KeyCode::Char('c')), &data),
            Action::ConfigCommonSnippetClear
        ));

        let action = app.on_key(key(KeyCode::Char('e')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| e.kind),
            Some(EditorKind::Json)
        ));
    }

    #[test]
    fn prompts_e_opens_editor_and_ctrl_s_submits() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Prompts;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.prompts.rows.push(super::super::data::PromptRow {
            id: "pr1".to_string(),
            prompt: crate::prompt::Prompt {
                id: "pr1".to_string(),
                name: "Demo".to_string(),
                content: "hello".to_string(),
                description: None,
                enabled: false,
                created_at: None,
                updated_at: None,
            },
        });

        let action = app.on_key(key(KeyCode::Char('e')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| &e.submit),
            Some(EditorSubmit::PromptEdit { id }) if id == "pr1"
        ));

        let submit = app.on_key(ctrl(KeyCode::Char('s')), &data);
        assert!(matches!(
            submit,
            Action::EditorSubmit {
                submit: EditorSubmit::PromptEdit { .. },
                content
            } if content.contains("hello")
        ));
    }

    #[test]
    fn providers_e_opens_editor_and_ctrl_s_submits() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;

        let mut data = UiData::default();
        data.providers.rows.push(super::super::data::ProviderRow {
            id: "p1".to_string(),
            provider: crate::provider::Provider::with_id(
                "p1".to_string(),
                "Provider One".to_string(),
                json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com"}}),
                None,
            ),
            api_url: Some("https://example.com".to_string()),
            is_current: false,
        });

        let action = app.on_key(key(KeyCode::Char('e')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| &e.submit),
            Some(EditorSubmit::ProviderEdit { id }) if id == "p1"
        ));

        let submit = app.on_key(ctrl(KeyCode::Char('s')), &data);
        assert!(matches!(
            submit,
            Action::EditorSubmit {
                submit: EditorSubmit::ProviderEdit { .. },
                content
            } if content.contains("\"id\"") && content.contains("Provider One")
        ));
    }

    #[test]
    fn providers_a_opens_add_editor() {
        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;

        let data = UiData::default();
        let action = app.on_key(key(KeyCode::Char('a')), &data);
        assert!(matches!(action, Action::None));
        assert!(matches!(
            app.editor.as_ref().map(|e| &e.submit),
            Some(EditorSubmit::ProviderAdd)
        ));

        let submit = app.on_key(ctrl(KeyCode::Char('s')), &data);
        assert!(matches!(
            submit,
            Action::EditorSubmit {
                submit: EditorSubmit::ProviderAdd,
                ..
            }
        ));
    }
}
