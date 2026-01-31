use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        TableState, Wrap,
    },
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app_config::AppType;
use crate::cli::i18n::texts;

use super::{
    app::{App, ConfigItem, EditorMode, Focus, Overlay, ToastKind},
    data::{McpRow, ProviderRow, UiData},
    route::{NavItem, Route},
    theme::theme_for,
};

fn pane_border_style(app: &App, pane: Focus, theme: &super::theme::Theme) -> Style {
    if app.focus == pane {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dim)
    }
}

fn selection_style(theme: &super::theme::Theme) -> Style {
    if theme.no_color {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    }
}

fn inactive_chip_style(theme: &super::theme::Theme) -> Style {
    if theme.no_color {
        Style::default()
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    }
}

fn active_chip_style(theme: &super::theme::Theme) -> Style {
    if theme.no_color {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    }
}

fn pad2(s: &str) -> String {
    format!("  {s}")
}

fn dracula_comment(theme: &super::theme::Theme) -> Style {
    if theme.no_color {
        Style::default().fg(theme.dim)
    } else {
        Style::default().fg(Color::Rgb(98, 114, 164)) // #6272a4
    }
}

fn dracula_cyan(theme: &super::theme::Theme) -> Style {
    if theme.no_color {
        Style::default()
    } else {
        Style::default().fg(Color::Rgb(139, 233, 253)) // #8be9fd
    }
}

fn dracula_dark(theme: &super::theme::Theme) -> Style {
    if theme.no_color {
        Style::default().fg(theme.dim)
    } else {
        Style::default().fg(Color::Rgb(68, 71, 90)) // #44475a
    }
}

fn strip_trailing_colon(label: &str) -> &str {
    label.trim_end_matches([':', '：'])
}

fn pad_to_display_width(label: &str, width: usize) -> String {
    let clean = strip_trailing_colon(label);
    let w = UnicodeWidthStr::width(clean);
    if w >= width {
        clean.to_string()
    } else {
        format!("{clean}{}", " ".repeat(width - w))
    }
}

fn kv_line<'a>(
    theme: &super::theme::Theme,
    label: &'a str,
    label_width: usize,
    value_spans: Vec<Span<'a>>,
) -> Line<'a> {
    let mut spans = vec![
        Span::raw(" "), // internal padding: keep content away from │
        Span::styled(
            pad_to_display_width(label, label_width),
            dracula_comment(theme).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": "),
    ];
    spans.extend(value_spans);
    Line::from(spans)
}

fn highlight_symbol(theme: &super::theme::Theme) -> &'static str {
    if theme.no_color {
        texts::tui_highlight_symbol()
    } else {
        " "
    }
}

fn key_bar_line(theme: &super::theme::Theme, items: &[(&str, &str)]) -> Line<'static> {
    if theme.no_color {
        let mut parts = Vec::new();
        for (k, v) in items {
            parts.push(format!("{k}={v}"));
        }
        return Line::raw(parts.join("  "));
    }

    let base = inactive_chip_style(theme);
    let key = base.add_modifier(Modifier::BOLD);

    let mut spans: Vec<Span<'static>> = vec![Span::styled(" ", base)];
    for (idx, (k, v)) in items.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled("  ", base));
        }
        spans.push(Span::styled((*k).to_string(), key));
        spans.push(Span::styled(" ", base));
        spans.push(Span::styled((*v).to_string(), base));
    }
    spans.push(Span::styled(" ", base));
    Line::from(spans)
}

fn render_key_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &super::theme::Theme,
    items: &[(&str, &str)],
) {
    frame.render_widget(
        Paragraph::new(key_bar_line(theme, items))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_key_bar_center(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &super::theme::Theme,
    items: &[(&str, &str)],
) {
    frame.render_widget(
        Paragraph::new(key_bar_line(theme, items))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn inset_left(area: Rect, left: u16) -> Rect {
    if area.width <= left {
        return Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        };
    }
    Rect {
        x: area.x + left,
        y: area.y,
        width: area.width - left,
        height: area.height,
    }
}

pub fn render(frame: &mut Frame<'_>, app: &App, data: &UiData) {
    let theme = theme_for(&app.app_type);

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme.dim));
    frame.render_widget(header_block.clone(), root[0]);
    render_header(frame, app, data, header_block.inner(root[0]), &theme);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)])
        .split(root[1]);

    render_nav(frame, app, body[0], &theme);
    render_content(frame, app, data, body[1], &theme);
    render_footer(frame, app, root[2], &theme);

    render_overlay(frame, app, data, &theme);
}

fn render_header(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12),
            Constraint::Min(0),
            Constraint::Length(28),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![Span::styled(
        format!("  {}", texts::tui_app_title()),
        if theme.no_color {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        },
    )]))
    .alignment(Alignment::Left);
    frame.render_widget(title, chunks[0]);

    let selected = match app.app_type {
        AppType::Claude => 0,
        AppType::Codex => 1,
        AppType::Gemini => 2,
    };
    let tabs_line = Line::from(vec![
        Span::styled(
            format!("  {}  ", AppType::Claude.as_str()),
            if selected == 0 {
                active_chip_style(theme)
            } else {
                inactive_chip_style(theme)
            },
        ),
        Span::raw(" "),
        Span::styled(
            format!("  {}  ", AppType::Codex.as_str()),
            if selected == 1 {
                active_chip_style(theme)
            } else {
                inactive_chip_style(theme)
            },
        ),
        Span::raw(" "),
        Span::styled(
            format!("  {}  ", AppType::Gemini.as_str()),
            if selected == 2 {
                active_chip_style(theme)
            } else {
                inactive_chip_style(theme)
            },
        ),
    ]);
    let tabs = Paragraph::new(tabs_line).alignment(Alignment::Center);
    frame.render_widget(tabs, chunks[1]);

    let current_provider = data
        .providers
        .rows
        .iter()
        .find(|p| p.is_current)
        .map(|p| p.provider.name.as_str())
        .unwrap_or(texts::none());

    let provider_text = format!(
        "{}: {}",
        strip_trailing_colon(texts::provider_label()),
        current_provider
    );
    let badge_content = format!("  {}  ", provider_text);
    let badge_width = (UnicodeWidthStr::width(badge_content.as_str()) as u16).min(chunks[2].width);
    let badge_area = Rect {
        x: chunks[2].x + chunks[2].width.saturating_sub(badge_width),
        y: chunks[2].y,
        width: badge_width,
        height: 1,
    };

    let badge_style = if theme.no_color {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::raw(badge_content)))
            .alignment(Alignment::Center)
            .style(badge_style)
            .block(Block::default().borders(Borders::NONE)),
        badge_area,
    );
}

fn nav_label(item: NavItem) -> &'static str {
    match item {
        NavItem::Main => texts::menu_home(),
        NavItem::Providers => texts::menu_manage_providers(),
        NavItem::Mcp => texts::menu_manage_mcp(),
        NavItem::Prompts => texts::menu_manage_prompts(),
        NavItem::Config => texts::menu_manage_config(),
        NavItem::Settings => texts::menu_settings(),
        NavItem::Exit => texts::menu_exit(),
    }
}

fn render_nav(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &super::theme::Theme) {
    fn split_nav_label(label: &'static str) -> (&'static str, &'static str) {
        if let Some((icon, rest)) = label.split_once(' ') {
            (icon, rest)
        } else {
            ("", label)
        }
    }

    let rows = NavItem::ALL.iter().map(|item| {
        let (icon, text) = split_nav_label(nav_label(*item));
        Row::new(vec![Cell::from(icon), Cell::from(text)])
    });

    let table = Table::new(rows, [Constraint::Length(3), Constraint::Min(10)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(pane_border_style(app, Focus::Nav, theme))
                .title(texts::tui_nav_title()),
        )
        .row_highlight_style(selection_style(theme))
        .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.nav_idx));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_content(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let (filter_area, content_area) = split_filter_area(area, app);

    if let Some(filter_area) = filter_area {
        render_filter_bar(frame, app, filter_area, theme);
    }

    if let Some(editor) = &app.editor {
        render_editor(frame, app, editor, content_area, theme);
        return;
    }

    match &app.route {
        Route::Main => render_main(frame, app, data, content_area, theme),
        Route::Providers => render_providers(frame, app, data, content_area, theme),
        Route::ProviderDetail { id } => {
            render_provider_detail(frame, app, data, content_area, theme, id)
        }
        Route::Mcp => render_mcp(frame, app, data, content_area, theme),
        Route::Prompts => render_prompts(frame, app, data, content_area, theme),
        Route::Config => render_config(frame, app, data, content_area, theme),
        Route::Settings => render_settings(frame, app, content_area, theme),
    }
}

fn render_editor(
    frame: &mut Frame<'_>,
    app: &App,
    editor: &super::app::EditorState,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(editor.title.clone());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let keys = match editor.mode {
        EditorMode::View => vec![
            ("Enter", texts::tui_key_edit_mode()),
            ("↑↓", texts::tui_key_scroll()),
            ("Ctrl+S", texts::tui_key_save()),
            ("Esc", texts::tui_key_close()),
        ],
        EditorMode::Edit => vec![
            ("↑↓←→", texts::tui_key_move()),
            ("Ctrl+S", texts::tui_key_save()),
            ("Esc", texts::tui_key_exit_edit()),
        ],
    };
    render_key_bar(frame, chunks[0], theme, &keys);

    let field_title = match editor.kind {
        super::app::EditorKind::Json => texts::tui_editor_json_field_title(),
        super::app::EditorKind::Plain => texts::tui_editor_text_field_title(),
    };
    let field_border_style = if matches!(editor.mode, EditorMode::Edit) {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dim)
    };

    let field = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(field_border_style)
        .title(format!("-{}", field_title));

    frame.render_widget(field.clone(), chunks[1]);
    let field_inner = field.inner(chunks[1]);

    let height = field_inner.height as usize;
    let start = editor.scroll.min(editor.lines.len());
    let end = (start + height).min(editor.lines.len());
    let shown = editor.lines[start..end]
        .iter()
        .map(|s| Line::raw(s.clone()))
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(shown).wrap(Wrap { trim: false }),
        field_inner,
    );

    if matches!(editor.mode, EditorMode::Edit) {
        let row_in_view = editor.cursor_row.saturating_sub(editor.scroll);
        if row_in_view < height {
            let x =
                field_inner.x + (editor.cursor_col as u16).min(field_inner.width.saturating_sub(1));
            let y = field_inner.y + row_in_view as u16;
            frame.set_cursor_position((x, y));
        }
    }

    // Key bar already shows mode-specific shortcuts.
}

fn split_filter_area(area: Rect, app: &App) -> (Option<Rect>, Rect) {
    let show = app.filter.active || !app.filter.buffer.trim().is_empty();
    if !show {
        return (None, area);
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    (Some(chunks[0]), chunks[1])
}

fn render_filter_bar(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &super::theme::Theme) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(if app.filter.active {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.dim)
        })
        .title(texts::tui_filter_title());

    frame.render_widget(outer.clone(), area);

    let inner = outer.inner(area);
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(if app.filter.active {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.dim)
        })
        .title(texts::tui_filter_icon());

    let input_inner = input_block.inner(inner);
    frame.render_widget(input_block, inner);
    let available = input_inner.width as usize;
    let full = app.filter.buffer.clone();
    let cursor = full.chars().count();
    let start = cursor.saturating_sub(available);
    let visible = full.chars().skip(start).take(available).collect::<String>();

    frame.render_widget(
        Paragraph::new(Line::from(Span::raw(visible))).wrap(Wrap { trim: false }),
        input_inner,
    );

    if app.filter.active {
        let cursor_x = input_inner.x + (cursor.saturating_sub(start) as u16);
        let cursor_y = input_inner.y;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_main(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let current_provider = data
        .providers
        .rows
        .iter()
        .find(|p| p.is_current)
        .map(|p| p.provider.name.as_str())
        .unwrap_or(texts::none());

    let mcp_enabled = data
        .mcp
        .rows
        .iter()
        .filter(|s| s.server.apps.is_enabled_for(&app.app_type))
        .count();

    let prompts_active = data
        .prompts
        .rows
        .iter()
        .find(|p| p.prompt.enabled)
        .map(|p| p.prompt.name.as_str())
        .unwrap_or(texts::none());

    let api_url = data
        .providers
        .rows
        .iter()
        .find(|p| p.is_current)
        .and_then(|p| p.api_url.as_deref())
        .unwrap_or(texts::tui_na());

    let is_online = api_url != texts::tui_na();
    let provider_status = if theme.no_color {
        String::new()
    } else if is_online {
        format!(" ({})", texts::tui_home_status_online())
    } else {
        format!(" ({})", texts::tui_home_status_offline())
    };
    let status_dot = if theme.no_color {
        if is_online {
            "● "
        } else {
            "○ "
        }
    } else {
        "● "
    };
    let status_dot_style = if theme.no_color {
        Style::default()
    } else if is_online {
        Style::default().fg(theme.ok)
    } else {
        Style::default().fg(theme.warn)
    };

    let label_width = 14;
    let value_style = dracula_cyan(theme);
    let provider_name_style = if theme.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let provider_value_spans = vec![
        Span::styled(status_dot, status_dot_style),
        Span::styled(current_provider.to_string(), provider_name_style),
        Span::raw(provider_status),
    ];

    let connection_lines = vec![
        kv_line(
            theme,
            texts::provider_label(),
            label_width,
            provider_value_spans,
        ),
        kv_line(
            theme,
            texts::tui_label_api_url(),
            label_width,
            vec![Span::styled(api_url.to_string(), value_style)],
        ),
        kv_line(
            theme,
            texts::mcp_servers_label(),
            label_width,
            vec![Span::styled(
                format!(
                    "{}/{} {}",
                    mcp_enabled,
                    data.mcp.rows.len(),
                    texts::tui_label_mcp_servers_active()
                ),
                value_style,
            )],
        ),
    ];

    let context_lines = vec![
        kv_line(
            theme,
            texts::prompts_label(),
            label_width,
            vec![Span::styled(prompts_active.to_string(), value_style)],
        ),
        kv_line(
            theme,
            texts::tui_config_title(),
            label_width,
            vec![Span::styled(
                data.config.config_path.display().to_string(),
                value_style,
            )],
        ),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::welcome_title());

    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(inner);

    frame.render_widget(block, area);

    let top = inset_left(chunks[0], 2);
    let top_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(top);

    let card_border = Style::default().fg(theme.dim);

    // Connection card.
    frame.render_widget(
        Paragraph::new(connection_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain)
                    .border_style(card_border)
                    .title(format!(" {} ", texts::tui_home_section_connection())),
            )
            .wrap(Wrap { trim: false }),
        top_chunks[1],
    );

    // Session card.
    frame.render_widget(
        Paragraph::new(context_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain)
                    .border_style(card_border)
                    .title(format!(" {} ", texts::tui_home_section_context())),
            )
            .wrap(Wrap { trim: false }),
        top_chunks[3],
    );

    let logo_style = if theme.no_color {
        dracula_dark(theme)
    } else {
        dracula_dark(theme)
    };
    let logo_lines = texts::tui_home_ascii_logo()
        .lines()
        .map(|s| Line::from(Span::styled(s.to_string(), logo_style)))
        .collect::<Vec<_>>();

    let logo_height = (logo_lines.len() as u16).min(chunks[1].height);
    let logo_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(logo_height),
            Constraint::Length(1),
        ])
        .split(chunks[1]);

    frame.render_widget(
        Paragraph::new(logo_lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        logo_chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Line::raw(texts::tui_main_hint()))
            .alignment(Alignment::Center)
            .style(dracula_dark(theme).add_modifier(Modifier::ITALIC)),
        logo_chunks[2],
    );
}

fn provider_rows_filtered<'a>(app: &App, data: &'a UiData) -> Vec<&'a ProviderRow> {
    let query = app.filter.query_lower();
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

fn render_providers(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let header_style = Style::default().fg(theme.dim).add_modifier(Modifier::BOLD);
    let table_style = Style::default();

    let selected_style = if theme.no_color {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::menu_manage_providers());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        render_key_bar_center(
            frame,
            chunks[0],
            theme,
            &[
                ("Enter", texts::tui_key_details()),
                ("s", texts::tui_key_switch()),
                ("a", texts::tui_key_add()),
                ("e", texts::tui_key_edit()),
                ("d", texts::tui_key_delete()),
                ("t", texts::tui_key_speedtest()),
            ],
        );
    }

    let visible = provider_rows_filtered(app, data);

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(texts::header_name()),
        Cell::from(texts::tui_header_api_url()),
    ])
    .style(header_style);

    let rows = visible.iter().map(|row| {
        let marker = if row.is_current {
            texts::tui_marker_active()
        } else {
            texts::tui_marker_inactive()
        };
        let api = row.api_url.as_deref().unwrap_or(texts::tui_na());
        Row::new(vec![
            Cell::from(marker),
            Cell::from(row.provider.name.clone()),
            Cell::from(api),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ],
    )
    .header(header)
    .style(table_style)
    .block(Block::default().borders(Borders::NONE))
    .row_highlight_style(selected_style)
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.provider_idx));

    frame.render_stateful_widget(table, inset_left(chunks[1], 2), &mut state);
}

fn render_provider_detail(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
    id: &str,
) {
    let Some(row) = data.providers.rows.iter().find(|p| p.id == id) else {
        frame.render_widget(
            Paragraph::new(texts::tui_provider_not_found()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain)
                    .border_style(pane_border_style(app, Focus::Content, theme))
                    .title(texts::tui_provider_title()),
            ),
            area,
        );
        return;
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::tui_provider_detail_title());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        render_key_bar_center(
            frame,
            chunks[0],
            theme,
            &[
                ("s", texts::tui_key_switch()),
                ("e", texts::tui_key_edit()),
                ("t", texts::tui_key_speedtest()),
            ],
        );
    }

    let mut lines = vec![
        Line::from(vec![
            Span::styled(texts::tui_label_id(), Style::default().fg(theme.accent)),
            Span::raw(": "),
            Span::raw(row.id.clone()),
        ]),
        Line::from(vec![
            Span::styled(texts::header_name(), Style::default().fg(theme.accent)),
            Span::raw(": "),
            Span::raw(row.provider.name.clone()),
        ]),
        Line::raw(""),
    ];

    if let Some(url) = row.api_url.as_deref() {
        lines.push(Line::from(vec![
            Span::styled(
                texts::tui_label_api_url(),
                Style::default().fg(theme.accent),
            ),
            Span::raw(": "),
            Span::raw(url),
        ]));
    }

    if matches!(app.app_type, crate::app_config::AppType::Claude) {
        if let Some(env) = row
            .provider
            .settings_config
            .get("env")
            .and_then(|v| v.as_object())
        {
            let api_key = env
                .get("ANTHROPIC_AUTH_TOKEN")
                .or_else(|| env.get("ANTHROPIC_API_KEY"))
                .and_then(|v| v.as_str())
                .map(mask_api_key)
                .unwrap_or_else(|| texts::tui_na().to_string());
            let base_url = env
                .get("ANTHROPIC_BASE_URL")
                .and_then(|v| v.as_str())
                .unwrap_or(texts::tui_na());

            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled(
                    texts::tui_label_base_url(),
                    Style::default().fg(theme.accent),
                ),
                Span::raw(": "),
                Span::raw(base_url),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    texts::tui_label_api_key(),
                    Style::default().fg(theme.accent),
                ),
                Span::raw(": "),
                Span::raw(api_key),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false }),
        inset_left(chunks[1], 2),
    );
}

fn mcp_rows_filtered<'a>(app: &App, data: &'a UiData) -> Vec<&'a McpRow> {
    let query = app.filter.query_lower();
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

fn render_mcp(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let visible = mcp_rows_filtered(app, data);

    let header = Row::new(vec![
        Cell::from(pad2(texts::header_name())),
        Cell::from(crate::app_config::AppType::Claude.as_str()),
        Cell::from(crate::app_config::AppType::Codex.as_str()),
        Cell::from(crate::app_config::AppType::Gemini.as_str()),
    ])
    .style(Style::default().fg(theme.dim).add_modifier(Modifier::BOLD));

    let rows = visible.iter().map(|row| {
        Row::new(vec![
            Cell::from(pad2(&row.server.name)),
            Cell::from(if row.server.apps.claude {
                texts::tui_marker_active()
            } else {
                texts::tui_marker_inactive()
            }),
            Cell::from(if row.server.apps.codex {
                texts::tui_marker_active()
            } else {
                texts::tui_marker_inactive()
            }),
            Cell::from(if row.server.apps.gemini {
                texts::tui_marker_active()
            } else {
                texts::tui_marker_inactive()
            }),
        ])
    });

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::menu_manage_mcp());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        render_key_bar_center(
            frame,
            chunks[0],
            theme,
            &[
                ("x", texts::tui_key_toggle()),
                ("m", texts::tui_key_apps()),
                ("a", texts::tui_key_add()),
                ("e", texts::tui_key_edit()),
                ("i", texts::tui_key_import()),
                ("v", texts::tui_key_validate()),
                ("d", texts::tui_key_delete()),
            ],
        );
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(55),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::NONE))
    .row_highlight_style(selection_style(theme))
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.mcp_idx));

    frame.render_stateful_widget(table, inset_left(chunks[1], 2), &mut state);
}

fn render_prompts(
    frame: &mut Frame<'_>,
    app: &App,
    data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let query = app.filter.query_lower();
    let visible: Vec<_> = data
        .prompts
        .rows
        .iter()
        .filter(|row| match &query {
            None => true,
            Some(q) => {
                row.prompt.name.to_lowercase().contains(q) || row.id.to_lowercase().contains(q)
            }
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(texts::tui_header_id()),
        Cell::from(texts::header_name()),
    ])
    .style(Style::default().fg(theme.dim).add_modifier(Modifier::BOLD));

    let rows = visible.iter().map(|row| {
        Row::new(vec![
            Cell::from(if row.prompt.enabled {
                texts::tui_marker_active()
            } else {
                texts::tui_marker_inactive()
            }),
            Cell::from(row.id.clone()),
            Cell::from(row.prompt.name.clone()),
        ])
    });

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::menu_manage_prompts());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        render_key_bar_center(
            frame,
            chunks[0],
            theme,
            &[
                ("Enter", texts::tui_key_view()),
                ("a", texts::tui_key_activate()),
                ("x", texts::tui_key_deactivate_active()),
                ("e", texts::tui_key_edit()),
                ("d", texts::tui_key_delete()),
            ],
        );
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(18),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::NONE))
    .row_highlight_style(selection_style(theme))
    .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.prompt_idx));
    frame.render_stateful_widget(table, inset_left(chunks[1], 2), &mut state);
}

fn config_items_filtered(app: &App) -> Vec<ConfigItem> {
    let Some(q) = app.filter.query_lower() else {
        return ConfigItem::ALL.to_vec();
    };
    ConfigItem::ALL
        .iter()
        .cloned()
        .filter(|item| config_item_label(item).to_lowercase().contains(&q))
        .collect()
}

fn config_item_label(item: &ConfigItem) -> &'static str {
    match item {
        ConfigItem::Path => texts::tui_config_item_show_path(),
        ConfigItem::ShowFull => texts::tui_config_item_show_full(),
        ConfigItem::Export => texts::tui_config_item_export(),
        ConfigItem::Import => texts::tui_config_item_import(),
        ConfigItem::Backup => texts::tui_config_item_backup(),
        ConfigItem::Restore => texts::tui_config_item_restore(),
        ConfigItem::Validate => texts::tui_config_item_validate(),
        ConfigItem::CommonSnippet => texts::tui_config_item_common_snippet(),
        ConfigItem::Reset => texts::tui_config_item_reset(),
    }
}

fn render_config(
    frame: &mut Frame<'_>,
    app: &App,
    _data: &UiData,
    area: Rect,
    theme: &super::theme::Theme,
) {
    let items = config_items_filtered(app);
    let rows = items
        .iter()
        .map(|item| Row::new(vec![Cell::from(pad2(config_item_label(item)))]));

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::tui_config_title());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        let mut keys = vec![("Enter", texts::tui_key_select())];
        if matches!(items.get(app.config_idx), Some(ConfigItem::CommonSnippet)) {
            keys.push(("e", texts::tui_key_edit_snippet()));
        }
        render_key_bar_center(frame, chunks[0], theme, &keys);
    }

    let table = Table::new(rows, [Constraint::Min(10)])
        .block(Block::default().borders(Borders::NONE))
        .row_highlight_style(selection_style(theme))
        .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.config_idx));
    frame.render_stateful_widget(table, inset_left(chunks[1], 2), &mut state);
}

fn render_settings(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &super::theme::Theme) {
    let header = Row::new(vec![Cell::from(
        pad2(texts::tui_settings_header_language()),
    )])
    .style(Style::default().fg(theme.dim).add_modifier(Modifier::BOLD));

    let rows = [
        crate::cli::i18n::Language::English,
        crate::cli::i18n::Language::Chinese,
    ]
    .iter()
    .map(|lang| Row::new(vec![Cell::from(pad2(lang.display_name()))]));

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(pane_border_style(app, Focus::Content, theme))
        .title(texts::menu_settings());
    frame.render_widget(outer.clone(), area);
    let inner = outer.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    if app.focus == Focus::Content {
        render_key_bar_center(
            frame,
            chunks[0],
            theme,
            &[("Enter", texts::tui_key_apply())],
        );
    }

    let table = Table::new(rows, [Constraint::Min(10)])
        .header(header)
        .block(Block::default().borders(Borders::NONE))
        .row_highlight_style(selection_style(theme))
        .highlight_symbol(highlight_symbol(theme));

    let mut state = TableState::default();
    state.select(Some(app.language_idx));
    frame.render_stateful_widget(table, inset_left(chunks[1], 2), &mut state);
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect, theme: &super::theme::Theme) {
    let mut spans = if app.filter.active {
        vec![Span::styled(
            texts::tui_footer_filter_mode(),
            Style::default().fg(theme.dim),
        )]
    } else {
        if theme.no_color {
            vec![Span::styled(
                format!(
                    "{} {}  {} {}",
                    texts::tui_footer_group_nav(),
                    texts::tui_footer_nav_keys(),
                    texts::tui_footer_group_actions(),
                    texts::tui_footer_action_keys_global()
                ),
                Style::default(),
            )]
        } else {
            let nav_bg = Color::DarkGray;
            let act_bg = Color::Gray;

            let nav_style = Style::default().fg(Color::White).bg(nav_bg);
            let nav_label_style = nav_style.add_modifier(Modifier::BOLD);
            let act_style = Style::default().fg(Color::White).bg(act_bg);
            let act_label_style = act_style.add_modifier(Modifier::BOLD);

            vec![
                Span::styled(" ", nav_style),
                Span::styled(texts::tui_footer_group_nav(), nav_label_style),
                Span::styled(" ", nav_style),
                Span::styled(texts::tui_footer_nav_keys(), nav_style),
                Span::styled(" ", nav_style),
                Span::raw(" "),
                Span::styled(" ", act_style),
                Span::styled(texts::tui_footer_group_actions(), act_label_style),
                Span::styled(" ", act_style),
                Span::styled(texts::tui_footer_action_keys_global(), act_style),
                Span::styled(" ", act_style),
            ]
        }
    };

    if let Some(toast) = &app.toast {
        let (prefix, color) = match toast.kind {
            ToastKind::Info => (texts::tui_toast_prefix_info(), theme.accent),
            ToastKind::Success => (texts::tui_toast_prefix_success(), theme.ok),
            ToastKind::Warning => (texts::tui_toast_prefix_warning(), theme.warn),
            ToastKind::Error => (texts::tui_toast_prefix_error(), theme.err),
        };
        spans.push(Span::raw("  "));
        spans.push(Span::styled(prefix, Style::default().fg(color)));
        spans.push(Span::raw(toast.message.clone()));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_overlay(frame: &mut Frame<'_>, app: &App, data: &UiData, theme: &super::theme::Theme) {
    match &app.overlay {
        Overlay::None => {}
        Overlay::Help => {
            let area = centered_rect(70, 70, frame.area());
            frame.render_widget(Clear, area);

            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(texts::tui_help_title());
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(frame, chunks[0], theme, &[("Esc", texts::tui_key_close())]);

            let lines = texts::tui_help_text()
                .lines()
                .map(|s| Line::raw(s.to_string()))
                .collect::<Vec<_>>();
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
        }
        Overlay::Confirm(confirm) => {
            let area = centered_rect(60, 35, frame.area());
            frame.render_widget(Clear, area);
            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(confirm.title.clone());
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("Enter", texts::tui_key_yes()),
                    ("Esc", texts::tui_key_cancel()),
                ],
            );

            frame.render_widget(
                Paragraph::new(Line::raw(confirm.message.clone()))
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: false }),
                chunks[1],
            );
        }
        Overlay::TextInput(input) => {
            let area = centered_rect(70, 35, frame.area());
            frame.render_widget(Clear, area);

            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(input.title.clone())
                .style(if theme.no_color {
                    Style::default()
                } else {
                    Style::default().bg(Color::Black)
                });

            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(2),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("Enter", texts::tui_key_submit()),
                    ("Esc", texts::tui_key_cancel()),
                ],
            );

            frame.render_widget(
                Paragraph::new(vec![Line::raw(input.prompt.clone()), Line::raw("")])
                    .wrap(Wrap { trim: false }),
                chunks[1],
            );

            let input_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.accent))
                .title(texts::tui_input_title())
                .style(if theme.no_color {
                    Style::default()
                } else {
                    Style::default().bg(Color::Black)
                });
            let input_inner = input_block.inner(chunks[2]);
            frame.render_widget(input_block, chunks[2]);

            let available = input_inner.width.saturating_sub(0) as usize;
            let full = input.buffer.clone();
            let cursor = full.chars().count();
            let start = cursor.saturating_sub(available);
            let visible = full.chars().skip(start).take(available).collect::<String>();
            frame.render_widget(
                Paragraph::new(Line::from(Span::raw(visible)))
                    .wrap(Wrap { trim: false })
                    .style(Style::default()),
                input_inner,
            );

            let cursor_x = input_inner.x + (cursor.saturating_sub(start) as u16);
            let cursor_y = input_inner.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        Overlay::BackupPicker { selected } => {
            let area = centered_rect(80, 80, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(texts::tui_backup_picker_title());
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("Enter", texts::tui_key_restore()),
                    ("Esc", texts::tui_key_cancel()),
                ],
            );

            let items = data.config.backups.iter().map(|backup| {
                ListItem::new(Line::from(Span::raw(format!(
                    "{}  ({})",
                    backup.display_name, backup.id
                ))))
            });

            let list = List::new(items)
                .highlight_style(selection_style(theme))
                .highlight_symbol(highlight_symbol(theme));

            let mut state = ListState::default();
            state.select(Some(*selected));
            frame.render_stateful_widget(list, chunks[1], &mut state);
        }
        Overlay::TextView(view) => {
            let area = centered_rect(90, 90, frame.area());
            frame.render_widget(Clear, area);

            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(view.title.clone());
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("↑↓", texts::tui_key_scroll()),
                    ("Esc", texts::tui_key_close()),
                ],
            );

            let height = chunks[1].height as usize;
            let start = view.scroll.min(view.lines.len());
            let end = (start + height).min(view.lines.len());
            let shown = view.lines[start..end]
                .iter()
                .map(|s| Line::raw(s.clone()))
                .collect::<Vec<_>>();

            frame.render_widget(Paragraph::new(shown).wrap(Wrap { trim: false }), chunks[1]);
        }
        Overlay::CommonSnippetView(view) => {
            let area = centered_rect(90, 90, frame.area());
            frame.render_widget(Clear, area);

            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(view.title.clone());
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("a", texts::tui_key_apply()),
                    ("c", texts::tui_key_clear()),
                    ("e", texts::tui_key_edit()),
                    ("↑↓", texts::tui_key_scroll()),
                    ("Esc", texts::tui_key_close()),
                ],
            );

            let height = chunks[1].height as usize;
            let start = view.scroll.min(view.lines.len());
            let end = (start + height).min(view.lines.len());
            let shown = view.lines[start..end]
                .iter()
                .map(|s| Line::raw(s.clone()))
                .collect::<Vec<_>>();

            frame.render_widget(Paragraph::new(shown).wrap(Wrap { trim: false }), chunks[1]);
        }
        Overlay::McpAppsPicker {
            name,
            selected,
            apps,
            ..
        } => {
            let area = centered_rect(60, 45, frame.area());
            frame.render_widget(Clear, area);

            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(texts::tui_mcp_apps_title(name));
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("x", texts::tui_key_toggle()),
                    ("Enter", texts::tui_key_apply()),
                    ("Esc", texts::tui_key_cancel()),
                ],
            );

            let items = [
                crate::app_config::AppType::Claude,
                crate::app_config::AppType::Codex,
                crate::app_config::AppType::Gemini,
            ]
            .into_iter()
            .map(|app_type| {
                let enabled = apps.is_enabled_for(&app_type);
                let marker = if enabled {
                    texts::tui_marker_active()
                } else {
                    texts::tui_marker_inactive()
                };

                ListItem::new(Line::from(Span::raw(format!(
                    "{marker}  {}",
                    app_type.as_str()
                ))))
            });

            let list = List::new(items)
                .highlight_style(selection_style(theme))
                .highlight_symbol(highlight_symbol(theme));

            let mut state = ListState::default();
            state.select(Some(*selected));
            frame.render_stateful_widget(list, chunks[1], &mut state);
        }
        Overlay::SpeedtestRunning { url } => {
            let area = centered_rect(70, 30, frame.area());
            frame.render_widget(Clear, area);
            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(texts::tui_speedtest_title());
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(frame, chunks[0], theme, &[("Esc", texts::tui_key_close())]);
            frame.render_widget(
                Paragraph::new(Line::raw(texts::tui_speedtest_running(url)))
                    .wrap(Wrap { trim: false }),
                chunks[1],
            );
        }
        Overlay::SpeedtestResult { url, lines, scroll } => {
            let area = centered_rect(90, 90, frame.area());
            frame.render_widget(Clear, area);

            let title = texts::tui_speedtest_title_with_url(url);
            let outer = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme.dim))
                .title(title);
            frame.render_widget(outer.clone(), area);
            let inner = outer.inner(area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);

            render_key_bar(
                frame,
                chunks[0],
                theme,
                &[
                    ("↑↓", texts::tui_key_scroll()),
                    ("Esc", texts::tui_key_close()),
                ],
            );

            let height = chunks[1].height as usize;
            let start = (*scroll).min(lines.len());
            let end = (start + height).min(lines.len());
            let shown = lines[start..end]
                .iter()
                .map(|s| Line::raw(s.clone()))
                .collect::<Vec<_>>();

            frame.render_widget(Paragraph::new(shown).wrap(Wrap { trim: false }), chunks[1]);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn mask_api_key(key: &str) -> String {
    let mut iter = key.chars();
    let prefix: String = iter.by_ref().take(8).collect();
    if iter.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use serde_json::json;
    use std::sync::Mutex;

    use crate::{
        app_config::AppType,
        cli::tui::{
            app::{App, Focus, Overlay, TextInputState, TextSubmit},
            data::{
                ConfigSnapshot, McpSnapshot, PromptsSnapshot, ProviderRow, ProvidersSnapshot,
                UiData,
            },
            route::Route,
            theme::theme_for,
        },
        provider::Provider,
    };

    #[test]
    fn mask_api_key_handles_multibyte_safely() {
        let short = "你你你"; // 3 chars, 9 bytes
        let masked = super::mask_api_key(short);
        assert_eq!(masked, short);

        let long = "你".repeat(9);
        let masked = super::mask_api_key(&long);
        assert!(masked.ends_with("..."));
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match ENV_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn remove(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                None => std::env::remove_var(self.key),
                Some(v) => std::env::set_var(self.key, v),
            }
        }
    }

    fn render(app: &App, data: &UiData) -> Buffer {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal created");
        terminal
            .draw(|f| super::render(f, app, data))
            .expect("draw ok");
        terminal.backend().buffer().clone()
    }

    fn line_at(buf: &Buffer, y: u16) -> String {
        let mut out = String::new();
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out
    }

    fn all_text(buf: &Buffer) -> String {
        let mut all = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                all.push_str(buf[(x, y)].symbol());
            }
            all.push('\n');
        }
        all
    }

    fn minimal_data(_app_type: &AppType) -> UiData {
        let provider = Provider::with_id(
            "p1".to_string(),
            "Demo Provider".to_string(),
            json!({}),
            None,
        );
        UiData {
            providers: ProvidersSnapshot {
                current_id: "p0".to_string(),
                rows: vec![ProviderRow {
                    id: "p1".to_string(),
                    provider,
                    api_url: Some("https://example.com".to_string()),
                    is_current: false,
                }],
            },
            mcp: McpSnapshot::default(),
            prompts: PromptsSnapshot::default(),
            config: ConfigSnapshot::default(),
        }
    }

    #[test]
    fn header_is_wrapped_in_a_rect_block() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);

        // Header is at y=0..=2, and should have an outer border at (0,0).
        assert_eq!(buf[(0, 0)].symbol(), "┌");
    }

    #[test]
    fn providers_pane_has_border_and_selected_row_is_accent() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Providers;
        app.focus = Focus::Content;
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);
        let theme = theme_for(&app.app_type);

        // Providers content area starts at x=30, y=3 (header=3 rows).
        let border_cell = &buf[(30, 3)];
        assert_eq!(border_cell.symbol(), "┌");
        assert_eq!(border_cell.fg, theme.accent);

        // Selected row should be highlighted with theme accent background.
        // Layout:
        // - content pane border (1)
        // - hint row (1)
        // - table header row (1)
        // - first data row (selected) (1)
        // Table is inset by 2 cells inside the content pane.
        let selected_row_cell = &buf[(33, 3 + 1 + 1 + 1)];
        assert_eq!(selected_row_cell.bg, theme.accent);
    }

    #[test]
    fn home_renders_ascii_logo() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Main;
        app.focus = Focus::Content;
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);
        let all = all_text(&buf);
        assert!(all.contains("___  ___"));
        assert!(all.contains("\\___|\\___|"));
    }

    #[test]
    fn home_does_not_repeat_welcome_title_in_body() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Main;
        app.focus = Focus::Content;
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);
        let all = all_text(&buf);

        let needle = "CC-Switch Interactive Mode";
        let count = all.matches(needle).count();
        assert_eq!(count, 1, "expected welcome title once, got {count}");
    }

    #[test]
    fn nav_does_not_show_manage_prefix_or_view_config() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Main;
        app.focus = Focus::Nav;
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);
        let all = all_text(&buf);

        assert!(
            !all.contains("Manage "),
            "expected nav to not include Manage prefix"
        );
        assert!(
            !all.contains("View Current Configuration"),
            "expected nav to not include View Current Configuration"
        );
    }

    #[test]
    fn text_input_overlay_renders_inner_input_box() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Config;
        app.focus = Focus::Content;
        app.overlay = Overlay::TextInput(TextInputState {
            title: "Demo".to_string(),
            prompt: "Enter value".to_string(),
            buffer: "hello".to_string(),
            submit: TextSubmit::ConfigBackupName,
        });
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);

        // Overlay area for centered_rect(70,35) in a 120x40 terminal:
        // width = 84, height = 14, top-left = ((120-84)/2, (40-14)/2) = (18, 13)
        let area_x = 18;
        let area_y = 13;
        let area_w = 84;
        let area_h = 14;

        // Outer border exists at (18,13). We also expect an inner input field border (another ┌)
        // somewhere inside the overlay.
        let mut inner_top_left_count = 0usize;
        for y in area_y..(area_y + area_h) {
            for x in area_x..(area_x + area_w) {
                if x == area_x && y == area_y {
                    continue;
                }
                if buf[(x as u16, y as u16)].symbol() == "┌" {
                    inner_top_left_count += 1;
                }
            }
        }

        assert!(
            inner_top_left_count >= 1,
            "expected an inner input box border in TextInput overlay"
        );
    }

    #[test]
    fn footer_shows_only_global_actions() {
        let _lock = lock_env();

        let prev = std::env::var("NO_COLOR").ok();
        std::env::set_var("NO_COLOR", "1");
        let _restore_no_color = EnvGuard {
            key: "NO_COLOR",
            prev,
        };

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Config;
        app.focus = Focus::Content;
        app.overlay = Overlay::CommonSnippetView(crate::cli::tui::app::TextViewState {
            title: "Common Snippet".to_string(),
            lines: vec!["{}".to_string()],
            scroll: 0,
        });
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);
        let footer = line_at(&buf, buf.area.height - 1);

        assert!(
            footer.contains("switch app") && footer.contains("/ filter"),
            "expected footer to show global actions; got: {footer:?}"
        );
        assert!(
            !footer.contains("clear") && !footer.contains("apply"),
            "expected footer to not show overlay/page actions; got: {footer:?}"
        );
    }

    #[test]
    fn backup_picker_overlay_shows_hint() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::Config;
        app.focus = Focus::Content;
        app.overlay = Overlay::BackupPicker { selected: 0 };

        let mut data = minimal_data(&app.app_type);
        data.config.backups = vec![crate::services::config::BackupInfo {
            id: "b1".to_string(),
            path: std::path::PathBuf::from("/tmp/b1.json"),
            timestamp: "20260131_000000".to_string(),
            display_name: "backup".to_string(),
        }];

        let buf = render(&app, &data);

        let mut all = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                all.push_str(buf[(x, y)].symbol());
            }
            all.push('\n');
        }

        assert!(
            all.contains("Enter")
                && all.contains("Esc")
                && (all.contains("restore") || all.contains("恢复")),
            "expected BackupPicker to show Enter/Esc restore hint"
        );
    }

    #[test]
    fn provider_detail_keys_line_does_not_include_q_back() {
        let _lock = lock_env();
        let _no_color = EnvGuard::remove("NO_COLOR");

        let mut app = App::new(Some(AppType::Claude));
        app.route = Route::ProviderDetail {
            id: "p1".to_string(),
        };
        app.focus = Focus::Content;
        let data = minimal_data(&app.app_type);

        let buf = render(&app, &data);
        let mut all = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                all.push_str(buf[(x, y)].symbol());
            }
            all.push('\n');
        }

        assert!(all.contains("speedtest"));
        assert!(
            !all.contains("q=back"),
            "provider detail inline keys should not include q=back"
        );
    }
}
